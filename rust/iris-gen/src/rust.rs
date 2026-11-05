//! The Rust backend: a program that calls `gecode_iris::build`.
//!
//! The output is what `rustfmt` would write, without `rustfmt`: statements
//! follow its width rules for calls and chains, and anything wider than those
//! rules allow on a line goes into a `let` first. The events are walked with a
//! loop and a stack of open closures, so no recursion depends on the spec.
mod expr;
mod names;
mod shape;
mod types;

use std::collections::HashMap;

use crate::Backend;
use crate::program::{
    Declared, Event, Expr, ListEnd, NameId, NameKind, Piece, Program, RecordId, RepeatCount,
    RepeatNames, SectionClause, Target, Type,
};
use expr::{Class, Value};
use names::{Identifiers, Role};
use shape::Shapes;

/// `rustfmt`'s `max_width`.
const LINE_WIDTH: usize = 100;
/// `rustfmt`'s `fn_call_width`, `chain_width` and `array_width`.
const CALL_WIDTH: usize = 60;
/// The width left for the arguments of a call that ends with a closure.
const CLOSURE_ARGUMENT_WIDTH: usize = CALL_WIDTH - "|s| {".len() - ", ".len();
/// The same before an empty closure, which rustfmt counts in full.
const EMPTY_CLOSURE_ARGUMENT_WIDTH: usize = CALL_WIDTH - "|_| {}".len() - ", ".len();
/// `rustfmt`'s `short_array_element_width_threshold`.
const SHORT_ARRAY_ITEM: usize = 10;
/// One indentation level.
const TAB: usize = 4;
/// What `let eN = ` and `;` take from a line.
const LET_OVERHEAD: usize = "let e000 = ;".len();
/// The narrowest budget for an expression, however deep the block.
const NARROWEST_EXPRESSION: usize = 24;

/// Above this many nested closures, `rustfmt` takes too long to be useful and
/// its layout no longer follows from widths alone: at eleven it breaks an
/// `.else_` that fits, and at ten and twelve it does not.
const DEEPEST_FORMATTED: usize = 10;
/// Above this many nested closures, `rustc` needs a higher recursion limit.
const DEEPEST_DEFAULT_RECURSION: usize = 100;

/// A string as a Rust literal, in ASCII.
///
/// `rustfmt` measures a line in display columns. Without a table of them, only
/// ASCII has a length that is its width, so everything else is escaped.
fn quoted(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    for character in format!("{text:?}").chars() {
        if character.is_ascii() {
            literal.push(character);
        } else {
            literal.extend(character.escape_unicode());
        }
    }
    literal
}

pub(crate) struct RustBackend;

impl Backend for RustBackend {
    fn render(&self, program: &Program) -> String {
        Renderer::new(program).render()
    }
}

/// One line of output, without its indentation.
#[derive(Debug, Clone)]
struct Line {
    indent: usize,
    text: String,
}

/// The lints that the generated code needs to allow.
#[derive(Debug, Default)]
struct Lints {
    approx_constant: bool,
    eq_op: bool,
}

/// What closes an open closure.
#[derive(Debug)]
enum OpenBlock {
    /// `});`, with this indentation.
    Plain {
        close_indent: usize,
        /// Whether the closure is on its own line, and so ends with `},` and `);`.
        own_line: bool,
    },
    /// The next branch of an `if`, or its end.
    Branch {
        chain: IfChain,
        position: usize,
        close_indent: usize,
    },
}

/// The branches of one `if`, rendered in advance: an `else if` cannot hoist
/// anything in the middle of the chain.
#[derive(Debug)]
struct IfChain {
    branches: Vec<Branch>,
    /// The call that ends the chain.
    end: String,
    /// The event after the whole `if`.
    next_event: usize,
}

#[derive(Debug)]
struct Branch {
    /// The call up to its closure's parameter, as in `.else_if(c, `.
    head: String,
    /// The block's first event; the block is empty when this is its `Close`.
    first_event: usize,
    close_event: usize,
}

impl Branch {
    fn is_empty(&self) -> bool {
        self.first_event == self.close_event
    }
}

struct Renderer<'a> {
    program: &'a Program,
    value_names: Vec<String>,
    record_names: Vec<String>,
    variable_names: Vec<String>,
    identifiers: Identifiers,
    /// The type names of the spec by their Iris spelling.
    type_names: HashMap<String, String>,
    lines: Vec<Line>,
    indent: usize,
    /// The receiver of statements in each open closure.
    receivers: Vec<&'static str>,
    blocks: Vec<OpenBlock>,
    /// The matching `Close` of each event that opens a block.
    closes: Vec<usize>,
    temporaries: usize,
    lints: Lints,
    shapes: Shapes,
    deep: bool,
    /// The most branches of one `if`.
    longest_if_chain: usize,
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
            .map(|record| identifiers.fresh(&record.each))
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
            lines: Vec::new(),
            indent: 1,
            receivers: vec!["b"],
            blocks: Vec::new(),
            closes: matching_closes(&program.events),
            temporaries: 0,
            lints: Lints::default(),
            shapes: Shapes::default(),
            deep: program.closure_depth > DEEPEST_FORMATTED,
            longest_if_chain: 0,
        }
    }

    fn render(mut self) -> String {
        let first = match self.program.events.first() {
            Some(Event::Format(_)) => 1,
            _ => 0,
        };
        let (constructor, arguments) = match self.program.events.first() {
            Some(Event::Format(name)) => ("SpecBuilder::new", vec![quoted(name)]),
            _ => ("SpecBuilder::unnamed", Vec::new()),
        };
        let empty = first == self.program.events.len();
        if empty {
            self.call(constructor, &arguments, "");
        } else {
            self.bind("let mut b", constructor, &arguments);
            self.handles();
            self.statements(first);
            self.line("b".to_owned());
        }
        self.finish()
    }

    /// Handles for every value name and record, made before their definitions
    /// because a reference may come first in the text.
    fn handles(&mut self) {
        let program = self.program;
        for (name, identifier) in program.names.iter().zip(self.value_names.clone()) {
            let text = quoted(&name.text);
            match name.kind {
                NameKind::Forward => {
                    self.bind(&format!("let {identifier}: DynField"), "b.forward", &[text]);
                }
                NameKind::Undefined => {
                    self.bind(&format!("let {identifier}"), "b.undefined", &[text]);
                }
            }
        }
        for (record, identifier) in program.records.iter().zip(self.record_names.clone()) {
            self.bind(
                &format!("let {identifier}"),
                "b.forward_record",
                &[quoted(&record.each), quoted(&record.list)],
            );
        }
    }

    fn finish(self) -> String {
        let mut text = String::new();
        // Each `else_if` nests the type of the guard one level further.
        if self.program.closure_depth + self.longest_if_chain > DEEPEST_DEFAULT_RECURSION {
            text.push_str("#![recursion_limit = \"4096\"]\n\n");
        }
        text.push_str("use gecode_iris::build::*;\n\n");
        let allowed: Vec<&str> = [
            (self.lints.approx_constant, "clippy::approx_constant"),
            (self.lints.eq_op, "clippy::eq_op"),
        ]
        .into_iter()
        .filter_map(|(needed, lint)| needed.then_some(lint))
        .collect();
        if !allowed.is_empty() {
            text.push_str(&format!("#[allow({})]\n", allowed.join(", ")));
        }
        if self.deep {
            text.push_str("#[rustfmt::skip]\n");
        }
        text.push_str("pub fn builder() -> SpecBuilder {\n");
        for line in &self.lines {
            for _ in 0..line.indent * TAB {
                text.push(' ');
            }
            text.push_str(&line.text);
            text.push('\n');
        }
        text.push_str("}\n\n");
        text.push_str("fn main() {\n");
        text.push_str("    let source = builder().source().expect(\"the spec is valid\");\n");
        text.push_str("    print!(\"{source}\");\n");
        text.push_str("}\n");
        text
    }

    fn line(&mut self, text: String) {
        self.lines.push(Line {
            indent: self.indent,
            text,
        });
    }

    fn temporary(&mut self) -> String {
        self.temporaries += 1;
        format!("e{}", self.temporaries)
    }

    /// The widest expression the current block can hold on one line.
    fn budget(&self) -> usize {
        if self.deep {
            return CALL_WIDTH;
        }
        LINE_WIDTH
            .saturating_sub(self.indent * TAB + LET_OVERHEAD)
            .clamp(NARROWEST_EXPRESSION, CALL_WIDTH)
    }

    /// `head = call(arguments);`. The call must fit on one line of its own, so
    /// an argument too wide for that goes into a `let` first.
    fn bind(&mut self, head: &str, call: &str, arguments: &[String]) {
        let mut width = LINE_WIDTH.saturating_sub((self.indent + 1) * TAB + ";".len());
        if arguments.len() > 1 {
            width = width.min(call.len() + "()".len() + CALL_WIDTH);
        }
        let mut arguments: Vec<Value> = arguments
            .iter()
            .map(|argument| Value::new(argument.clone(), Class::Other))
            .collect();
        let value = self.fit(&mut arguments, width, |values| {
            let arguments: Vec<&str> = values.iter().map(|value| value.text.as_str()).collect();
            Value::new(format!("{call}({})", arguments.join(", ")), Class::Other)
        });
        self.let_statement(head, &value.text);
    }

    /// `head = value;`, with the value on the next line if that lets it fit. A
    /// value too wide for either line stays beside its `=`, as rustfmt leaves it.
    fn let_statement(&mut self, head: &str, value: &str) {
        let statement = format!("{head} = {value};");
        let next_line = format!("{value};");
        if self.fits(&statement) || !self.fits_next_line(&next_line) {
            self.line(statement);
            return;
        }
        self.line(format!("{head} ="));
        self.indent += 1;
        self.line(next_line);
        self.indent -= 1;
    }

    fn fits(&self, text: &str) -> bool {
        self.indent * TAB + text.len() <= LINE_WIDTH
    }

    /// Whether `text` fits one level in.
    fn fits_next_line(&self, text: &str) -> bool {
        (self.indent + 1) * TAB + text.len() <= LINE_WIDTH
    }

    fn receiver(&self) -> &'static str {
        self.receivers.last().expect("the root receiver stays")
    }

    fn type_name(&mut self, name: &str) -> String {
        if let Some(identifier) = self.type_names.get(name) {
            return identifier.clone();
        }
        let identifier = self.identifiers.named(Role::Type, name);
        self.type_names.insert(name.to_owned(), identifier.clone());
        identifier
    }

    fn target(&self, target: &Target) -> String {
        match target {
            Target::Name(name) => self.value_names[name.0].clone(),
            Target::Discard => "discard()".to_owned(),
        }
    }

    /// `prefix(arguments)suffix`, with `rustfmt`'s rule for when the arguments
    /// go one to a line.
    fn call(&mut self, prefix: &str, arguments: &[String], suffix: &str) {
        let joined = arguments.join(", ");
        let one_line = format!("{prefix}({joined}){suffix}");
        let narrow = arguments.len() == 1 || joined.len() <= CALL_WIDTH;
        if narrow && self.fits(&one_line) {
            self.line(one_line);
            return;
        }
        self.line(format!("{prefix}("));
        self.indent += 1;
        for argument in arguments {
            self.line(format!("{argument},"));
        }
        self.indent -= 1;
        self.line(format!("){suffix}"));
    }

    /// `calls` on the current receiver, with `rustfmt`'s rule for chains.
    fn chain(&mut self, calls: &[String]) {
        let receiver = self.receiver();
        let one_line = format!("{receiver}{};", calls.concat());
        if calls.len() == 1 || (one_line.len() - 1 <= CALL_WIDTH && self.fits(&one_line)) {
            self.line(one_line);
            return;
        }
        self.line(format!("{receiver}{}", calls[0]));
        self.indent += 1;
        let last = calls.len() - 1;
        for (index, call) in calls.iter().enumerate().skip(1) {
            let end = if index == last { ";" } else { "" };
            self.line(format!("{call}{end}"));
        }
        self.indent -= 1;
    }

    /// `calls` on the current receiver, and `closure` as the argument of the
    /// last one. Returns the closure's closing indentation, and whether it is
    /// on a line of its own.
    fn chain_into_closure(&mut self, calls: &[String], closure: &str) -> (usize, bool) {
        let receiver = self.receiver();
        let head = format!("{receiver}{}", calls.concat());
        let one_line = format!("{head}{closure}");
        let room = self.fits(&format!("{one_line});"));
        if calls.len() == 1 || (one_line.len() <= CALL_WIDTH && room) {
            self.line(one_line);
            return (self.indent, false);
        }
        if !room && head.len() <= CALL_WIDTH && self.fits(&format!("{head})")) {
            // rustfmt keeps a narrow chain whole and puts the closure on its own.
            self.line(head);
            self.indent += 1;
            self.line(closure.to_owned());
            return (self.indent, true);
        }
        let (last, rest) = calls[1..].split_last().expect("a chain has two calls");
        self.line(format!("{receiver}{}", calls[0]));
        self.indent += 1;
        for call in rest {
            self.line(call.clone());
        }
        self.line(format!("{last}{closure}"));
        (self.indent, false)
    }

    /// `.method(argument)` on a line of a chain, the argument going into a `let`
    /// if the line would be too wide for it.
    fn chain_call(&mut self, method: &str, argument: String) -> String {
        let overhead = method.len() + ".();".len();
        let width = LINE_WIDTH.saturating_sub((self.indent + 1) * TAB + overhead);
        let argument = self.narrow(Value::new(argument, Class::Other), width);
        format!(".{method}({})", argument.text)
    }

    /// `.store(name)`, for a chain.
    fn store_call(&mut self, name: NameId) -> String {
        let name = Value::new(self.value_names[name.0].clone(), Class::Handle);
        let name = self.narrow(name, self.budget());
        format!(".store({})", name.text)
    }

    /// The width for the arguments of `call`, which is missing its closure,
    /// when the line must also hold the receiver and the closure.
    fn closure_argument_width(&self, call: &str, empty: bool) -> usize {
        let (width, closure) = if empty {
            // An `else` may run on from an empty branch.
            (EMPTY_CLOSURE_ARGUMENT_WIDTH, ", |_| {}).else_(|s| {")
        } else {
            (CLOSURE_ARGUMENT_WIDTH, ", |s| {")
        };
        // A line of a chain starts one level deeper. A receiver takes less than
        // that, but rustfmt keeps room on its line for the `);` of the closure.
        let line = (self.indent + 1) * TAB + call.len() + closure.len();
        width.min(LINE_WIDTH.saturating_sub(line))
    }

    /// An array of strings, as a call's only argument.
    fn strings(&mut self, strings: &[String]) -> String {
        let items = strings.iter().map(|text| quoted(text)).collect();
        self.array(items, CALL_WIDTH.min(self.budget()))
    }

    /// An array, through a `let` if it is wider than `width`.
    fn array(&mut self, items: Vec<String>, width: usize) -> String {
        let one_line = format!("[{}]", items.join(", "));
        if one_line.len() <= width {
            return one_line;
        }
        let name = self.temporary();
        let head = format!("let {name}");
        let narrow = items.len() == 1 || one_line.len() - "[]".len() <= CALL_WIDTH;
        let statement = format!("{head} = {one_line};");
        if narrow && (self.fits(&statement) || self.fits_next_line(&format!("{one_line};"))) {
            self.let_statement(&head, &one_line);
            return name;
        }
        self.line(format!("{head} = ["));
        self.indent += 1;
        if items.iter().all(|item| item.len() <= SHORT_ARRAY_ITEM) {
            self.packed_items(&items);
        } else {
            for item in items {
                self.line(format!("{item},"));
            }
        }
        self.indent -= 1;
        self.line("];".to_owned());
        name
    }

    /// Short array items share lines.
    fn packed_items(&mut self, items: &[String]) {
        let mut line = String::new();
        for item in items {
            let added = line.len() + usize::from(!line.is_empty()) + item.len() + 1;
            if !line.is_empty() && self.indent * TAB + added > LINE_WIDTH {
                self.line(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(item);
            line.push(',');
        }
        if !line.is_empty() {
            self.line(line);
        }
    }

    fn statements(&mut self, first: usize) {
        let program = self.program;
        let mut index = first;
        while let Some(event) = program.events.get(index) {
            index = self.statement(event, index);
        }
    }

    /// Render the event at `index` and return the next event to render.
    fn statement(&mut self, event: &'a Event, index: usize) -> usize {
        match event {
            Event::Format(name) => self.simple("format", &[quoted(name)]),
            Event::Comment(markers) => {
                let markers = self.strings(markers);
                self.simple("comment", &[markers]);
            }
            Event::Stop(markers) => {
                let markers = self.strings(markers);
                self.simple("stop", &[markers]);
            }
            Event::Tokens(text) => self.simple("tokens", &[quoted(text)]),
            Event::Separators(text) => self.simple("separators", &[quoted(text)]),
            Event::IgnoreTrailing => self.simple("ignore_trailing", &[]),
            Event::Read { target, declared } => {
                let target = self.target(target);
                let reading = self.declared(declared);
                self.simple("read", &[target, reading.text]);
            }
            Event::List {
                target,
                declared,
                end,
            } => self.list(target, declared, end),
            Event::Let { target, value } => {
                let target = self.value_names[target.0].clone();
                let value = self.expr(*value);
                self.simple("let_", &[target, value.text]);
            }
            Event::Param { target, default } => {
                let target = self.value_names[target.0].clone();
                match default {
                    Some(default) => {
                        let default = self.expr(*default);
                        self.simple("param_default", &[target, default.text]);
                    }
                    None => self.simple("param", &[target]),
                }
            }
            Event::DefineType { name, ty, used } => self.define_type(name, ty, *used),
            Event::Check { condition, message } => {
                let condition = self.expr(*condition);
                match message {
                    Some(message) => {
                        let condition = self.narrow(condition, self.budget());
                        let check = format!(".check({})", condition.text);
                        let message = self.chain_call("message", quoted(message));
                        self.chain(&[check, message]);
                    }
                    None => self.simple("check", &[condition.text]),
                }
            }
            Event::Expect(words) => {
                let words = self.strings(words);
                self.simple("expect", &[words]);
            }
            Event::Line(pattern) => {
                let pattern = self.pattern(pattern);
                self.simple("line", &[pattern]);
            }
            Event::Find(pattern) => {
                let pattern = self.pattern(pattern);
                self.simple("find", &[pattern]);
            }
            Event::SkipLine => self.simple("skip_line", &[]),
            Event::SkipLines(count) => {
                let count = self.expr(*count);
                self.simple("skip_lines", &[count.text]);
            }
            Event::Index(position) => {
                let position = self.expr(*position);
                self.simple("index", &[position.text]);
            }
            Event::Repeat { count, names } => return self.repeat(count, names, index),
            Event::Sections(clauses) => return self.sections(clauses, index),
            Event::Arm {
                pattern,
                optional,
                last,
                block,
            } => return self.arm(pattern, *optional, *last, *block, index),
            Event::If(_) => return self.if_chain(index),
            Event::Instances => {
                let calls = [".instances(".to_owned()];
                return self.block(&calls, "s", index);
            }
            Event::Close => return self.close_block(index),
            Event::ElseIf(_) | Event::ElseRead { .. } | Event::Else => {
                unreachable!("an if chain consumes its branches")
            }
        }
        index + 1
    }

    fn simple(&mut self, method: &str, arguments: &[String]) {
        let prefix = format!("{}.{method}", self.receiver());
        self.call(&prefix, arguments, ";");
    }

    fn list(&mut self, target: &Target, declared: &Declared, end: &ListEnd) {
        let target = self.target(target);
        let reading = self.list_reading(declared, end);
        self.simple("list", &[target, reading.text]);
    }

    fn define_type(&mut self, name: &str, ty: &Type, used: bool) {
        let reading = self.reading(ty);
        let arguments = [quoted(name), reading.text];
        if used {
            let identifier = self.type_name(name);
            let call = format!("{}.define_type", self.receiver());
            self.bind(&format!("let {identifier}"), &call, &arguments);
        } else {
            self.simple("define_type", &arguments);
        }
    }

    /// Open the block of the event at `index` through `calls`, whose last one
    /// is missing its closure. An empty block closes at once.
    fn block(&mut self, calls: &[String], parameter: &'static str, index: usize) -> usize {
        let close = self.closes[index];
        if close == index + 1 {
            let mut calls = calls.to_vec();
            let last = calls.last_mut().expect("a block has its call");
            last.push_str("|_| {})");
            self.chain(&calls);
            return close + 1;
        }
        self.open_closure(calls, parameter);
        index + 1
    }

    /// Start the closure `|parameter| {` as the argument of the last of `calls`.
    fn open_closure(&mut self, calls: &[String], parameter: &'static str) {
        let closure = format!("|{parameter}| {{");
        let (close_indent, own_line) = self.chain_into_closure(calls, &closure);
        self.indent = close_indent + 1;
        self.receivers.push(parameter);
        self.blocks.push(OpenBlock::Plain {
            close_indent,
            own_line,
        });
    }

    fn close_block(&mut self, index: usize) -> usize {
        self.receivers.pop();
        match self.blocks.pop().expect("every close has an open block") {
            OpenBlock::Plain {
                close_indent,
                own_line,
            } => {
                self.indent = close_indent;
                if own_line {
                    self.line("},".to_owned());
                    self.indent -= 1;
                    self.line(");".to_owned());
                } else {
                    self.line("});".to_owned());
                }
                let original = self.statement_indent();
                self.indent = original;
                index + 1
            }
            OpenBlock::Branch {
                chain,
                position,
                close_indent,
            } => {
                self.indent = close_indent;
                self.next_branch(chain, position + 1, "})".to_owned())
            }
        }
    }

    /// The indentation of statements in the innermost open block.
    fn statement_indent(&self) -> usize {
        match self.blocks.last() {
            None => 1,
            Some(OpenBlock::Plain { close_indent, .. })
            | Some(OpenBlock::Branch { close_indent, .. }) => close_indent + 1,
        }
    }

    fn repeat(&mut self, count: &RepeatCount, names: &RepeatNames, index: usize) -> usize {
        let calls = match count {
            RepeatCount::Times(count) => {
                let count = self.expr(*count);
                let width =
                    self.closure_argument_width(".repeat(", self.closes[index] == index + 1);
                let count = self.narrow(count, width);
                vec![format!(".repeat({}, ", count.text)]
            }
            RepeatCount::UntilEof => {
                vec![".repeat_until_eof()".to_owned(), ".body(".to_owned()]
            }
            RepeatCount::Until { terminators, store } => {
                let terminators = self.strings(terminators);
                let mut calls = vec![self.chain_call("repeat_until", terminators)];
                if let Some(store) = store {
                    calls.push(self.store_call(*store));
                }
                calls.push(".body(".to_owned());
                calls
            }
        };
        let header = self.repeat_header(names);
        if header.is_empty() {
            return self.block(&calls, "s", index);
        }
        self.open_closure(&calls, "s");
        for (method, argument) in header {
            self.simple(method, &[argument]);
        }
        index + 1
    }

    /// The statements that name a repeat's record.
    fn repeat_header(&self, names: &RepeatNames) -> Vec<(&'static str, String)> {
        match names {
            RepeatNames::Forward(RecordId(record)) => {
                vec![("record_as", self.record_names[*record].clone())]
            }
            RepeatNames::InPlace { each, list } => {
                let each = each.iter().map(|name| ("each_name", quoted(name)));
                let list = list.iter().map(|name| ("as_name", quoted(name)));
                each.chain(list).collect()
            }
        }
    }

    fn sections(&mut self, clauses: &[SectionClause], index: usize) -> usize {
        let mut calls = vec![".sections()".to_owned()];
        for clause in clauses {
            match clause {
                SectionClause::Until { terminators, store } => {
                    let terminators = self.strings(terminators);
                    calls.push(self.chain_call("until", terminators));
                    if let Some(store) = store {
                        calls.push(self.store_call(*store));
                    }
                }
                SectionClause::SkipOther => calls.push(".skip_other()".to_owned()),
            }
        }
        calls.push(".body(".to_owned());
        self.block(&calls, "a", index)
    }

    fn arm(
        &mut self,
        pattern: &[Piece],
        optional: bool,
        last: bool,
        block: bool,
        index: usize,
    ) -> usize {
        let pattern = self.pattern(pattern);
        if !optional && !last && !block {
            self.simple("arm", &[pattern]);
            return index + 1;
        }
        let pattern = self.narrow(Value::new(pattern, Class::Other), self.budget());
        let mut calls = vec![format!(".arm({})", pattern.text)];
        if optional {
            calls.push(".optional()".to_owned());
        }
        if last {
            calls.push(".last()".to_owned());
        }
        if block {
            calls.push(".body(".to_owned());
            return self.block(&calls, "s", index);
        }
        self.chain(&calls);
        index + 1
    }

    /// The branches after the `if` at `index`, with everything they hoist
    /// written before the chain starts.
    fn if_branches(&mut self, index: usize) -> IfChain {
        let program = self.program;
        let mut branches = Vec::new();
        let mut opener = index;
        let mut end = ".finish();".to_owned();
        let next_event = loop {
            let close = self.closes[opener];
            let head = match &program.events[opener] {
                Event::If(condition) => {
                    let call = ".if_(";
                    let condition = self.branch_condition(*condition, call, opener + 1 == close);
                    format!("{call}{condition}, ")
                }
                Event::ElseIf(condition) => {
                    let call = ".else_if(";
                    let condition = self.branch_condition(*condition, call, opener + 1 == close);
                    format!("{call}{condition}, ")
                }
                _ => ".else_(".to_owned(),
            };
            branches.push(Branch {
                head,
                first_event: opener + 1,
                close_event: close,
            });
            let after = close + 1;
            match program.events.get(after) {
                Some(Event::ElseIf(_) | Event::Else) => opener = after,
                Some(Event::ElseRead { target, declared }) => {
                    let target = self.target(target);
                    let reading = self.declared(declared);
                    end = self.else_read_call(target, reading);
                    break after + 1;
                }
                _ => break after,
            }
        };
        if matches!(program.events[opener], Event::Else) {
            end = String::new();
        }
        self.longest_if_chain = self.longest_if_chain.max(branches.len());
        IfChain {
            branches,
            end,
            next_event,
        }
    }

    /// A condition narrow enough to keep its call on one line.
    fn branch_condition(&mut self, condition: Expr, call: &str, empty_branch: bool) -> String {
        let width = self.closure_argument_width(call, empty_branch);
        let condition = self.expr(condition);
        self.narrow(condition, width).text
    }

    /// `.else_read_if(target, reading);`, which must fit on a line of the chain.
    fn else_read_call(&mut self, target: String, reading: Value) -> String {
        let target = Value::new(target, Class::Handle);
        let width = CALL_WIDTH.min(LINE_WIDTH.saturating_sub((self.indent + 1) * TAB));
        let mut arguments = [target, reading];
        let value = self.fit(&mut arguments, width, |values| {
            let text = format!(".else_read_if({}, {});", values[0].text, values[1].text);
            Value::new(text, Class::Other)
        });
        value.text
    }

    fn if_chain(&mut self, index: usize) -> usize {
        let chain = self.if_branches(index);
        let receiver = self.receiver();
        if chain.branches[0].is_empty() {
            let rest = self.one_line_rest(&chain);
            if let Some(rest) = rest {
                let head = &chain.branches[0].head;
                let text = format!("{receiver}{head}|_| {{}}){rest}");
                if text.len() <= CALL_WIDTH + 1 && self.fits(&text) {
                    self.line(text);
                    return chain.next_event;
                }
            }
            let head = chain.branches[0].head.clone();
            if let [_, only] = &chain.branches[..]
                && chain.end.is_empty()
                && !only.is_empty()
            {
                // rustfmt lets a chain's only call run on into its closure.
                let text = format!("{receiver}{head}|_| {{}}){}|s| {{", only.head);
                if text.len() <= CALL_WIDTH && self.fits(&text) {
                    self.line(text);
                    let close_indent = self.indent;
                    let first_event = only.first_event;
                    self.enter_branch(chain, 1, close_indent);
                    return first_event;
                }
            }
            self.line(format!("{receiver}{head}|_| {{}})"));
            self.indent += 1;
            return self.next_branch(chain, 1, String::new());
        }
        let head = &chain.branches[0].head;
        self.line(format!("{receiver}{head}|s| {{"));
        let close_indent = self.indent;
        let first_event = chain.branches[0].first_event;
        self.enter_branch(chain, 0, close_indent);
        first_event
    }

    /// The rest of an `if` on one line, if every branch is empty.
    fn one_line_rest(&self, chain: &IfChain) -> Option<String> {
        let mut text = String::new();
        for branch in &chain.branches[1..] {
            if !branch.is_empty() {
                return None;
            }
            text.push_str(&format!("{}|_| {{}})", branch.head));
        }
        text.push_str(&chain.end);
        if chain.end.is_empty() {
            text.push(';');
        }
        Some(text)
    }

    fn enter_branch(&mut self, chain: IfChain, position: usize, close_indent: usize) {
        self.indent = close_indent + 1;
        self.receivers.push("s");
        self.blocks.push(OpenBlock::Branch {
            chain,
            position,
            close_indent,
        });
    }

    /// Continue an `if` from branch `position`. `pending` is the text that the
    /// next call goes after: the end of the block before it, on its own line.
    fn next_branch(&mut self, chain: IfChain, position: usize, pending: String) -> usize {
        if !pending.is_empty() {
            self.line(pending);
        }
        let mut position = position;
        while let Some(branch) = chain.branches.get(position) {
            if !branch.is_empty() {
                let text = format!("{}|s| {{", branch.head);
                self.line(text);
                let close_indent = self.indent;
                let first_event = branch.first_event;
                self.enter_branch(chain, position, close_indent);
                return first_event;
            }
            let end = if position + 1 == chain.branches.len() && chain.end.is_empty() {
                ";"
            } else {
                ""
            };
            self.line(format!("{}|_| {{}}){end}", branch.head));
            position += 1;
        }
        if chain.end.is_empty() {
            let last = self.lines.last_mut().expect("the chain has a line");
            if !last.text.ends_with(';') {
                last.text.push(';');
            }
        } else {
            self.line(chain.end.clone());
        }
        self.indent = self.statement_indent();
        chain.next_event
    }
}
