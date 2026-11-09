use std::ops::Range;

use rowan::WalkEvent;

use super::layout::Layout;
use crate::{SyntaxKind as K, SyntaxNode};

pub(super) fn is_statement(kind: K) -> bool {
    matches!(
        kind,
        K::Format
            | K::Setting
            | K::Declaration
            | K::List
            | K::TypeDefinition
            | K::Expect
            | K::Param
            | K::Line
            | K::Find
            | K::Skip
            | K::Index
            | K::Repeat
            | K::Each
            | K::Instances
            | K::Sections
            | K::If
            | K::Let
            | K::Check
    )
}

impl Layout<'_> {
    pub fn format_document(&mut self, root: &SyntaxNode) {
        self.prepare_sequence_children(root, 0);
        for statement in root.children() {
            self.format_statement(&statement, 0);
        }
    }

    pub fn prepare_statement(&mut self, node: &SyntaxNode, indent: usize) {
        let span = self.atom_range(node);
        self.set_indent(span.clone(), indent + 2);
        if let Some(gap) = self.gaps.get_mut(span.start) {
            gap.indent = indent;
        }
        self.prepare_punctuation(node);
        for child in node.children() {
            match child.kind() {
                K::Block => self.prepare_body(&child, indent),
                K::If | K::Declaration if node.kind() == K::If => {
                    let first = self.atom_range(&child).start;
                    self.prepare_statement(&child, indent);
                    if self.gaps[first].line {
                        self.gaps[first].indent = indent + 2;
                    }
                }
                _ => {}
            }
        }
        if node.kind() == K::Sections {
            self.prepare_body(node, indent);
        }
    }

    fn prepare_body(&mut self, node: &SyntaxNode, indent: usize) {
        let span = self.atom_range(node);
        let sig = self.significant(span.clone());
        let open = *sig
            .iter()
            .find(|&&i| self.atoms[i].text == "{")
            .expect("clean block has opener");
        let close = *sig.last().expect("clean block has closer");
        self.set_indent(open + 1..close, indent + 2);
        self.prepare_sequence_children(node, indent + 2);
        // R3 trims only gaps directly at brace edges, including an empty body.
        if open + 1 < self.gaps.len() {
            self.gaps[open + 1].blank = false;
        }
        self.gaps[close].blank = false;
        if open + 1 == close {
            self.gaps[close].line = false;
            self.set_tight(close);
        } else {
            if !self.atoms[open + 1].trailing_comment {
                self.set_line(open + 1, indent + 2);
            }
            self.set_line(close, indent);
        }
    }

    fn prepare_sequence_children(&mut self, node: &SyntaxNode, indent: usize) {
        for child in node.children() {
            if is_statement(child.kind()) || child.kind() == K::Arm {
                self.prepare_statement(&child, indent);
                self.set_line(self.atom_range(&child).start, indent);
            }
        }
    }

    /// Tightens punctuation in `root` and the nodes below it, in preorder,
    /// without entering nested statements or bodies. The walk keeps its own
    /// stack, as left operands nest far deeper than any thread's stack.
    fn prepare_punctuation(&mut self, root: &SyntaxNode) {
        let mut pending = vec![root.clone()];
        while let Some(node) = pending.pop() {
            if self.prepare_node_punctuation(&node) {
                let start = pending.len();
                pending.extend(node.children().filter(|child| {
                    !is_statement(child.kind()) && !matches!(child.kind(), K::Block | K::Arm)
                }));
                pending[start..].reverse();
            }
        }
    }

    /// Tightens the punctuation of `node` itself, and returns whether its
    /// children need the same.
    fn prepare_node_punctuation(&mut self, node: &SyntaxNode) -> bool {
        let sig = if matches!(
            node.kind(),
            K::Call
                | K::Comprehension
                | K::Dimension
                | K::Unary
                | K::AlphabetOption
                | K::Field
                | K::Range
        ) {
            self.significant(self.atom_range(node))
        } else {
            Vec::new()
        };
        match node.kind() {
            K::Call | K::Comprehension => {
                if let Some(&open) = sig.get(1) {
                    self.set_tight(open);
                }
            }
            K::Dimension => {
                if let Some(&open) = sig.first() {
                    self.set_tight(open);
                }
            }
            K::IndexExpression => {
                if let Some(open) = node
                    .children_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| token.text() == "[")
                {
                    self.set_tight(self.atoms_in(open.text_range()).start);
                }
            }
            K::Unary => {
                if let Some(&operand) = sig.get(1) {
                    self.set_tight(operand);
                }
            }
            K::AlphabetOption => {
                if let Some(&minus) = sig.get(1)
                    && self.atoms[minus].text == "-"
                    && let Some(&value) = sig.get(2)
                {
                    self.set_tight(value);
                }
            }
            K::Field => self.set_field_spacing(&sig),
            K::Range => {
                if let Some(position) = sig
                    .iter()
                    .position(|&i| matches!(self.atoms[i].text, ".." | "..<"))
                {
                    if position > 0 {
                        self.set_tight(sig[position]);
                    }
                    if let Some(&next) = sig.get(position + 1) {
                        self.set_tight(next);
                    }
                }
            }
            // Pattern contents and format-name components are already atoms.
            K::Pattern | K::Capture | K::Format => return false,
            _ => {}
        }
        true
    }

    fn set_field_spacing(&mut self, sig: &[usize]) {
        let Some(position) = sig.iter().rposition(|&i| self.atoms[i].text == ".") else {
            return;
        };
        let dot = sig[position];
        let before = sig[position - 1];
        let after = sig[position + 1];
        if super::trivia::needs_separator(&[
            self.atoms[before].text,
            self.atoms[dot].text,
            self.atoms[after].text,
        ]) {
            // R5 keeps spaces on both sides of a field dot when gluing would
            // change its tokens, even if just one separator would suffice.
            self.gaps[dot].space = true;
            self.gaps[after].space = true;
        } else {
            self.set_tight(dot);
            self.set_tight(after);
        }
    }

    pub fn format_statement(&mut self, node: &SyntaxNode, indent: usize) {
        let span = self.atom_range(node);
        if span.is_empty() {
            return;
        }
        let header_end = node
            .children()
            .find(|child| child.kind() == K::Block)
            .map(|block| {
                self.significant(self.atom_range(&block))
                    .into_iter()
                    .find(|&i| self.atoms[i].text == "{")
                    .expect("clean block has opener")
                    + 1
            })
            .or_else(|| {
                (node.kind() == K::Sections)
                    .then(|| span.clone().find(|&i| self.atoms[i].text == "{").unwrap() + 1)
            })
            .unwrap_or(span.end);
        let header = span.start..header_end;
        let sig = self.significant(header.clone());
        if node.kind() == K::Check
            && self.overflows(header.clone())
            && let Some(&message) = sig.last()
            && self.atoms[message].kind == K::String
            && node
                .children()
                .any(|child| self.atom_range(&child).end <= message)
        {
            self.set_line(message, indent + 2);
        }
        let groups = self.collect_option_groups(node, header.clone());
        if self.overflows(header.clone()) {
            for group in &groups {
                self.set_line(group.start, indent + 2);
            }
        }
        self.wrap_strings(node, header.clone(), &groups, indent);
        self.wrap_header_expressions(node, header_end, indent, &groups);
        // Atomic arguments have no internal break; move only when useful.
        self.move_atomic_argument(node, header.clone(), &sig, indent);
        for child in node.children() {
            match child.kind() {
                K::Block => {
                    for statement in child.children() {
                        self.format_statement(&statement, indent + 2);
                    }
                }
                K::Arm if node.kind() == K::Sections => self.format_statement(&child, indent + 2),
                K::If | K::Declaration if node.kind() == K::If => {
                    self.format_statement(&child, indent)
                }
                _ => {}
            }
        }
    }

    fn collect_option_groups(&self, node: &SyntaxNode, header: Range<usize>) -> Vec<Range<usize>> {
        let mut starts = Vec::new();
        let mut descendants = node.preorder();
        while let Some(event) = descendants.next() {
            let WalkEvent::Enter(child) = event else {
                continue;
            };
            if matches!(child.kind(), K::Pattern | K::Capture) {
                descendants.skip_subtree();
                continue;
            }
            if matches!(child.kind(), K::Layout | K::Offset | K::AlphabetOption) {
                // An option node can begin with the previous line's trailing
                // comment; the group starts at the option keyword after it.
                let keyword = self.significant(self.atom_range(&child)).first().copied();
                if let Some(start) = keyword.filter(|start| header.contains(start)) {
                    starts.push(start);
                }
            }
        }
        // These keyword groups are direct tokens in the grammar, not expression
        // names. Never interpret a contextual identifier inside an expression.
        let direct: Vec<_> = node
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| !matches!(t.kind(), K::Whitespace | K::Comment))
            .collect();
        if matches!(node.kind(), K::List | K::Repeat | K::Sections) {
            for token in direct {
                if matches!(token.text(), "until" | "as" | "skip") {
                    let span = self.atoms_in(token.text_range());
                    starts.push(span.start);
                }
            }
            for terminators in node.children().filter(|n| n.kind() == K::Terminators) {
                let sig = self.significant(self.atom_range(&terminators));
                if let Some(&store) = sig.iter().find(|&&i| self.atoms[i].text == "store") {
                    starts.push(store);
                }
            }
        }
        starts.sort_unstable();
        starts.dedup();
        let end = header
            .clone()
            .find(|&i| self.atoms[i].text == "{")
            .unwrap_or(header.end);
        starts
            .iter()
            .enumerate()
            .map(|(i, &start)| start..starts.get(i + 1).copied().unwrap_or(end))
            .collect()
    }

    fn wrap_strings(
        &mut self,
        node: &SyntaxNode,
        header: Range<usize>,
        groups: &[Range<usize>],
        indent: usize,
    ) {
        let list = node.kind() == K::Expect
            || (node.kind() == K::Setting
                && matches!(self.atoms[header.start].text, "comment" | "stop"));
        let lists: Vec<_> = if list {
            vec![(header.clone(), indent)]
        } else {
            groups
                .iter()
                .filter(|g| self.atoms[g.start].text == "until")
                .map(|g| {
                    (
                        g.clone(),
                        if self.gaps[g.start].line {
                            indent + 2
                        } else {
                            indent
                        },
                    )
                })
                .collect()
        };
        for (group, base) in lists {
            let strings: Vec<_> = group
                .clone()
                .filter(|&i| self.atoms[i].kind == K::String)
                .collect();
            if !strings.is_empty() && self.overflows(group.clone()) {
                for string in strings {
                    self.set_line(string, base + 2);
                }
            }
        }
    }

    fn wrap_header_expressions(
        &mut self,
        node: &SyntaxNode,
        header_end: usize,
        indent: usize,
        groups: &[Range<usize>],
    ) {
        for child in node.children() {
            let span = self.atom_range(&child);
            if span.start >= header_end {
                break;
            }
            let indent = groups
                .iter()
                .find(|group| group.contains(&span.start) && self.gaps[group.start].line)
                .map_or(indent, |group| self.gaps[group.start].indent);
            if super::expressions::is_expression(child.kind()) || child.kind() == K::Range {
                self.format_expression(&child, indent);
                self.move_expression(&child, indent);
            } else {
                match child.kind() {
                    K::Type => {
                        self.wrap_header_expressions(&child, span.end, indent, groups);
                        self.move_expression(&child, indent);
                    }
                    K::Layout | K::Offset | K::Terminators => {
                        self.wrap_header_expressions(&child, span.end, indent, groups)
                    }
                    K::Alphabet => {
                        self.wrap_header_expressions(&child, span.end, indent, groups);
                        let sig = self.significant(span.clone());
                        self.move_single_argument(span.clone(), sig[1], indent);
                    }
                    K::AlphabetOption => {
                        let sig = self.significant(span.clone());
                        if sig.len() == 2 && self.atoms[sig[1]].kind == K::String {
                            self.move_single_argument(span, sig[1], indent);
                        }
                    }
                    K::Dimension => self.format_expression(&child, indent),
                    _ => {}
                }
            }
        }
    }

    fn move_atomic_argument(
        &mut self,
        node: &SyntaxNode,
        header: Range<usize>,
        sig: &[usize],
        indent: usize,
    ) {
        if matches!(node.kind(), K::Setting | K::Format | K::Line | K::Find)
            && sig.len() == 2
            && (node.kind() != K::Setting || self.atoms[sig[1]].kind == K::String)
        {
            self.move_single_argument(header, sig[1], indent);
        }
    }

    fn move_single_argument(&mut self, span: Range<usize>, argument: usize, indent: usize) {
        let suffix = self.suffix(argument + 1);
        if self.overflows(span.start..argument + 1)
            && indent + 2 + self.atoms[argument].text.chars().count() + suffix <= self.width
        {
            self.set_line(argument, indent + 2);
        }
    }
}
