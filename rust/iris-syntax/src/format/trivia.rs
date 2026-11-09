use crate::{SyntaxElement, SyntaxKind as K, SyntaxNode, TextRange};

/// Patterns and glued format names are indivisible source slices. In particular,
/// whitespace leaves inside capture types are not formatter whitespace.
pub(super) struct Atom<'a> {
    pub text: &'a str,
    pub range: TextRange,
    pub kind: K,
    pub trailing_comment: bool,
}

/// Reuse the lexer rather than duplicate its number, name and punctuation rules.
/// Each source atom must still end at a token boundary after concatenation.
pub(super) fn needs_separator(parts: &[&str]) -> bool {
    let joined = parts.concat();
    let lexed = crate::lexer::lex(&joined, "formatter boundary".into());
    let mut boundary = 0;
    !lexed.diagnostics.is_empty()
        || parts[..parts.len() - 1].iter().any(|part| {
            boundary += part.len();
            !lexed.tokens.iter().any(|token| token.range.end == boundary)
        })
}

pub(super) fn atoms<'a>(source: &'a str, root: &SyntaxNode) -> Vec<Atom<'a>> {
    let mut result = Vec::new();
    collect(source, root, &mut result);
    let mut previous_end = 0;
    for atom in &mut result {
        atom.trailing_comment = atom.kind == K::Comment
            && previous_end > 0
            && !source[previous_end..usize::from(atom.range.start())].contains('\n');
        previous_end = usize::from(atom.range.end());
    }
    result
}

/// Collects the atoms of `root` in source order. The walk keeps its own stack,
/// as left operands nest far deeper than any thread's stack.
fn collect<'a>(source: &'a str, root: &SyntaxNode, result: &mut Vec<Atom<'a>>) {
    let mut pending = vec![SyntaxElement::Node(root.clone())];
    while let Some(element) = pending.pop() {
        match element {
            SyntaxElement::Node(node) => {
                if !collect_indivisible(source, &node, result) {
                    let start = pending.len();
                    pending.extend(node.children_with_tokens());
                    pending[start..].reverse();
                }
            }
            SyntaxElement::Token(token) if token.kind() != K::Whitespace => {
                push(source, token.text_range(), token.kind(), result);
            }
            SyntaxElement::Token(_) => {}
        }
    }
}

/// Collects a pattern or a format setting with a name as indivisible atoms,
/// and returns whether `node` was one.
fn collect_indivisible<'a>(source: &'a str, node: &SyntaxNode, result: &mut Vec<Atom<'a>>) -> bool {
    if node.kind() == K::Pattern {
        push(source, node.text_range(), K::Pattern, result);
        return true;
    }
    if node.kind() == K::Format {
        let tokens: Vec<_> = node
            .children_with_tokens()
            .filter_map(SyntaxElement::into_token)
            .filter(|token| !matches!(token.kind(), K::Whitespace | K::Comment))
            .collect();
        if tokens.len() >= 2 {
            // The parser permits only contiguous punctuation/name components.
            // Leading comments still pass through the ordinary token walk.
            let name = TextRange::new(
                tokens[1].text_range().start(),
                tokens.last().unwrap().text_range().end(),
            );
            for element in node.children_with_tokens() {
                if let Some(token) = element.into_token() {
                    let range = token.text_range();
                    if range.start() == name.start() {
                        push(source, name, token.kind(), result);
                    } else if !name.contains_range(range) && token.kind() != K::Whitespace {
                        push(source, range, token.kind(), result);
                    }
                }
            }
            return true;
        }
    }
    false
}

fn push<'a>(source: &'a str, range: TextRange, kind: K, result: &mut Vec<Atom<'a>>) {
    result.push(Atom {
        text: &source[std::ops::Range::<usize>::from(range)],
        range,
        kind,
        trailing_comment: false,
    });
}
