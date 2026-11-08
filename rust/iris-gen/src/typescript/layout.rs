//! The layouts that oxfmt gives a statement with a call.
//!
//! oxfmt puts a call on one line if it fits. Otherwise it breaks after the
//! opening parenthesis, puts each argument on a line of its own two columns
//! in, each with a comma after it, and closes the call on a line of its own.
//! A last argument that is an object, an array or an arrow function with a
//! block body stays on the call's line instead, which is how a block's body
//! follows its opener. A chain of more than two calls, such as an `if` with
//! its `else`, puts each call after the first on a line of its own. A layout
//! that would split an argument inside, or that oxfmt would not choose, is
//! never written: the argument goes into a temporary instead, and the
//! statement is laid out again.
use super::expr::{Form, Parts, Value};
use super::{Line, Renderer, TAB, fits};

/// The block of a call: the arrow function that takes the scope `parameter`
/// and holds `body`.
#[derive(Debug)]
pub(super) struct Block {
    pub(super) parameter: String,
    pub(super) body: Vec<Line>,
}

/// One call of a statement: `.method(arguments)`, with the block as the
/// last argument if there is one.
#[derive(Debug)]
pub(super) struct Link {
    pub(super) method: &'static str,
    pub(super) arguments: Vec<Value>,
    pub(super) block: Option<Block>,
}

impl Link {
    /// The arguments' text, with the head of the block's arrow function, or
    /// all of it for an empty block.
    fn opener(&self) -> String {
        let mut texts: Vec<&str> = self
            .arguments
            .iter()
            .map(|value| value.text.as_str())
            .collect();
        let arrow = match &self.block {
            Some(block) if block.body.is_empty() => Some("() => {}".to_owned()),
            Some(block) => Some(format!("({}) => {{", block.parameter)),
            None => None,
        };
        if let Some(arrow) = &arrow {
            texts.push(arrow);
        }
        format!(".{}({}", self.method, texts.join(", "))
    }

    /// Whether the block has statements, so the link's text stops at `{`.
    fn opens(&self) -> bool {
        self.block
            .as_ref()
            .is_some_and(|block| !block.body.is_empty())
    }
}

/// The lines of the chain `receiver` and `links` at `column`, with the link
/// that each line belongs to: hugged when the chain has at most two calls,
/// and with each call after the first on a line of its own otherwise.
fn chain_lines(receiver: &str, links: &[Link], column: usize) -> Vec<(Line, usize)> {
    let expanded = links.len() > 2;
    let mut lines = Vec::new();
    // The text of the line being written, with the link it belongs to.
    let mut pending = (receiver.to_owned(), 0);
    let mut pending_column = column;
    for (index, link) in links.iter().enumerate() {
        let link_column = if expanded && index > 0 {
            column + TAB
        } else {
            column
        };
        if expanded && index > 0 {
            lines.push((Line::new(pending_column, pending.0), pending.1));
            pending = (String::new(), index);
            pending_column = link_column;
        }
        let text = format!("{}{}", pending.0, link.opener());
        if link.opens() {
            lines.push((Line::new(pending_column, text), index));
            let block = link.block.as_ref().expect("an opening link has a block");
            lines.extend(block.body.iter().map(|line| (line.clone(), index)));
            pending = ("})".to_owned(), index);
            pending_column = link_column;
        } else {
            pending = (format!("{text})"), index);
        }
    }
    lines.push((
        Line::new(pending_column, format!("{};", pending.0)),
        pending.1,
    ));
    lines
}

/// The lines of `callee(texts)tail` at `column`, as oxfmt lays them out,
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
    let mut lines = vec![Line::new(column, opener)];
    for text in texts {
        let text = format!("{text},");
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

/// The lines of `first`, `open`, each of `items` with a comma on a line of
/// its own, then `close` and `tail`. None if some line is too wide.
fn bracket_lines(
    first: &str,
    open: &str,
    items: &[String],
    close: &str,
    tail: &str,
    column: usize,
) -> Option<Vec<Line>> {
    let inner = column + TAB;
    let mut lines = vec![Line::new(column, format!("{first}{open}"))];
    for item in items {
        let item = format!("{item},");
        if !fits(inner, &item) {
            return None;
        }
        lines.push(Line::new(inner, item));
    }
    lines.push(Line::new(column, format!("{close}{tail}")));
    Some(lines)
}

impl Renderer<'_> {
    /// `receiver.method(arguments);` without a block, as oxfmt lays it out,
    /// with arguments in temporaries until a layout fits that splits none of
    /// them.
    pub(super) fn call_statement(&mut self, method: &str, mut arguments: Vec<Value>) {
        let column = self.column;
        let callee = format!("{}.{method}", self.receiver());
        loop {
            let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
            let one_line = format!("{callee}({});", texts.join(", "));
            if fits(column, &one_line) {
                self.line(one_line);
                return;
            }
            // oxfmt keeps a last object or array on the call's line, so only
            // a call without one breaks between its arguments.
            let hugs = arguments.last().is_some_and(|value| value.hugs);
            if !hugs && let Some(lines) = call_lines("", &callee, &texts, ";", column) {
                self.lines.extend(lines);
                return;
            }
            if !self.shrink_widest(&mut arguments, column + TAB) {
                self.line(one_line);
                return;
            }
        }
    }

    /// The chain `receiver` and `links` as one statement, with arguments in
    /// temporaries until every line fits.
    pub(super) fn chain_statement(&mut self, receiver: &str, mut links: Vec<Link>) {
        let column = self.column;
        loop {
            let lines = chain_lines(receiver, &links, column);
            let wide = lines
                .iter()
                .find(|(line, _)| !fits(line.column, &line.text))
                .map(|&(_, link)| link);
            let shrunk = match wide {
                Some(link) => self.shrink_widest(&mut links[link].arguments, column + TAB),
                None => false,
            };
            if !shrunk {
                self.lines.extend(lines.into_iter().map(|(line, _)| line));
                return;
            }
        }
    }

    /// `const name = value;`, broken as oxfmt breaks it when it is too wide:
    /// after `=` for a string, and between the arguments of a call, the
    /// elements of an array or the properties of an object. An object or an
    /// array keeps its literal types, such as `true` for `skipOther`, only
    /// with a constant assertion.
    pub(super) fn let_statement(&mut self, name: &str, value: &Value) {
        let column = self.column;
        let suffix = if value.hugs { " as const;" } else { ";" };
        let head = format!("const {name} = ");
        let one_line = format!("{head}{}{suffix}", value.text);
        if fits(column, &one_line) {
            self.line(one_line);
            return;
        }
        let broken = match &value.parts {
            Some(Parts::Call(callee, arguments)) => {
                call_lines(&head, callee, arguments, suffix, column)
            }
            Some(Parts::Array(items)) => bracket_lines(&head, "[", items, "]", suffix, column),
            Some(Parts::Object(items)) => bracket_lines(&head, "{", items, "}", suffix, column),
            None => None,
        };
        if let Some(lines) = broken {
            self.lines.extend(lines);
        } else if matches!(value.form, Form::Str) {
            self.line(format!("const {name} ="));
            self.line_at(column + TAB, format!("{}{suffix}", value.text));
        } else {
            self.line(one_line);
        }
    }

    /// Shorten the widest argument that does not fit a line at `column`, or
    /// else the widest that a temporary can shorten, an object or an array
    /// last. Returns whether one changed.
    pub(super) fn shrink_widest(&mut self, arguments: &mut [Value], column: usize) -> bool {
        let mut order: Vec<usize> = (0..arguments.len())
            .filter(|&index| !arguments[index].is_fixed())
            .collect();
        order.sort_by_key(|&index| {
            let text = &arguments[index].text;
            (
                arguments[index].hugs,
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
