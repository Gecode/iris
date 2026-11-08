//! The layouts that google-java-format gives a statement with a call.
//!
//! google-java-format puts a call on one line if it fits. Otherwise it breaks
//! after the opening parenthesis and puts the arguments on one line, four
//! columns in, or, if that is too wide, fills lines with them when every one
//! is at most [`SHORT_ARGUMENT`] columns wide and else puts each on a line of
//! its own. An assignment breaks after `=` before it breaks the call. A block
//! lambda that is the only argument stays on the call's line, and one after
//! other arguments takes a line of its own like them. A layout that would
//! split an argument inside is never written: the argument goes into a
//! temporary instead, and the statement is laid out again.
use super::expr::Value;
use super::{CONTINUATION, LINE_WIDTH, Line, Renderer, fits};

/// The widest argument that google-java-format fills lines with.
const SHORT_ARGUMENT: usize = 9;

/// The lines of `callee(texts)tail` at `column`, as google-java-format lays
/// them out, with `first` before the call on its first line. None if some
/// line is too wide.
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
    let inner = column + CONTINUATION;
    let close = format!("){tail}");
    let mut lines = vec![Line::new(column, opener)];
    let together = format!("{}{close}", texts.join(", "));
    if fits(inner, &together) {
        lines.push(Line::new(inner, together));
        return Some(lines);
    }
    if texts.iter().all(|text| text.len() <= SHORT_ARGUMENT) {
        lines.extend(
            fill(inner, texts, &close)
                .into_iter()
                .map(|text| Line::new(inner, text)),
        );
        return Some(lines);
    }
    for (index, text) in texts.iter().enumerate() {
        let end = if index + 1 == texts.len() {
            close.as_str()
        } else {
            ","
        };
        let text = format!("{text}{end}");
        if !fits(inner, &text) {
            return None;
        }
        lines.push(Line::new(inner, text));
    }
    Some(lines)
}

/// `texts` separated by `, ` and packed greedily at `column`, the last
/// followed by `tail`.
fn fill(column: usize, texts: &[String], tail: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for (index, text) in texts.iter().enumerate() {
        let end = if index + 1 == texts.len() { tail } else { "," };
        let own = format!("{text}{end}");
        if line.is_empty() {
            line = own;
        } else if column + line.len() + 1 + own.len() <= LINE_WIDTH {
            line.push(' ');
            line.push_str(&own);
        } else {
            lines.push(std::mem::take(&mut line));
            line = own;
        }
    }
    lines.push(line);
    lines
}

/// The lines of an assignment `head value;` at `column`, where `head` is
/// `var e1 = ` or a field's declaration and `value` is the call
/// `callee(texts)`.
pub(super) fn assignment_lines(
    head: &str,
    callee: &str,
    texts: &[String],
    column: usize,
) -> Option<Vec<Line>> {
    let one_line = format!("{head}{callee}({});", texts.join(", "));
    if fits(column, &one_line) {
        return Some(vec![Line::new(column, one_line)]);
    }
    let mut lines = vec![Line::new(column, head.trim_end().to_owned())];
    lines.extend(call_lines("", callee, texts, ";", column + CONTINUATION)?);
    Some(lines)
}

impl Renderer<'_> {
    /// `callee(arguments);` as google-java-format lays it out, with `body`
    /// as a block lambda after the arguments if it is set, and arguments in
    /// temporaries until a layout fits that splits none of them. `body` is
    /// indented for the lambda's place: two columns in if it is the only
    /// argument, and six if it is not.
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
                None => call_lines("", callee, &texts, ";", column),
                Some(body) => block_lines(callee, &texts, body, column),
            };
            if let Some(lines) = layout {
                self.lines.extend(lines);
                return;
            }
            if !self.shrink_widest(&mut arguments, column + CONTINUATION) {
                break;
            }
        }
        // Nothing fits: google-java-format will disagree, and the round trip
        // says so.
        let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
        match body {
            None => self.line(format!("{callee}({});", texts.join(", "))),
            Some(body) => {
                let mut parts = texts;
                parts.push("() -> {".to_owned());
                self.line(format!("{callee}({}", parts.join(", ")));
                self.lines.extend(body);
                self.line("});".to_owned());
            }
        }
    }

    /// `var name = value;`.
    pub(super) fn let_statement(&mut self, name: &str, value: Value) {
        let head = format!("var {name} = ");
        let column = self.column;
        match value.form {
            super::expr::Form::Call {
                callee,
                mut arguments,
            } => loop {
                let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
                if let Some(lines) = assignment_lines(&head, &callee, &texts, column) {
                    self.lines.extend(lines);
                    return;
                }
                let inner = column + 2 * CONTINUATION;
                if !self.shrink_widest(&mut arguments, inner) {
                    self.line(format!("{head}{callee}({});", texts.join(", ")));
                    return;
                }
            },
            _ => {
                let one_line = format!("{head}{};", value.text);
                if fits(column, &one_line) || !fits(column + CONTINUATION, &value.text) {
                    self.line(one_line);
                } else {
                    self.line(head.trim_end().to_owned());
                    self.line_at(column + CONTINUATION, format!("{};", value.text));
                }
            }
        }
    }

    /// Shorten the widest argument that does not fit a line at `column`, or
    /// else the widest that a temporary can shorten. Returns whether one
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

/// `callee(texts, () -> { body });` at `column`.
fn block_lines(callee: &str, texts: &[String], body: &[Line], column: usize) -> Option<Vec<Line>> {
    if texts.is_empty() {
        let opener = format!("{callee}(() -> {{");
        if !fits(column, &opener) {
            return None;
        }
        let mut lines = vec![Line::new(column, opener)];
        lines.extend(body.iter().cloned());
        lines.push(Line::new(column, "});".to_owned()));
        return Some(lines);
    }
    let inner = column + CONTINUATION;
    let mut lines = vec![Line::new(column, format!("{callee}("))];
    for text in texts {
        let text = format!("{text},");
        if !fits(inner, &text) {
            return None;
        }
        lines.push(Line::new(inner, text));
    }
    lines.push(Line::new(inner, "() -> {".to_owned()));
    lines.extend(body.iter().cloned());
    lines.push(Line::new(inner, "});".to_owned()));
    Some(lines)
}
