use super::{
    FormatEdit, FormatError, FormatOptions, RangeFormat, check_options, layout::Layout,
    statements::is_statement,
};
use crate::{
    DiagnosticPhase, LanguageVersion, SyntaxKind as K, SyntaxNode, TextRange, TextSize, parse,
};

/// Format the smallest complete statement sequence in a common root or block
/// covering `range`. Coordinates are half-open UTF-8 byte offsets; endpoints
/// outside the source or inside a scalar are rejected. A selection spanning
/// sections arms widens to the sections statement; one inside an arm's block
/// can select its body statements. A selection through `else` includes its if.
///
/// Widening includes attached trailing comments and the complete whitespace
/// runs on both sides, up to the nearest exterior token/comment or file edge.
/// These runs use the same blank-line, brace-edge and spacing rules as whole
/// documents, including the indentation before the following exterior token.
/// Every edit stays within the returned range;
/// bytes outside it are unchanged. No edits means the selection already matches.
/// Syntax errors owned by selected statements refuse the operation, including
/// missing-token errors reported at the next statement. Errors elsewhere do not.
/// For example, selecting `good:int` in `let bad=+ good:int` formats that clean
/// recovered declaration without changing the malformed definition.
///
/// A cursor chooses its containing statement, the following statement at an
/// exact boundary, or the preceding statement at EOF. Whitespace-only documents
/// are a no-op. A cursor in an error region without a clean statement is refused.
pub fn format_range(
    source: &[u8],
    origin: &str,
    version: LanguageVersion,
    range: TextRange,
    options: FormatOptions,
) -> Result<RangeFormat, FormatError> {
    check_options(options)?;
    let parsed = parse(source, origin, version).map_err(FormatError::Source)?;
    let source = std::str::from_utf8(source).expect("parse validated UTF-8");
    let requested = std::ops::Range::<usize>::from(range);
    if requested.end > source.len()
        || !source.is_char_boundary(requested.start)
        || !source.is_char_boundary(requested.end)
    {
        return Err(FormatError::InvalidRange);
    }
    if source.bytes().all(|b| b.is_ascii_whitespace()) {
        return Ok(RangeFormat {
            range,
            edits: Vec::new(),
        });
    }
    let root = parsed.syntax();
    let mut layout = Layout::new(source, &root, options.line_width);
    if root.children().next().is_none() {
        return Ok(make_result(
            source,
            root.text_range(),
            layout.render_document(),
        ));
    }
    let selection = include_comment_owner(&layout, range);
    let selected = select(&root, selection)
        .ok_or_else(|| FormatError::Syntax(parsed.diagnostics().to_vec()))?;
    check_syntax(
        &parsed,
        &selected,
        TextRange::new(
            selected[0].text_range().start(),
            selected.last().unwrap().text_range().end(),
        ),
    )?;
    let indent = indentation(&selected[0]);
    let extent = extent(&layout, &selected, range);
    check_syntax(&parsed, &selected, extent.bytes)?;
    layout.set_indent(extent.atoms.clone(), indent);
    for statement in &selected {
        layout.prepare_statement(statement, indent);
        layout.set_line(layout.atom_range(statement).start, indent);
    }
    prepare_boundary_gaps(&mut layout, &extent, indent);
    for statement in &selected {
        layout.format_statement(statement, indent);
    }
    let replacement = render_fragment(&layout, &extent);
    Ok(make_result(source, extent.bytes, replacement))
}

fn make_result(source: &str, range: TextRange, replacement: String) -> RangeFormat {
    let edits = if source[std::ops::Range::<usize>::from(range)] == replacement {
        Vec::new()
    } else {
        vec![FormatEdit { range, replacement }]
    };
    RangeFormat { range, edits }
}

fn include_comment_owner(layout: &Layout<'_>, range: TextRange) -> TextRange {
    let mut start = range.start();
    for (i, atom) in layout.atoms.iter().enumerate() {
        let selected = if range.is_empty() {
            atom.range.contains(range.start())
        } else {
            atom.range.intersect(range).is_some_and(|r| !r.is_empty())
        };
        if atom.trailing_comment && i > 0 {
            let owner = &layout.atoms[i - 1];
            // Selecting the gap before a trailing comment must include its
            // owner too, rather than absorb the comment into the next statement.
            let selects_gap =
                owner.range.end() <= range.start() && range.start() < atom.range.start();
            if selected || selects_gap {
                start = start.min(owner.range.start());
            }
        }
    }
    TextRange::new(start, range.end())
}

fn prepare_boundary_gaps(layout: &mut Layout<'_>, extent: &SelectionExtent, indent: usize) {
    let atoms = &extent.atoms;
    layout.set_line(atoms.start, indent);
    if atoms.start == 0 || layout.atoms[atoms.start - 1].text == "{" {
        layout.gaps[atoms.start].blank = false;
    }
    if let Some(next) = layout.atoms.get(atoms.end) {
        let closer = next.text == "}";
        layout.set_line(
            atoms.end,
            if closer {
                indent.saturating_sub(2)
            } else {
                indent
            },
        );
        if closer {
            layout.gaps[atoms.end].blank = false;
        }
    }
}

fn render_fragment(layout: &Layout<'_>, extent: &SelectionExtent) -> String {
    let atoms = extent.atoms.clone();
    let mut output = String::new();
    if atoms.start > 0 {
        layout.render_gap(atoms.start, &mut output);
    } else {
        output.extend(std::iter::repeat_n(' ', layout.gaps[atoms.start].indent));
    }
    output.push_str(&layout.render(atoms.clone()));
    if atoms.end < layout.atoms.len() {
        layout.render_gap(atoms.end, &mut output);
    } else {
        output.push('\n');
    }
    output
}

fn select(root: &SyntaxNode, range: TextRange) -> Option<Vec<SyntaxNode>> {
    let mut containers: Vec<_> = root
        .descendants()
        .filter(|node| {
            matches!(node.kind(), K::Root | K::Block) && node.text_range().contains_range(range)
        })
        .collect();
    containers.sort_by_key(|node| node.text_range().len());
    for container in containers {
        let statements: Vec<_> = container
            .children()
            .filter(|node| is_statement(node.kind()) || node.kind() == K::Error)
            .collect();
        if statements.is_empty() {
            continue;
        }
        let mut selected: Vec<_> = statements
            .iter()
            .filter(|node| {
                let span = node.text_range();
                if range.is_empty() {
                    span.contains(range.start())
                } else {
                    span.start() < range.end() && range.start() < span.end()
                }
            })
            .cloned()
            .collect();
        if selected.is_empty() {
            let next = statements
                .iter()
                .find(|node| node.text_range().start() >= range.start());
            selected.push(next.or_else(|| statements.last())?.clone());
        }
        if container.kind() == K::Block {
            let braces: Vec<_> = container
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| matches!(token.text(), "{" | "}"))
                .collect();
            let Some(open) = braces.first() else {
                continue;
            };
            let Some(close) = braces.last() else {
                continue;
            };
            if range.start() < open.text_range().end()
                || range.end() > close.text_range().start()
                || (range.is_empty() && close.text_range().contains(range.start()))
            {
                continue;
            }
        }
        return Some(selected);
    }
    None
}

fn indentation(statement: &SyntaxNode) -> usize {
    statement
        .ancestors()
        .skip(1)
        .map(|node| match node.kind() {
            K::Block | K::Sections => 2,
            _ => 0,
        })
        .sum()
}

struct SelectionExtent {
    bytes: TextRange,
    atoms: std::ops::Range<usize>,
}

fn extent(layout: &Layout<'_>, statements: &[SyntaxNode], requested: TextRange) -> SelectionExtent {
    let first = layout.atom_range(&statements[0]);
    let last = layout.atom_range(statements.last().unwrap());
    let mut atoms = first.start..last.end;
    while atoms.end < layout.atoms.len() && layout.atoms[atoms.end].trailing_comment {
        atoms.end += 1;
    }
    while atoms.start > 0
        && layout.atoms[atoms.start - 1].range.end() > requested.start()
        && layout.atoms[atoms.start - 1].kind == K::Comment
    {
        atoms.start -= 1;
    }
    while atoms.end < layout.atoms.len()
        && layout.atoms[atoms.end].range.start() < requested.end()
        && layout.atoms[atoms.end].kind == K::Comment
    {
        atoms.end += 1;
    }
    let previous_end = atoms
        .start
        .checked_sub(1)
        .map_or(0, |i| usize::from(layout.atoms[i].range.end()));
    let next_start = layout
        .atoms
        .get(atoms.end)
        .map_or(layout.source.len(), |atom| usize::from(atom.range.start()));
    SelectionExtent {
        bytes: TextRange::new(
            TextSize::try_from(previous_end).unwrap(),
            TextSize::try_from(next_start).unwrap(),
        ),
        atoms,
    }
}

fn check_syntax(
    parsed: &crate::Parse,
    statements: &[SyntaxNode],
    extent: TextRange,
) -> Result<(), FormatError> {
    let bad_node = statements.iter().any(|node| {
        node.descendants().any(|child| child.kind() == K::Error)
            || node
                .descendants_with_tokens()
                .filter_map(|e| e.into_token())
                .any(|t| t.kind() == K::Invalid)
    });
    // Parsing diagnostics belong to their Error nodes even when lookahead points
    // into a recovered sibling. Lexical errors instead own their raw bytes.
    let bad_diagnostics: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.phase != DiagnosticPhase::Parsing
                && (diagnostic
                    .range
                    .intersect(extent)
                    .is_some_and(|r| !r.is_empty())
                    || extent.contains(diagnostic.range.start()))
        })
        .cloned()
        .collect();
    if bad_node {
        return Err(FormatError::Syntax(parsed.diagnostics().to_vec()));
    }
    if !bad_diagnostics.is_empty() {
        return Err(FormatError::Syntax(bad_diagnostics));
    }
    Ok(())
}
