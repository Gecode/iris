//! The layouts that ktfmt's kotlinlang style gives a statement with a call.
//!
//! ktfmt puts a call on one line if it fits. Otherwise it breaks after the
//! opening parenthesis, puts each argument on a line of its own four columns
//! in, with a comma after each when there are several, and closes the call on
//! a line of its own. An assignment breaks after `=` before it breaks the
//! call. A block's lambda follows the closing parenthesis. A layout that would
//! split an argument inside is never written: the argument goes into a local
//! value instead, and the statement is laid out again.
use super::expr::{Form, Value};
use super::{Line, Renderer, TAB, fits};

/// The lines of `callee(texts)tail` at `column`, as ktfmt lays them out,
/// with `first` before the call on its first line. None if some line is too
/// wide.
pub(super) fn call_lines(
    first: &str,
    callee: &str,
    texts: &[String],
    tail: &str,
    column: usize,
) -> Option<Vec<Line>> {
    let one_line = format!("{first}{callee}({}){tail}", texts.join(", "));
    if fits(column, &one_line) {
        return Some(vec![Line::new(column, one_line)]);
    }
    let opener = format!("{first}{callee}(");
    if texts.is_empty() || !fits(column, &opener) {
        return None;
    }
    let inner = column + TAB;
    let comma = if texts.len() > 1 { "," } else { "" };
    let mut lines = vec![Line::new(column, opener)];
    for text in texts {
        let text = format!("{text}{comma}");
        if !fits(inner, &text) {
            return None;
        }
        lines.push(Line::new(inner, text));
    }
    let close = format!("){tail}");
    if !fits(column, &close) {
        return None;
    }
    lines.push(Line::new(column, close));
    Some(lines)
}

/// The lines of an assignment `head value` at `column`, where `head` is
/// `val e1 = ` or a property's declaration and `value` is the call
/// `callee(texts)`.
pub(super) fn assignment_lines(
    head: &str,
    callee: &str,
    texts: &[String],
    column: usize,
) -> Option<Vec<Line>> {
    let one_line = format!("{head}{callee}({})", texts.join(", "));
    if fits(column, &one_line) {
        return Some(vec![Line::new(column, one_line)]);
    }
    let mut lines = vec![Line::new(column, head.trim_end().to_owned())];
    lines.extend(call_lines("", callee, texts, "", column + TAB)?);
    Some(lines)
}

/// `callee(texts) { body }` at `column`: the opener's layout, then the body
/// and the closing brace, or `{}` on the opener's line for an empty body.
fn block_lines(callee: &str, texts: &[String], body: &[Line], column: usize) -> Option<Vec<Line>> {
    let brace = if body.is_empty() { " {}" } else { " {" };
    let mut lines = if texts.is_empty() {
        let opener = format!("{callee}{brace}");
        if !fits(column, &opener) {
            return None;
        }
        vec![Line::new(column, opener)]
    } else {
        call_lines("", callee, texts, brace, column)?
    };
    if !body.is_empty() {
        lines.extend(body.iter().cloned());
        lines.push(Line::new(column, "}".to_owned()));
    }
    Some(lines)
}

impl Renderer<'_> {
    /// `callee(arguments)` as ktfmt lays it out, with `body` as the block of
    /// a trailing lambda if it is set, and arguments in local values until a
    /// layout fits that splits none of them.
    pub(super) fn call_statement(
        &mut self,
        callee: &str,
        mut arguments: Vec<Value>,
        body: Option<Vec<Line>>,
    ) {
        let column = self.column;
        loop {
            let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
            let layout = match &body {
                None => call_lines("", callee, &texts, "", column),
                Some(body) => block_lines(callee, &texts, body, column),
            };
            if let Some(lines) = layout {
                self.lines.extend(lines);
                return;
            }
            if !self.shrink_widest(&mut arguments, column + TAB) {
                break;
            }
        }
        // Nothing fits: ktfmt will disagree, and the round trip says so.
        let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
        let call = if texts.is_empty() && body.is_some() {
            callee.to_owned()
        } else {
            format!("{callee}({})", texts.join(", "))
        };
        match body {
            None => self.line(call),
            Some(body) if body.is_empty() => self.line(format!("{call} {{}}")),
            Some(body) => {
                self.line(format!("{call} {{"));
                self.lines.extend(body);
                self.line("}".to_owned());
            }
        }
    }

    /// `val name = value`.
    pub(super) fn let_statement(&mut self, name: &str, value: Value) {
        let head = format!("val {name} = ");
        let column = self.column;
        match value.form {
            Form::Call {
                callee,
                mut arguments,
            } => loop {
                let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
                if let Some(lines) = assignment_lines(&head, &callee, &texts, column) {
                    self.lines.extend(lines);
                    return;
                }
                let inner = column + 2 * TAB;
                if !self.shrink_widest(&mut arguments, inner) {
                    self.line(format!("{head}{callee}({})", texts.join(", ")));
                    return;
                }
            },
            _ => {
                let one_line = format!("{head}{}", value.text);
                if fits(column, &one_line) {
                    self.line(one_line);
                } else {
                    self.line(head.trim_end().to_owned());
                    self.line_at(column + TAB, value.text);
                }
            }
        }
    }

    /// Shorten the widest argument that does not fit a line at `column`, or
    /// else the widest that a local value can shorten. Returns whether one
    /// changed.
    fn shrink_widest(&mut self, arguments: &mut [Value], column: usize) -> bool {
        let mut order: Vec<usize> = (0..arguments.len())
            .filter(|&index| !arguments[index].is_fixed())
            .collect();
        order.sort_by_key(|&index| {
            let text = &arguments[index].text;
            (
                fits(column, &format!("{text},")),
                std::cmp::Reverse(text.len()),
            )
        });
        match order.first() {
            Some(&index) => {
                self.hoist(&mut arguments[index]);
                true
            }
            None => false,
        }
    }
}
