use std::ops::Range;

use super::trivia::{self, Atom};
use crate::{SyntaxKind as K, SyntaxNode, TextRange};

/// The gap before an atom coalesces structural and trivia line requests.
/// Indentation is always a logical number of spaces, never an opener column.
#[derive(Clone, Copy, Default)]
pub(super) struct Gap {
    pub space: bool,
    pub line: bool,
    pub blank: bool,
    pub indent: usize,
}

pub(super) struct Layout<'a> {
    pub source: &'a str,
    pub atoms: Vec<Atom<'a>>,
    pub gaps: Vec<Gap>,
    pub width: usize,
}

impl<'a> Layout<'a> {
    pub fn new(source: &'a str, root: &SyntaxNode, width: usize) -> Self {
        let atoms = trivia::atoms(source, root);
        let mut gaps = Vec::with_capacity(atoms.len());
        for (i, atom) in atoms.iter().enumerate() {
            let previous = i.checked_sub(1).map(|i| &atoms[i]);
            let whitespace = &source[previous.map_or(0, |a| usize::from(a.range.end()))
                ..usize::from(atom.range.start())];
            let blank = whitespace.bytes().filter(|&b| b == b'\n').count() >= 2;
            let line = blank
                || previous.is_some_and(|a| a.kind == K::Comment)
                || (atom.kind == K::Comment && !atom.trailing_comment);
            gaps.push(Gap {
                space: i > 0,
                line,
                blank,
                indent: 0,
            });
        }
        let mut layout = Self {
            source,
            atoms,
            gaps,
            width,
        };
        for i in 1..layout.atoms.len() {
            if !layout.atoms[i].trailing_comment
                && (matches!(layout.atoms[i].text, ":" | "," | ")" | "]")
                    || matches!(layout.atoms[i - 1].text, "(" | "["))
            {
                layout.set_tight(i);
            }
        }
        layout
    }

    pub fn atom_range(&self, node: &SyntaxNode) -> Range<usize> {
        self.atoms_in(node.text_range())
    }

    pub fn atoms_in(&self, range: TextRange) -> Range<usize> {
        let start = self
            .atoms
            .partition_point(|atom| atom.range.end() <= range.start());
        let end = self
            .atoms
            .partition_point(|atom| atom.range.start() < range.end());
        start..end
    }

    pub fn significant(&self, span: Range<usize>) -> Vec<usize> {
        span.filter(|&i| self.atoms[i].kind != K::Comment).collect()
    }

    pub fn set_line(&mut self, index: usize, indent: usize) {
        if let Some(gap) = self.gaps.get_mut(index) {
            gap.line = true;
            gap.indent = indent;
        }
    }

    pub fn set_tight(&mut self, index: usize) {
        if index == 0 || index >= self.atoms.len() {
            return;
        }
        // Check the whole joined run: `gecode`, `.`, `value` needs three
        // atoms before the lexer can recognize a host-constant token.
        let mut start = index - 1;
        while start > 0 && !self.gaps[start].space && !self.gaps[start].line {
            start -= 1;
        }
        let mut end = index + 1;
        while end < self.atoms.len() && !self.gaps[end].space && !self.gaps[end].line {
            end += 1;
        }
        let parts: Vec<_> = self.atoms[start..end]
            .iter()
            .map(|atom| atom.text)
            .collect();
        let space = trivia::needs_separator(&parts);
        if let Some(gap) = self.gaps.get_mut(index) {
            gap.space = space;
        }
    }

    pub fn set_indent(&mut self, span: Range<usize>, indent: usize) {
        for gap in &mut self.gaps[span] {
            gap.indent = indent;
        }
    }

    /// Ignore a trailing comment's protected overflow when choosing breaks.
    /// Existing hard breaks are measured as actual segments, never flattened.
    /// Always include the complete suffix sharing the final line.
    pub fn overflows(&self, span: Range<usize>) -> bool {
        if span.is_empty() {
            return false;
        }
        let mut column = self.column_before(span.start);
        for i in span.clone() {
            let gap = self.gaps[i];
            if gap.line {
                column = gap.indent;
            } else if gap.space {
                column += 1;
            }
            if self.atoms[i].kind != K::Comment {
                column += self.atoms[i].text.chars().count();
                if column > self.width {
                    return true;
                }
            }
        }
        column.saturating_add(self.suffix(span.end)) > self.width
    }

    pub fn column_before(&self, index: usize) -> usize {
        let start = (0..index).rev().find(|&i| self.gaps[i].line).unwrap_or(0);
        let mut column = 0;
        for i in start..index {
            let gap = self.gaps[i];
            if gap.line {
                column = gap.indent;
            } else if i > 0 && gap.space {
                column += 1;
            }
            column += self.atoms[i].text.chars().count();
        }
        column
    }

    /// Complete suffix through the next mandatory break, regardless of CST
    /// ownership: compact blocks, joined else branches, options and delimiters.
    pub fn suffix(&self, end: usize) -> usize {
        let mut width = 0;
        for i in end..self.atoms.len() {
            if self.gaps[i].line || self.atoms[i].kind == K::Comment {
                break;
            }
            width += usize::from(self.gaps[i].space) + self.atoms[i].text.chars().count();
        }
        width
    }

    pub fn render(&self, span: Range<usize>) -> String {
        let mut output = String::new();
        for i in span.clone() {
            if i > span.start {
                self.render_gap(i, &mut output);
            }
            output.push_str(self.atoms[i].text);
        }
        output
    }

    pub fn render_gap(&self, index: usize, output: &mut String) {
        let gap = self.gaps[index];
        if gap.line {
            output.push('\n');
            if gap.blank {
                output.push('\n');
            }
            output.extend(std::iter::repeat_n(' ', gap.indent));
        } else if gap.space {
            output.push(' ');
        }
    }

    pub fn render_document(&self) -> String {
        let mut output = self.render(0..self.atoms.len());
        if !output.is_empty() {
            output.push('\n');
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::Layout;
    use crate::{LanguageVersion, SyntaxKind, lexer, parse};

    fn tokens(source: &str) -> Vec<(SyntaxKind, String)> {
        lexer::lex(source, "boundary test".into())
            .tokens
            .into_iter()
            .filter(|token| token.kind != SyntaxKind::Whitespace)
            .map(|token| (token.kind, source[token.range].to_owned()))
            .collect()
    }

    #[test]
    fn tightened_gaps_preserve_tokens() {
        // Exercise every punctuation spelling and representative names,
        // numbers and protected strings, even where a pair isn't grammatical.
        let spellings = [
            "gecode",
            "value",
            "e",
            "e2",
            "1",
            "1.",
            "1.e2",
            "0.5",
            "\"a\"",
            "\"{:alphabet \\\"a\\\" base 0}\"",
            "{",
            "}",
            "[",
            "]",
            "(",
            ")",
            "=",
            "<",
            ">",
            "+",
            "-",
            "*",
            "/",
            "%",
            "^",
            "!",
            ",",
            ".",
            ":",
            "..",
            "..<",
            "==",
            "!=",
            "<=",
            ">=",
            "&&",
            "||",
        ];
        let mut sources = Vec::new();
        for left in spellings {
            for right in spellings {
                sources.push(format!("{left} {right}"));
            }
        }
        sources.extend([
            "gecode . value".to_owned(),
            "1 . e2".to_owned(),
            "1. .. 2".to_owned(),
            "1. ..< 2".to_owned(),
        ]);
        let mut gaps = 0;
        for source in &sources {
            let expected = tokens(source);
            let parsed = parse(source.as_bytes(), "boundary test", LanguageVersion::V0_1).unwrap();
            let mut layout = Layout::new(source, &parsed.syntax(), 100);
            assert_eq!(
                tokens(&layout.render_document()),
                expected,
                "initial gaps: {source}"
            );
            for i in 1..layout.atoms.len() {
                layout.set_tight(i);
                gaps += 1;
                assert_eq!(
                    tokens(&layout.render_document()),
                    expected,
                    "gap {i}: {source}"
                );
            }
        }
        println!(
            "token boundary sources={}, tightened gap checks={gaps}",
            sources.len()
        );
    }
}
