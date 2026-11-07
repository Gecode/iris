//! The layouts that clang-format gives a statement with a call.
//!
//! clang-format chooses the layout with the fewest lines among those that
//! fit, and between layouts with as many lines, the first of [`Shape`]'s
//! order. A layout that would split an argument inside is never written: the
//! argument goes into a temporary instead, and the statement is laid out
//! again.
use super::expr::{Bracket, Form, Value, joined_lambda};
use super::{Arg, CONTINUATION, Line, Renderer, TAB, can_shrink, fits, is_splittable, pack};

/// A call's layouts, in clang-format's order of preference between layouts
/// with as many lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    /// Everything on one line.
    OneLine,
    /// `auto x =`, then the call on one line.
    AfterAssignment,
    /// The trailing lambda's body on lines of its own.
    Expanded,
    /// `auto x =`, then the call with its arguments aligned.
    AssignedAligned,
    /// The arguments packed and aligned after the opening bracket.
    Aligned,
    /// A break after the opening bracket, then every argument on one line.
    Continued,
    /// A break after the opening bracket, then the arguments packed.
    Packed,
}

impl Shape {
    /// Whether clang-format might split an argument inside rather than take
    /// this layout.
    fn breaks_after_bracket(self) -> bool {
        matches!(self, Self::Continued | Self::Packed)
    }
}

/// A lambda that ends a call's arguments.
pub(super) struct Lambda {
    /// `[&](auto &s1)`.
    pub(super) introducer: String,
    /// The lambda on one line, if clang-format may join it.
    pub(super) joined: Option<String>,
    /// The body's lines, one indentation level in from the statement.
    pub(super) body: Vec<Line>,
}

impl Lambda {
    /// A lambda with the statements `body`, joined if it has one line.
    pub(super) fn new(parameter: &str, body: Vec<Line>) -> Self {
        let joined = match body.as_slice() {
            // An empty body uses no parameter, so it has no name.
            [] => Some(format!(
                "[&]({}) {{}}",
                parameter.trim_end_matches(|c: char| c != '&')
            )),
            [only] if !only.is_blank() => Some(joined_lambda(parameter, &only.text)),
            _ => None,
        };
        Self {
            introducer: format!("[&]({parameter})"),
            joined,
            body,
        }
    }
}

/// `head callee(arguments, lambda)suffix`, where `head` is `auto x = ` or
/// empty.
struct Statement<'s> {
    head: &'s str,
    callee: &'s str,
    bracket: Bracket,
    lambda: Option<&'s Lambda>,
    suffix: &'s str,
    column: usize,
}

impl Statement<'_> {
    fn open(&self) -> String {
        format!("{}{}", self.callee, self.bracket.open())
    }

    fn tail(&self) -> String {
        format!("{}{}", self.bracket.close(), self.suffix)
    }

    /// The best layout, if any fits.
    fn layout(&self, arguments: &[Arg]) -> Option<(Shape, Vec<Line>)> {
        let mut texts: Vec<String> = arguments.iter().map(Arg::text).collect();
        let joined = match self.lambda {
            Some(lambda) => lambda
                .joined
                .clone()
                .inspect(|text| texts.push(text.clone())),
            None => Some(String::new()),
        };
        let mut candidates: Vec<(Shape, Option<Vec<Line>>)> = Vec::new();
        if joined.is_some() {
            candidates.extend([
                (Shape::OneLine, self.one_line(&texts)),
                (Shape::AfterAssignment, self.after_assignment(&texts)),
                (Shape::AssignedAligned, self.assigned_aligned(&texts)),
                (Shape::Aligned, self.aligned(&texts)),
                (Shape::Continued, self.continued(&texts)),
                (Shape::Packed, self.packed(&texts)),
            ]);
        }
        if let Some(lambda) = self.lambda {
            let count = texts.len() - usize::from(joined.is_some());
            candidates.push((Shape::Expanded, self.expanded(&texts[..count], lambda)));
        }
        candidates
            .into_iter()
            .filter_map(|(shape, lines)| Some((shape, lines?)))
            .min_by_key(|(shape, lines)| (lines.len(), *shape))
    }

    fn line(&self, column: usize, text: String) -> Line {
        Line { column, text }
    }

    fn one_line(&self, texts: &[String]) -> Option<Vec<Line>> {
        let text = format!(
            "{}{}{}{}",
            self.head,
            self.open(),
            texts.join(", "),
            self.tail()
        );
        fits(self.column, &text).then(|| vec![self.line(self.column, text)])
    }

    fn after_assignment(&self, texts: &[String]) -> Option<Vec<Line>> {
        if self.head.is_empty() {
            return None;
        }
        let text = format!("{}{}{}", self.open(), texts.join(", "), self.tail());
        let column = self.column + CONTINUATION;
        fits(column, &text).then(|| {
            vec![
                self.line(self.column, self.head.trim_end().to_owned()),
                self.line(column, text),
            ]
        })
    }

    /// The call at `column` after `first`, with its arguments aligned.
    fn aligned_at(&self, first: String, column: usize, texts: &[String]) -> Option<Vec<Line>> {
        let open = self.open();
        let align = column + first.len() + open.len();
        let packed = pack(align, align, texts, &self.tail())?;
        let mut packed = packed.into_iter();
        let mut lines = vec![self.line(column, format!("{first}{open}{}", packed.next()?))];
        lines.extend(packed.map(|text| self.line(align, text)));
        Some(lines)
    }

    fn aligned(&self, texts: &[String]) -> Option<Vec<Line>> {
        self.aligned_at(self.head.to_owned(), self.column, texts)
    }

    fn assigned_aligned(&self, texts: &[String]) -> Option<Vec<Line>> {
        if self.head.is_empty() {
            return None;
        }
        let mut lines = vec![self.line(self.column, self.head.trim_end().to_owned())];
        lines.extend(self.aligned_at(String::new(), self.column + CONTINUATION, texts)?);
        Some(lines)
    }

    /// The opening line of the layouts that break after the bracket.
    fn opener(&self) -> Option<Line> {
        let text = format!("{}{}", self.head, self.open());
        fits(self.column, &text).then(|| self.line(self.column, text))
    }

    fn continued(&self, texts: &[String]) -> Option<Vec<Line>> {
        let column = self.column + CONTINUATION;
        let text = format!("{}{}", texts.join(", "), self.tail());
        (!texts.is_empty() && fits(column, &text))
            .then(|| {
                self.opener()
                    .map(|opener| vec![opener, self.line(column, text)])
            })
            .flatten()
    }

    fn packed(&self, texts: &[String]) -> Option<Vec<Line>> {
        let column = self.column + CONTINUATION;
        let packed = pack(column, column, texts, &self.tail())?;
        let mut lines = vec![self.opener()?];
        lines.extend(packed.into_iter().map(|text| self.line(column, text)));
        Some(lines)
    }

    fn expanded(&self, texts: &[String], lambda: &Lambda) -> Option<Vec<Line>> {
        let mut parts = texts.to_vec();
        parts.push(format!("{} {{", lambda.introducer));
        let open = format!("{}{}{}", self.head, self.open(), parts.join(", "));
        if lambda.body.is_empty() || !fits(self.column, &open) {
            return None;
        }
        let mut lines = vec![self.line(self.column, open)];
        lines.extend(lambda.body.iter().cloned());
        let close = format!("}}{}", self.tail());
        lines.push(self.line(self.column, close));
        Some(lines)
    }
}

impl Renderer<'_> {
    /// `head callee(arguments, lambda)suffix` as clang-format lays it out,
    /// with arguments in temporaries until a layout fits that splits none of
    /// them.
    pub(super) fn statement_call(
        &mut self,
        head: &str,
        callee: &str,
        bracket: Bracket,
        mut arguments: Vec<Arg>,
        lambda: Option<Lambda>,
        suffix: &str,
    ) {
        let statement = Statement {
            head,
            callee,
            bracket,
            lambda: lambda.as_ref(),
            suffix,
            column: self.column(),
        };
        loop {
            let layout = statement.layout(&arguments);
            if let Some((shape, _)) = &layout
                && shape.breaks_after_bracket()
            {
                let splittable = arguments
                    .iter()
                    .enumerate()
                    .filter(|(_, argument)| {
                        is_splittable(&argument.value) && can_shrink(&argument.value)
                    })
                    .max_by_key(|(_, argument)| argument.text().len())
                    .map(|(index, _)| index);
                if let Some(index) = splittable
                    && self.shrink(&mut arguments[index].value)
                {
                    continue;
                }
            }
            if let Some((_, lines)) = layout {
                self.lines.extend(lines);
                return;
            }
            if !self.shrink_widest(&mut arguments, statement.column + CONTINUATION) {
                break;
            }
        }
        // Nothing fits: clang-format will disagree, and the round trip says so.
        let mut texts: Vec<String> = arguments.iter().map(Arg::text).collect();
        let column = statement.column;
        match &lambda {
            Some(lambda) => {
                texts.push(format!("{} {{", lambda.introducer));
                let open = format!("{head}{}{}", statement.open(), texts.join(", "));
                self.line_at(column, open);
                self.lines.extend(lambda.body.iter().cloned());
                self.line_at(column, format!("}}{}", statement.tail()));
            }
            None => {
                let text = format!(
                    "{head}{}{}{}",
                    statement.open(),
                    texts.join(", "),
                    statement.tail()
                );
                self.line_at(column, text);
            }
        }
    }

    /// Shorten the widest argument that does not fit a line at `column`, or
    /// else the widest that a temporary can shorten. Returns whether one
    /// changed.
    fn shrink_widest(&mut self, arguments: &mut [Arg], column: usize) -> bool {
        let mut order: Vec<usize> = (0..arguments.len())
            .filter(|&index| can_shrink(&arguments[index].value))
            .collect();
        order.sort_by_key(|&index| {
            let text = arguments[index].text();
            (
                fits(column, &format!("{text},")),
                std::cmp::Reverse(text.len()),
            )
        });
        order
            .into_iter()
            .any(|index| self.shrink(&mut arguments[index].value))
    }

    /// `callee(arguments)suffix` as a statement.
    pub(super) fn call(
        &mut self,
        callee: &str,
        bracket: Bracket,
        arguments: Vec<Arg>,
        suffix: &str,
    ) {
        self.statement_call("", callee, bracket, arguments, None, suffix);
    }

    /// `head value;`, where `head` is `auto x = `.
    pub(super) fn let_statement(&mut self, head: &str, value: Value) {
        match value.form {
            Form::Call {
                callee,
                bracket,
                arguments,
                ..
            } => self.statement_call(head, &callee, bracket, arguments, None, ";"),
            Form::Comprehension { .. } => self.expanded(head, value, Vec::new()),
            _ => {
                let column = self.column();
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

    /// `head kind(domain, [&](parameter) { body return; })`, with `body`'s
    /// statements before the `return`.
    pub(super) fn expanded(&mut self, head: &str, value: Value, mut body: Vec<Line>) {
        let Form::Comprehension {
            callee,
            domain,
            parameter,
            body: result,
        } = value.form
        else {
            unreachable!("only a comprehension has a lambda");
        };
        body.push(Line {
            column: self.column() + TAB,
            text: format!("return {result};"),
        });
        let lambda = Lambda::new(&parameter, body);
        let arguments = vec![Arg::new(*domain)];
        self.statement_call(head, callee, Bracket::Paren, arguments, Some(lambda), ";");
    }

    /// `head(arguments, [&](parameter) { body });`, where `head` may start
    /// with `auto x = `.
    pub(super) fn block_call(
        &mut self,
        head: &str,
        arguments: Vec<Arg>,
        parameter: &str,
        body: Vec<Line>,
    ) {
        let (assignment, callee) = match head.split_once(" = ") {
            Some((assignment, callee)) => (format!("{assignment} = "), callee),
            None => (String::new(), head),
        };
        let lambda = Lambda::new(parameter, body);
        self.statement_call(
            &assignment,
            callee,
            Bracket::Paren,
            arguments,
            Some(lambda),
            ";",
        );
    }
}
