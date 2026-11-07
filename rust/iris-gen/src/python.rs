//! The Python backend: a program that calls `gecode_iris.build`.
//!
//! The output is what `ruff format` would write, without `ruff`: a statement
//! that is too wide goes into the layouts that ruff gives calls and tuples, and
//! an argument too wide for those goes into a temporary first. Python allows 20
//! nested blocks in one function, so a block deeper than [`MAX_INDENT`] becomes
//! a helper function at the top of `builder`. The events are walked with a loop
//! and a stack of open blocks, so no recursion depends on the spec.
mod expr;
mod names;
mod types;

use std::collections::HashMap;

use crate::Backend;
use crate::program::{
    BaseType, Event, NameKind, Program, RecordId, RepeatCount, RepeatNames, SectionClause, Target,
    Type,
};
use expr::{Class, Form, Value};
use names::{Identifiers, Role};

/// `ruff format`'s `line-length`.
const LINE_WIDTH: usize = 88;
/// One indentation level.
const TAB: usize = 4;
/// What `eN = ` takes from a line.
const LET_OVERHEAD: usize = "e0000 = ".len();
/// The narrowest budget for an expression, however deep the block.
const NARROWEST_EXPRESSION: usize = 24;
/// The deepest indentation of a block's statements in one function. Deeper
/// blocks become helper functions, well inside Python's limit of 20 nested
/// blocks and 100 indentation levels.
const MAX_INDENT: usize = 10;
/// The most `with` items around a block's statements in one function. Deeper
/// blocks become helper functions, inside Python's limit of 20 nested blocks:
/// a `with` of several items counts each of them.
const MAX_NESTING: usize = 16;
/// The indentation of a helper function's statements.
const HELPER_BODY_INDENT: usize = 2;

/// A string as a Python literal, in ASCII, as `ruff format` writes it.
///
/// Ruff measures a line in display columns. Without a table of them, only
/// ASCII has a length that is its width, so everything else is escaped. Ruff
/// prefers double quotes, unless single quotes need fewer escapes.
fn quoted(text: &str) -> String {
    let doubles = text.matches('"').count();
    let singles = text.matches('\'').count();
    let quote = if doubles > singles { '\'' } else { '"' };
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push(quote);
    for character in text.chars() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            _ if character == quote => {
                literal.push('\\');
                literal.push(character);
            }
            ' '..='~' => literal.push(character),
            _ => {
                let code = u32::from(character);
                let escape = if code < 0x100 {
                    format!("\\x{code:02x}")
                } else if code < 0x1_0000 {
                    format!("\\u{code:04x}")
                } else {
                    format!("\\U{code:08x}")
                };
                literal.push_str(&escape);
            }
        }
    }
    literal.push(quote);
    literal
}

pub(crate) struct PythonBackend;

impl Backend for PythonBackend {
    fn render(&self, program: &Program) -> String {
        Renderer::new(program).render()
    }
}

/// One line of output without its indentation; an empty one is a blank line.
#[derive(Debug, Clone)]
struct Line {
    indent: usize,
    text: String,
}

impl Line {
    fn blank() -> Self {
        Self {
            indent: 0,
            text: String::new(),
        }
    }

    fn is_blank(&self) -> bool {
        self.text.is_empty()
    }
}

/// An argument of a call, with its keyword if it has one.
#[derive(Debug, Clone)]
struct Arg {
    keyword: Option<&'static str>,
    value: Value,
}

impl Arg {
    fn new(value: Value) -> Self {
        Self {
            keyword: None,
            value,
        }
    }

    fn keyword(keyword: &'static str, value: Value) -> Self {
        Self {
            keyword: Some(keyword),
            value,
        }
    }

    fn text(&self) -> String {
        match self.keyword {
            Some(keyword) => format!("{keyword}={}", self.value.text),
            None => self.value.text.clone(),
        }
    }
}

/// A block whose statements go to a helper function.
#[derive(Debug)]
struct Outlined {
    /// The lines of the function that holds the block.
    outer: Vec<Line>,
    /// The indentation of the call in the function that holds the block.
    call_indent: usize,
    /// Where the helper goes among the others, so outer helpers come first.
    position: usize,
    function: String,
    parameter: String,
    /// The types of other helpers that the block uses, which the helper takes
    /// after its scope.
    types: Vec<TypeVariable>,
}

/// The variable that holds a defined type, with its annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeVariable {
    identifier: String,
    annotation: &'static str,
}

/// A type definition that a later type uses, while it is visible.
#[derive(Debug)]
struct VisibleType {
    name: String,
    variable: TypeVariable,
    /// The number of helper functions around the definition: 0 is `builder`.
    function: usize,
}

#[derive(Debug)]
struct OpenBlock {
    /// The scope or arms that the block's statements call.
    receiver: String,
    /// The number of repeats around the block, which decides its scope class.
    depth: usize,
    /// The indentation of the `with` statement.
    with_indent: usize,
    /// The number of `with` items around the block's statements in their
    /// function.
    nesting: usize,
    /// The number of lines before the block's first statement.
    start: usize,
    /// The number of visible types before the block.
    types: usize,
    outlined: Option<Outlined>,
}

/// An item of a `with` statement that is not written yet: ruff's default
/// rules want a `with` whose only statement is another `with` to be one
/// statement with several items.
#[derive(Debug)]
struct PendingWith {
    guard: Value,
    target: Option<String>,
    /// The event index of the block's `Close`.
    close: usize,
}

/// How `call` lays out a call, or the argument it moves out first.
enum Layout {
    OneLine,
    /// The arguments on one line of their own.
    Body,
    /// One argument to a line.
    PerLine,
    Hoist(usize),
}

/// The event index of each opener's `Close`.
fn matching_closes(events: &[Event]) -> Vec<usize> {
    let mut closes = vec![0; events.len()];
    let mut open = Vec::new();
    for (index, event) in events.iter().enumerate() {
        if event.opens_block() {
            open.push(index);
        } else if matches!(event, Event::Close) {
            let opener = open.pop().expect("every close has an opener");
            closes[opener] = index;
        }
    }
    closes
}

/// The annotation of a helper's scope parameter.
fn scope_annotation(depth: usize) -> String {
    if depth <= 3 {
        format!("ib.Scope{depth}[None]")
    } else {
        "ib.ScopeN[None]".to_owned()
    }
}

struct Renderer<'a> {
    program: &'a Program,
    value_names: Vec<String>,
    record_names: Vec<String>,
    variable_names: Vec<String>,
    identifiers: Identifiers,
    type_names: HashMap<String, String>,
    /// The definitions that later types use, while they are visible.
    visible_types: Vec<VisibleType>,
    lines: Vec<Line>,
    indent: usize,
    blocks: Vec<OpenBlock>,
    closes: Vec<usize>,
    temporaries: usize,
    functions: usize,
    /// Helper functions, each with its lines.
    helpers: Vec<Vec<Line>>,
    /// The items of the next `with` statement, written before the next line.
    pending: Vec<PendingWith>,
    /// The indentation of the pending `with` statement.
    pending_indent: usize,
}

impl<'a> Renderer<'a> {
    fn new(program: &'a Program) -> Self {
        let mut identifiers = Identifiers::default();
        let value_names = program
            .names
            .iter()
            .map(|name| identifiers.named(Role::Value, &name.text))
            .collect();
        let record_names = program
            .records
            .iter()
            .map(|record| identifiers.fresh(&record.each, "record"))
            .collect();
        let variable_names = program
            .variables
            .iter()
            .map(|variable| identifiers.named(Role::Variable, variable))
            .collect();
        Self {
            program,
            value_names,
            record_names,
            variable_names,
            identifiers,
            type_names: HashMap::new(),
            visible_types: Vec::new(),
            lines: Vec::new(),
            indent: 1,
            blocks: Vec::new(),
            closes: matching_closes(&program.events),
            temporaries: 0,
            functions: 0,
            helpers: Vec::new(),
            pending: Vec::new(),
            pending_indent: 1,
        }
    }

    fn render(mut self) -> String {
        let events = &self.program.events;
        let (first, arguments) = match events.first() {
            Some(Event::Format(name)) => (1, vec![Arg::new(string(name))]),
            _ => (0, Vec::new()),
        };
        let constructor = Value::call("ib.SpecBuilder", arguments, Class::Other);
        if first == events.len() {
            self.let_statement("return ", constructor);
            return self.finish(0);
        }
        self.let_statement("b = ", constructor);
        self.handles();
        let handles_end = self.lines.len();
        let mut index = first;
        while let Some(event) = self.program.events.get(index) {
            index = self.statement(event, index);
        }
        self.line("return b".to_owned());
        self.finish(handles_end)
    }

    /// Handles for every value name and record, made before their definitions
    /// because a reference may come first in the text.
    fn handles(&mut self) {
        let program = self.program;
        for (name, identifier) in program.names.iter().zip(self.value_names.clone()) {
            let head = format!("{identifier} = ");
            let value = match name.kind {
                NameKind::Forward => {
                    let arguments = [
                        string(&name.text),
                        Value::atom("ib.DynField".into(), Class::Other),
                    ];
                    Value::call_of("b.forward", &arguments, Class::Handle)
                }
                NameKind::Undefined => {
                    Value::call_of("b.undefined", &[string(&name.text)], Class::Handle)
                }
            };
            self.let_statement(&head, value);
        }
        for (record, identifier) in program.records.iter().zip(self.record_names.clone()) {
            let arguments = [string(&record.each), string(&record.list)];
            let value = Value::call_of("b.forward_record", &arguments, Class::Record);
            self.let_statement(&format!("{identifier} = "), value);
        }
    }

    fn finish(self, handles_end: usize) -> String {
        let mut lines = self.lines;
        let body = lines.split_off(handles_end);
        for helper in self.helpers {
            lines.push(Line::blank());
            lines.extend(helper);
            lines.push(Line::blank());
        }
        lines.extend(body);
        let mut text = String::from("import gecode_iris.build as ib\n\n\n");
        text.push_str("def builder() -> ib.SpecBuilder:\n");
        let mut previous_blank = true;
        for line in &lines {
            if line.is_blank() {
                if !previous_blank {
                    text.push('\n');
                }
                previous_blank = true;
                continue;
            }
            previous_blank = false;
            for _ in 0..line.indent * TAB {
                text.push(' ');
            }
            text.push_str(&line.text);
            text.push('\n');
        }
        text.push_str("\n\nif __name__ == \"__main__\":\n");
        text.push_str("    print(builder().source(), end=\"\")\n");
        text
    }

    fn line(&mut self, text: String) {
        self.flush();
        self.lines.push(Line {
            indent: self.indent,
            text,
        });
    }

    fn blank(&mut self) {
        self.flush();
        if !self.lines.last().is_some_and(Line::is_blank) {
            self.lines.push(Line::blank());
        }
    }

    fn temporary(&mut self) -> String {
        self.temporaries += 1;
        format!("e{}", self.temporaries)
    }

    fn next_function(&mut self) -> usize {
        self.functions += 1;
        self.functions
    }

    /// The widest expression the current block can hold on one line.
    fn budget(&self) -> usize {
        LINE_WIDTH
            .saturating_sub(self.indent * TAB + LET_OVERHEAD)
            .max(NARROWEST_EXPRESSION)
    }

    fn fits(&self, text: &str) -> bool {
        fits_at(self.indent, text)
    }

    /// Whether `text` fits one level in.
    fn fits_next_line(&self, text: &str) -> bool {
        fits_at(self.indent + 1, text)
    }

    /// The lines that `write` writes at `indent`, apart from the current ones.
    fn detached(&mut self, indent: usize, write: impl FnOnce(&mut Self)) -> Vec<Line> {
        let lines = std::mem::take(&mut self.lines);
        let pending = std::mem::take(&mut self.pending);
        let saved = std::mem::replace(&mut self.indent, indent);
        write(self);
        self.indent = saved;
        self.pending = pending;
        std::mem::replace(&mut self.lines, lines)
    }

    /// Set the pending `with` statement aside, as an expression's function
    /// body starts.
    fn set_pending_aside(&mut self) -> (Vec<PendingWith>, usize) {
        (std::mem::take(&mut self.pending), self.pending_indent)
    }

    fn restore_pending(&mut self, (pending, indent): (Vec<PendingWith>, usize)) {
        self.pending = pending;
        self.pending_indent = indent;
    }

    /// Write the pending `with` statement. Its arguments may need
    /// temporaries, which go before it: they do not refer to its scopes.
    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let mut items = std::mem::take(&mut self.pending);
        let saved = std::mem::replace(&mut self.indent, self.pending_indent);
        if items.len() == 1 {
            let item = items.pop().expect("one item");
            let suffix = item_suffix(&item, ":");
            match item.guard.form {
                Form::Call { callee, arguments } => {
                    self.call(&format!("with {callee}"), arguments, &suffix);
                }
                _ => self.line(format!("with {}{suffix}", item.guard.text)),
            }
        } else {
            // Ruff puts the items one to a line in parentheses.
            for item in &mut items {
                let suffix = item_suffix(item, ",");
                if let Form::Call { callee, arguments } = &mut item.guard.form {
                    let fitted =
                        self.fit_call(callee, std::mem::take(arguments), &suffix, self.indent + 1);
                    item.guard = Value::call(&callee.clone(), fitted, item.guard.class);
                }
            }
            let texts: Vec<String> = items
                .iter()
                .map(|item| format!("{}{}", item.guard.text, item_suffix(item, "")))
                .collect();
            let one_line = format!("with {}:", texts.join(", "));
            if self.fits(&one_line) {
                self.line(one_line);
            } else {
                self.line("with (".to_owned());
                self.indent += 1;
                for item in items {
                    let suffix = item_suffix(&item, ",");
                    match item.guard.form {
                        Form::Call { callee, arguments } => {
                            self.lay_out_call(&callee, &arguments, &suffix);
                        }
                        _ => self.line(format!("{}{suffix}", item.guard.text)),
                    }
                }
                self.indent -= 1;
                self.line("):".to_owned());
            }
        }
        self.indent = saved;
    }

    /// `prefix(arguments)suffix`, laid out as ruff lays out a call. An argument
    /// that ruff would split inside goes into a temporary instead.
    fn call(&mut self, prefix: &str, arguments: Vec<Arg>, suffix: &str) {
        let arguments = self.fit_call(prefix, arguments, suffix, self.indent);
        self.lay_out_call(prefix, &arguments, suffix);
    }

    /// The arguments of a call at `indent`, with temporaries for those that
    /// ruff would split inside.
    fn fit_call(
        &mut self,
        prefix: &str,
        mut arguments: Vec<Arg>,
        suffix: &str,
        indent: usize,
    ) -> Vec<Arg> {
        while let Layout::Hoist(index) = call_layout(prefix, &arguments, suffix, indent) {
            self.hoist(&mut arguments[index].value);
        }
        arguments
    }

    /// Write a call whose arguments need no temporaries.
    fn lay_out_call(&mut self, prefix: &str, arguments: &[Arg], suffix: &str) {
        let texts: Vec<String> = arguments.iter().map(Arg::text).collect();
        let joined = texts.join(", ");
        match call_layout(prefix, arguments, suffix, self.indent) {
            Layout::OneLine | Layout::Hoist(_) => self.line(format!("{prefix}({joined}){suffix}")),
            Layout::Body => self.body_form(prefix, &joined, suffix),
            Layout::PerLine => {
                self.line(format!("{prefix}("));
                self.indent += 1;
                for text in texts {
                    self.line(format!("{text},"));
                }
                self.indent -= 1;
                self.line(format!("){suffix}"));
            }
        }
    }

    /// `prefix(`, the arguments on one line of their own, and `)suffix`.
    fn body_form(&mut self, prefix: &str, arguments: &str, suffix: &str) {
        self.line(format!("{prefix}("));
        self.indent += 1;
        self.line(arguments.to_owned());
        self.indent -= 1;
        self.line(format!("){suffix}"));
    }

    /// `head value`, where `head` is `eN = `, `name = ` or `return `.
    fn let_statement(&mut self, head: &str, value: Value) {
        let one_line = format!("{head}{}", value.text);
        if self.fits(&one_line) {
            self.line(one_line);
            return;
        }
        match value.form {
            Form::Call { callee, arguments } => {
                self.call(&format!("{head}{callee}"), arguments, "")
            }
            Form::Tuple(items) => self.exploded(head, items),
            _ if self.fits_next_line(&value.text) => self.body_form(head, &value.text, ""),
            _ => self.line(one_line),
        }
    }

    /// `head(`, one item to a line, and `)`, as ruff splits a tuple.
    fn exploded(&mut self, head: &str, mut items: Vec<Value>) {
        loop {
            let widest = items
                .iter_mut()
                .filter(|item| {
                    !item.is_unsplittable() && !self.fits_next_line(&format!("{},", item.text))
                })
                .max_by_key(|item| item.text.len());
            match widest {
                Some(item) => self.hoist(item),
                None => break,
            }
        }
        self.line(format!("{head}("));
        self.indent += 1;
        for item in items {
            self.line(format!("{},", item.text));
        }
        self.indent -= 1;
        self.line(")".to_owned());
    }

    /// A call as a statement of its own.
    fn expression_statement(&mut self, value: Value) {
        match value.form {
            Form::Call { callee, arguments } => self.call(&callee, arguments, ""),
            _ => self.line(value.text),
        }
    }

    /// Start the block of the event at `index` with a `with` item, which
    /// joins the pending `with` statement when the block is all of its body.
    /// Returns the indentation of the `with` statement.
    fn open_with(&mut self, guard: Value, target: Option<String>, index: usize) -> usize {
        let close = self.closes[index];
        let joins = self
            .pending
            .last()
            .is_some_and(|last| last.close == close + 1);
        if !joins {
            self.flush();
            self.pending_indent = self.indent;
        }
        self.pending.push(PendingWith {
            guard,
            target,
            close,
        });
        self.indent = self.pending_indent + 1;
        self.pending_indent
    }

    fn nesting(&self) -> usize {
        self.blocks.last().map_or(0, |block| block.nesting)
    }

    /// The scope or arms that statements call.
    fn receiver(&self) -> String {
        self.blocks
            .last()
            .map_or_else(|| "b".to_owned(), |block| block.receiver.clone())
    }

    fn depth(&self) -> usize {
        self.blocks.last().map_or(0, |block| block.depth)
    }

    /// `receiver.method(arguments)` as a statement.
    fn simple(&mut self, method: &str, arguments: Vec<Value>) {
        let callee = format!("{}.{method}", self.receiver());
        let arguments = arguments.into_iter().map(Arg::new).collect();
        self.call(&callee, arguments, "");
    }

    fn method(&self, method: &str, arguments: Vec<Arg>) -> Value {
        Value::call(
            &format!("{}.{method}", self.receiver()),
            arguments,
            Class::Other,
        )
    }

    /// The variable of every definition of the type `name`. Sibling blocks
    /// may each define it, and only one of them is visible at a time.
    fn type_name(&mut self, name: &str) -> String {
        if let Some(identifier) = self.type_names.get(name) {
            return identifier.clone();
        }
        let identifier = self.identifiers.named(Role::Type, name);
        self.type_names.insert(name.to_owned(), identifier.clone());
        identifier
    }

    /// The variable of the visible type `name`. Helpers are functions of
    /// `builder` that see its variables but not each other's, so every helper
    /// between the definition and this use takes the type as a parameter.
    pub(super) fn type_reference(&mut self, name: &str) -> String {
        let visible = self
            .visible_types
            .iter()
            .rev()
            .find(|ty| ty.name == name)
            .expect("a type that a type uses is visible");
        let (variable, function) = (visible.variable.clone(), visible.function);
        if function > 0 {
            let helpers = self
                .blocks
                .iter_mut()
                .filter_map(|block| block.outlined.as_mut());
            for helper in helpers.skip(function) {
                if !helper.types.contains(&variable) {
                    helper.types.push(variable.clone());
                }
            }
        }
        variable.identifier
    }

    /// The number of helper functions around the current statement.
    fn helper_depth(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block| block.outlined.is_some())
            .count()
    }

    /// The annotation of the type that `define_type` returns for `ty`.
    fn type_annotation(&self, ty: &Type) -> &'static str {
        match ty {
            Type::Base(BaseType::Int) | Type::Range { .. } | Type::Alphabet { .. } => {
                "ib.IntType[int]"
            }
            Type::Base(BaseType::Float) => "ib.FloatType[float]",
            Type::Base(BaseType::Num) => "ib.NumType[int | float]",
            Type::Base(BaseType::Word | BaseType::Rest) => "ib.StrType[str]",
            Type::Named(name) => {
                self.visible_types
                    .iter()
                    .rev()
                    .find(|visible| &visible.name == name)
                    .expect("a type that a type uses is visible")
                    .variable
                    .annotation
            }
        }
    }

    fn handle(&self, name: crate::program::NameId) -> Value {
        Value::name(self.value_names[name.0].clone(), Class::Handle)
    }

    fn target(&self, target: &Target) -> Value {
        match target {
            Target::Name(name) => self.handle(*name),
            Target::Discard => Value::atom("ib.discard()".to_owned(), Class::Other),
        }
    }

    /// `value` after `shape`, through a temporary if that is too wide.
    fn shaped(&mut self, value: Value, shape: impl Fn(&Value) -> Value) -> Value {
        self.combine(vec![value], |values| shape(&values[0]))
    }

    /// Render the event at `index` and return the next event to render.
    fn statement(&mut self, event: &'a Event, index: usize) -> usize {
        match event {
            Event::Format(name) => self.simple("format", vec![string(name)]),
            Event::Comment(markers) => self.simple("comment", strings(markers)),
            Event::Stop(markers) => self.simple("stop", strings(markers)),
            Event::Tokens(text) => self.simple("tokens", vec![string(text)]),
            Event::Separators(text) => self.simple("separators", vec![string(text)]),
            Event::IgnoreTrailing => self.simple("ignore_trailing", Vec::new()),
            Event::Read { target, declared } => {
                let target = self.target(target);
                let reading = self.declared(declared);
                self.simple("read", vec![target, reading]);
            }
            Event::List {
                target,
                declared,
                end,
            } => {
                let target = self.target(target);
                let reading = self.list_reading(declared, end);
                self.simple("list", vec![target, reading]);
            }
            Event::Let { target, value } => {
                let target = self.handle(*target);
                let value = self.expr(*value);
                self.simple("let", vec![target, value]);
            }
            Event::Param { target, default } => {
                let target = self.handle(*target);
                let mut arguments = vec![target];
                if let Some(default) = default {
                    arguments.push(self.expr(*default));
                }
                self.simple("param", arguments);
            }
            Event::DefineType { name, ty, used } => {
                let reading = self.reading(ty);
                let arguments = vec![Arg::new(string(name)), Arg::new(reading)];
                let value = self.method("define_type", arguments);
                if *used {
                    let identifier = self.type_name(name);
                    self.let_statement(&format!("{identifier} = "), value);
                    let annotation = self.type_annotation(ty);
                    self.visible_types.push(VisibleType {
                        name: name.clone(),
                        variable: TypeVariable {
                            identifier,
                            annotation,
                        },
                        function: self.helper_depth(),
                    });
                } else {
                    self.expression_statement(value);
                }
            }
            Event::Check { condition, message } => {
                let condition = self.expr(*condition);
                let condition = self.shaped(condition, Value::condition);
                let mut arguments = vec![condition];
                if let Some(message) = message {
                    arguments.push(string(message));
                }
                self.simple("check", arguments);
            }
            Event::Expect(words) => self.simple("expect", strings(words)),
            Event::Line(pattern) => {
                let pattern = self.pattern(pattern);
                self.simple("line", vec![pattern]);
            }
            Event::Find(pattern) => {
                let pattern = self.pattern(pattern);
                self.simple("find", vec![pattern]);
            }
            Event::SkipLine => self.simple("skip_line", Vec::new()),
            Event::SkipLines(count) => {
                let count = self.integral(*count);
                self.simple("skip_lines", vec![count]);
            }
            Event::Index(position) => {
                let position = self.integral(*position);
                self.simple("index", vec![position]);
            }
            Event::Repeat { count, names } => return self.repeat(count, names, index),
            Event::Sections(clauses) => return self.sections(clauses, index),
            Event::Arm {
                pattern,
                optional,
                last,
                block,
            } => return self.arm(pattern, *optional, *last, *block, index),
            Event::If(condition) | Event::ElseIf(condition) => {
                let method = if matches!(event, Event::If(_)) {
                    "if_"
                } else {
                    "else_if"
                };
                let condition = self.expr(*condition);
                let condition = self.shaped(condition, Value::condition);
                let guard = self.method(method, vec![Arg::new(condition)]);
                return self.open_block(guard, Vec::new(), self.depth(), index, true);
            }
            Event::Else => {
                let guard = self.method("else_", Vec::new());
                return self.open_block(guard, Vec::new(), self.depth(), index, true);
            }
            Event::ElseRead { target, declared } => {
                let target = self.target(target);
                let reading = self.declared(declared);
                self.simple("else_read_if", vec![target, reading]);
            }
            Event::Instances => {
                let guard = self.method("instances", Vec::new());
                return self.open_block(guard, Vec::new(), 0, index, true);
            }
            Event::Close => self.close_block(),
        }
        index + 1
    }

    /// An integer expression, as `repeat`, `skip_lines` and `index` take it.
    fn integral(&mut self, expr: crate::program::Expr) -> Value {
        let value = self.expr(expr);
        self.shaped(value, Value::integral)
    }

    /// Open the block of the event at `index` with `guard`, then write
    /// `header`. An empty block without a header is the guard alone when
    /// `bare_when_empty` is set, since a guard never entered writes `{}`.
    fn open_block(
        &mut self,
        guard: Value,
        header: Vec<(&'static str, Value)>,
        depth: usize,
        index: usize,
        bare_when_empty: bool,
    ) -> usize {
        let close = self.closes[index];
        let empty = close == index + 1;
        if empty && header.is_empty() {
            if bare_when_empty {
                self.expression_statement(guard);
            } else {
                let with_indent = self.open_with(guard, None, index);
                self.line("pass".to_owned());
                self.indent = with_indent;
            }
            return close + 1;
        }
        let receiver = format!("s{}", self.blocks.len() + 1);
        let with_indent = self.open_with(guard, Some(receiver.clone()), index);
        let nesting = self.nesting() + 1;
        let outlined = if self.indent > MAX_INDENT || nesting > MAX_NESTING {
            // The `with` statement stays in the function that calls the helper.
            self.flush();
            let call_indent = std::mem::replace(&mut self.indent, HELPER_BODY_INDENT);
            Some(Outlined {
                outer: std::mem::take(&mut self.lines),
                call_indent,
                position: self.helpers.len(),
                function: format!("block_{}", self.next_function()),
                parameter: format!("{receiver}: {}", scope_annotation(depth)),
                types: Vec::new(),
            })
        } else {
            None
        };
        self.blocks.push(OpenBlock {
            receiver,
            depth,
            with_indent,
            nesting: if outlined.is_some() { 0 } else { nesting },
            start: self.lines.len(),
            types: self.visible_types.len(),
            outlined,
        });
        for (method, argument) in header {
            self.simple(method, vec![argument]);
        }
        index + 1
    }

    fn close_block(&mut self) {
        let block = self.blocks.pop().expect("every close has an open block");
        if self.lines.len() == block.start {
            self.line("pass".to_owned());
        }
        self.visible_types.truncate(block.types);
        if let Some(outlined) = block.outlined {
            self.close_helper(outlined, &block.receiver);
        }
        self.indent = block.with_indent;
    }

    /// Put the statements of an outlined block into its helper, and call the
    /// helper with the block's scope and the types of other helpers it uses.
    fn close_helper(&mut self, outlined: Outlined, receiver: &str) {
        let mut body = std::mem::replace(&mut self.lines, outlined.outer);
        let type_parameters = outlined.types.iter().map(|ty| {
            let parameter = format!("{}: {}", ty.identifier, ty.annotation);
            Value::name(parameter, Class::Other)
        });
        let parameters = std::iter::once(Value::name(outlined.parameter, Class::Other))
            .chain(type_parameters)
            .collect();
        let signature = (outlined.function.clone(), parameters, "None");
        let mut function = self.signature(1, signature);
        while body.first().is_some_and(Line::is_blank) {
            body.remove(0);
        }
        function.append(&mut body);
        self.helpers.insert(outlined.position, function);

        let arguments = std::iter::once(receiver.to_owned())
            .chain(outlined.types.into_iter().map(|ty| ty.identifier))
            .map(|argument| Arg::new(Value::name(argument, Class::Other)))
            .collect();
        self.indent = outlined.call_indent;
        self.call(&outlined.function, arguments, "");
    }

    /// The `def` line of a function, at `indent`.
    fn signature(&mut self, indent: usize, signature: Signature) -> Vec<Line> {
        let (name, parameters, annotation) = signature;
        let parameters = parameters.into_iter().map(Arg::new).collect();
        self.detached(indent, |renderer| {
            let prefix = format!("def {name}");
            renderer.call(&prefix, parameters, &format!(" -> {annotation}:"));
        })
    }

    /// A function before the statement that uses it, in the current block.
    fn local_function(&mut self, signature: Signature, mut body: Vec<Line>) {
        self.blank();
        let mut function = self.signature(self.indent, signature);
        while body.first().is_some_and(Line::is_blank) {
            body.remove(0);
        }
        function.append(&mut body);
        self.lines.append(&mut function);
        self.blank();
    }

    /// A function among the helpers at the top of `builder`.
    fn outlined_function(&mut self, signature: Signature, mut body: Vec<Line>) {
        let mut function = self.signature(1, signature);
        while body.first().is_some_and(Line::is_blank) {
            body.remove(0);
        }
        function.append(&mut body);
        self.helpers.push(function);
    }

    fn repeat(&mut self, count: &RepeatCount, names: &RepeatNames, index: usize) -> usize {
        let guard = match count {
            RepeatCount::Times(count) => {
                let count = self.integral(*count);
                self.method("repeat", vec![Arg::new(count)])
            }
            RepeatCount::UntilEof => self.method("repeat_until_eof", Vec::new()),
            RepeatCount::Until { terminators, store } => {
                let mut arguments: Vec<Arg> =
                    strings(terminators).into_iter().map(Arg::new).collect();
                if let Some(store) = store {
                    arguments.push(Arg::keyword("store", self.handle(*store)));
                }
                self.method("repeat_until", arguments)
            }
        };
        let header = match names {
            RepeatNames::Forward(RecordId(record)) => {
                let record = Value::name(self.record_names[*record].clone(), Class::Record);
                vec![("record_as", record)]
            }
            RepeatNames::InPlace { each, list } => {
                let each = each.iter().map(|name| ("each_name", string(name)));
                let list = list.iter().map(|name| ("as_name", string(name)));
                each.chain(list).collect()
            }
        };
        let depth = self.depth() + 1;
        self.open_block(guard, header, depth, index, true)
    }

    fn sections(&mut self, clauses: &[SectionClause], index: usize) -> usize {
        let guard = match clauses {
            []
            | [SectionClause::SkipOther]
            | [SectionClause::Until { .. }]
            | [SectionClause::Until { .. }, SectionClause::SkipOther] => {
                let mut arguments = Vec::new();
                for clause in clauses {
                    match clause {
                        SectionClause::Until { terminators, store } => {
                            let terminators = match strings(terminators).as_slice() {
                                [only] => only.clone(),
                                all => Value::tuple(all.to_vec()),
                            };
                            arguments.push(Arg::keyword("until", terminators));
                            if let Some(store) = store {
                                arguments.push(Arg::keyword("store", self.handle(*store)));
                            }
                        }
                        SectionClause::SkipOther => {
                            let yes = Value::atom("True".to_owned(), Class::Other);
                            arguments.push(Arg::keyword("skip_other", yes));
                        }
                    }
                }
                // `store` follows `skip_other` in the signature, but keywords
                // may come in any order.
                self.method("sections", arguments)
            }
            _ => {
                let mut guard = self.method("sections", Vec::new());
                for clause in clauses {
                    let (method, arguments) = match clause {
                        SectionClause::Until { terminators, store } => {
                            let mut arguments: Vec<Arg> =
                                strings(terminators).into_iter().map(Arg::new).collect();
                            if let Some(store) = store {
                                arguments.push(Arg::keyword("store", self.handle(*store)));
                            }
                            ("until", arguments)
                        }
                        SectionClause::SkipOther => ("skip_other", Vec::new()),
                    };
                    if !self.fits(&format!("with {}.{method}() as s0000:", guard.text)) {
                        self.hoist(&mut guard);
                    }
                    let callee = format!("{}.{method}", guard.text);
                    guard = Value::call(&callee, arguments, Class::Other);
                }
                guard
            }
        };
        let close = self.closes[index];
        if close == index + 1 {
            self.expression_statement(guard);
            return close + 1;
        }
        let receiver = format!("a{}", self.blocks.len() + 1);
        let with_indent = self.open_with(guard, Some(receiver.clone()), index);
        let depth = self.depth();
        let nesting = self.nesting() + 1;
        self.blocks.push(OpenBlock {
            receiver,
            depth,
            with_indent,
            nesting,
            start: self.lines.len(),
            types: self.visible_types.len(),
            outlined: None,
        });
        index + 1
    }

    fn arm(
        &mut self,
        pattern: &[crate::program::Piece],
        optional: bool,
        last: bool,
        block: bool,
        index: usize,
    ) -> usize {
        let pattern = self.pattern(pattern);
        let mut arguments = vec![Arg::new(pattern)];
        let yes = || Value::atom("True".to_owned(), Class::Other);
        if optional {
            arguments.push(Arg::keyword("optional", yes()));
        }
        if last {
            arguments.push(Arg::keyword("last", yes()));
        }
        let guard = self.method("arm", arguments);
        if !block {
            self.expression_statement(guard);
            return index + 1;
        }
        let depth = self.depth();
        self.open_block(guard, Vec::new(), depth, index, false)
    }
}

fn fits_at(indent: usize, text: &str) -> bool {
    indent * TAB + text.len() <= LINE_WIDTH
}

/// How ruff lays out `prefix(arguments)suffix` at `indent`, or the argument
/// that it would split inside, which must go into a temporary first.
fn call_layout(prefix: &str, arguments: &[Arg], suffix: &str, indent: usize) -> Layout {
    let texts: Vec<String> = arguments.iter().map(Arg::text).collect();
    let joined = texts.join(", ");
    if arguments.is_empty() || fits_at(indent, &format!("{prefix}({joined}){suffix}")) {
        return Layout::OneLine;
    }
    if let [only] = arguments {
        return if only.value.is_unsplittable() || fits_at(indent + 1, &joined) {
            Layout::Body
        } else {
            Layout::Hoist(0)
        };
    }
    if fits_at(indent + 1, &joined) {
        return Layout::Body;
    }
    let widest = arguments
        .iter()
        .zip(&texts)
        .enumerate()
        .filter(|(_, (argument, text))| {
            !argument.value.is_unsplittable() && !fits_at(indent + 1, &format!("{text},"))
        })
        .max_by_key(|(_, (_, text))| text.len());
    match widest {
        Some((index, _)) => Layout::Hoist(index),
        None => Layout::PerLine,
    }
}

/// What follows the guard of a `with` item: its target, then `end`.
fn item_suffix(item: &PendingWith, end: &str) -> String {
    match &item.target {
        Some(target) => format!(" as {target}{end}"),
        None => end.to_owned(),
    }
}

/// A function's name, parameters and return annotation.
type Signature = (String, Vec<Value>, &'static str);

/// A string argument.
fn string(text: &str) -> Value {
    Value::atom(quoted(text), Class::Str)
}

fn strings(texts: &[String]) -> Vec<Value> {
    texts.iter().map(|text| string(text)).collect()
}

#[cfg(test)]
mod tests {
    use super::quoted;

    #[test]
    fn strings_are_ascii_with_ruffs_quotes() {
        assert_eq!(quoted("a"), "\"a\"");
        assert_eq!(quoted("say \"hi\""), "'say \"hi\"'");
        assert_eq!(quoted("it's \"x\" 'y'"), "\"it's \\\"x\\\" 'y'\"");
        assert_eq!(quoted("tab\there\\"), "\"tab\\there\\\\\"");
        assert_eq!(quoted("é€😀\u{7f}"), "\"\\xe9\\u20ac\\U0001f600\\x7f\"");
    }
}
