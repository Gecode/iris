use super::layout::Layout;
use crate::{SyntaxKind as K, SyntaxNode};

pub(super) fn is_expression(kind: K) -> bool {
    matches!(
        kind,
        K::Literal
            | K::Name
            | K::HostConstant
            | K::Parenthesized
            | K::Unary
            | K::Binary
            | K::Power
            | K::Call
            | K::Comprehension
            | K::IndexExpression
            | K::Field
    )
}

/// One step of laying out an expression. Steps run in the order in which a
/// recursive walk would reach them; the walk keeps its own stack, as left
/// operands nest far deeper than any thread's stack (Specification §5.12).
enum Step {
    Format(SyntaxNode, usize),
    /// After a dimension's children: wrap it if it still overflows.
    WrapDimension(SyntaxNode, usize),
    /// After an index expression's children: break around a complex index.
    WrapIndex(SyntaxNode, usize),
}

/// Schedules `nodes` to be formatted in order, after the steps above them.
fn push_in_order(
    pending: &mut Vec<Step>,
    nodes: impl IntoIterator<Item = SyntaxNode>,
    indent: usize,
) {
    let start = pending.len();
    pending.extend(nodes.into_iter().map(|node| Step::Format(node, indent)));
    pending[start..].reverse();
}

impl Layout<'_> {
    pub fn format_expression(&mut self, node: &SyntaxNode, indent: usize) {
        let mut pending = vec![Step::Format(node.clone(), indent)];
        while let Some(step) = pending.pop() {
            match step {
                Step::Format(node, indent) => {
                    self.format_expression_step(&node, indent, &mut pending)
                }
                Step::WrapDimension(node, indent) => {
                    if self.overflows(self.atom_range(&node))
                        && node.children().any(|child| !is_atomic(&child))
                    {
                        self.wrap_delimited(&node, indent, &mut pending);
                    }
                }
                Step::WrapIndex(node, indent) => {
                    let span = self.atom_range(&node);
                    let index = node.children().nth(1).expect("clean index has subscript");
                    if self.overflows(span.clone()) && !is_atomic(&index) {
                        let sig = self.significant(span);
                        self.set_line(self.atom_range(&index).start, indent + 2);
                        self.set_line(*sig.last().unwrap(), indent);
                        pending.push(Step::Format(index, indent + 2));
                    }
                }
            }
        }
    }

    fn format_expression_step(
        &mut self,
        node: &SyntaxNode,
        indent: usize,
        pending: &mut Vec<Step>,
    ) {
        let span = self.atom_range(node);
        if !self.overflows(span.clone()) {
            return;
        }
        match node.kind() {
            K::Binary | K::Power => {
                let mut operands = Vec::new();
                let mut operators = Vec::new();
                self.collect_binary_chain(node, precedence(node), &mut operands, &mut operators);
                for operator in operators {
                    let next = (operator + 1..span.end).find(|&i| self.atoms[i].kind != K::Comment);
                    if let Some(next) = next {
                        self.set_line(next, indent + 2);
                    }
                }
                push_in_order(pending, operands, indent + 2);
            }
            K::Range => {
                let bounds: Vec<_> = node
                    .children()
                    .filter(|n| is_expression(n.kind()))
                    .collect();
                if bounds.len() == 2 {
                    self.set_line(self.atom_range(&bounds[1]).start, indent + 2);
                }
                push_in_order(pending, bounds, indent + 2);
            }
            K::Call => self.wrap_call(node, indent, pending),
            K::Comprehension => self.wrap_comprehension(node, indent, pending),
            K::Parenthesized => self.wrap_delimited(node, indent, pending),
            K::Dimension => {
                pending.push(Step::WrapDimension(node.clone(), indent));
                push_in_order(pending, node.children(), indent);
            }
            K::IndexExpression => {
                // An atomic suffix stays attached. A complex index may use its
                // own internal breaks, retaining the postfix chain's tokens.
                pending.push(Step::WrapIndex(node.clone(), indent));
                push_in_order(pending, node.children(), indent);
            }
            K::Unary | K::Field => push_in_order(pending, node.children(), indent),
            _ => {}
        }
    }

    fn collect_binary_chain(
        &self,
        node: &SyntaxNode,
        level: u8,
        operands: &mut Vec<SyntaxNode>,
        operators: &mut Vec<usize>,
    ) {
        if matches!(node.kind(), K::Binary | K::Power) && precedence(node) == level {
            let mut children = node.children();
            let left = children.next().expect("clean binary has left operand");
            let right = children.next().expect("clean binary has right operand");
            self.collect_binary_chain(&left, level, operands, operators);
            let operator = self.atoms_in(binary_operator(node).text_range()).start;
            operators.push(operator);
            self.collect_binary_chain(&right, level, operands, operators);
        } else {
            operands.push(node.clone());
        }
    }

    fn wrap_call(&mut self, node: &SyntaxNode, indent: usize, pending: &mut Vec<Step>) {
        let span = self.atom_range(node);
        let close = *self.significant(span.clone()).last().unwrap();
        for argument in node.children() {
            self.set_line(self.atom_range(&argument).start, indent + 2);
        }
        self.set_line(close, indent);
        push_in_order(pending, node.children(), indent + 2);
    }

    fn wrap_comprehension(&mut self, node: &SyntaxNode, indent: usize, pending: &mut Vec<Step>) {
        let span = self.atom_range(node);
        let expressions: Vec<_> = node.children().collect();
        let domain = &expressions[0];
        let body = &expressions[1];
        let body_span = self.atom_range(body);
        let sig = self.significant(span.clone());
        self.set_line(body_span.start, indent + 2);
        self.set_line(*sig.last().unwrap(), indent);
        let head_end = body_span.start;
        pending.push(Step::Format(body.clone(), indent + 2));
        if self.overflows(span.start..head_end) {
            self.set_line(sig[2], indent + 2);
            pending.push(Step::Format(domain.clone(), indent + 2));
        }
    }

    fn wrap_delimited(&mut self, node: &SyntaxNode, indent: usize, pending: &mut Vec<Step>) {
        let span = self.atom_range(node);
        let sig = self.significant(span.clone());
        if sig.len() < 3
            || (node.kind() == K::Dimension
                && node
                    .children()
                    .all(|child| matches!(child.kind(), K::Literal | K::Name | K::HostConstant)))
        {
            return;
        }
        self.set_line(sig[1], indent + 2);
        self.set_line(*sig.last().unwrap(), indent);
        push_in_order(pending, node.children(), indent + 2);
    }

    pub fn move_expression(&mut self, node: &SyntaxNode, indent: usize) {
        let span = self.atom_range(node);
        if !self.overflows(span.clone())
            || span.is_empty()
            || self.gaps[span.start + 1..span.end]
                .iter()
                .any(|gap| gap.line)
        {
            return;
        }
        let width: usize = span
            .clone()
            .map(|i| {
                self.atoms[i].text.chars().count()
                    + usize::from(i > span.start && self.gaps[i].space)
            })
            .sum();
        if indent + 2 + width + self.suffix(span.end) <= self.width {
            self.set_line(span.start, indent + 2);
        }
    }
}

fn precedence(node: &SyntaxNode) -> u8 {
    if node.kind() == K::Power {
        return 7;
    }
    let operator = binary_operator(node);
    match operator.text() {
        "||" => 1,
        "&&" => 2,
        "==" | "!=" => 3,
        "<" | ">" | "<=" | ">=" => 4,
        "+" | "-" => 5,
        "*" | "/" | "%" => 6,
        _ => 7,
    }
}

fn is_atomic(node: &SyntaxNode) -> bool {
    matches!(node.kind(), K::Literal | K::Name | K::HostConstant)
        || (node.kind() == K::Unary && node.children().all(|child| is_atomic(&child)))
}

fn binary_operator(node: &SyntaxNode) -> crate::SyntaxToken {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| token.kind() == K::Punct)
        .expect("clean binary has operator")
}
