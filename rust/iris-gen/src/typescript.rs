//! The TypeScript backend: a module that exports a function giving the
//! `SpecBuilder` of `gecode-iris/build`, and prints its source when it runs.
//!
//! The output is what oxfmt writes, without oxfmt: a statement that is too
//! wide goes into the layouts that it gives calls, and an argument too wide
//! for those goes into a local constant or, for a string, a module constant
//! first. Blocks are arrow functions that take their scope, numbered by how
//! deep they are so that none hides another, and a block that would be too
//! deep, or whose statements would start too far right, becomes a function
//! of the module that takes the handles and types it uses. An `if` with its
//! `else` branches is one chain of calls. A comprehension always goes into
//! a local constant of its own, and its body becomes a function of the
//! module when it needs statements far to the right.
//!
//! The events are walked with a loop and a stack of open blocks, so no
//! recursion depends on the spec.
mod expr;
mod layout;
mod names;
mod types;
pub(crate) mod view;

use std::collections::BTreeSet;

use crate::Backend;
use crate::program::{
    Event, NameId, NameKind, Piece, Program, RecordId, RepeatCount, RepeatNames, SectionClause,
    Target,
};
use expr::{Capture, Class, Form, Value};
use layout::{Block, Link, call_lines};
use names::{Identifiers, Role};

/// oxfmt's line width, from `typescript/.oxfmtrc.json`.
const LINE_WIDTH: usize = 100;
/// One indentation level.
const TAB: usize = 2;
/// The column of a function's statements.
const BODY_COLUMN: usize = 2;
/// What `const eN = ;` takes from a line.
const LET_OVERHEAD: usize = "const e0000 = ;".len();
/// The narrowest budget for an expression, however deep the block.
const NARROWEST_EXPRESSION: usize = 24;
/// The column after which a block's statements go to a function of their own.
const DEEPEST_BODY: usize = 36;
/// The deepest arrow function in one function. A scope parameter up to
/// this depth has at most two characters, which keeps oxfmt's layout of a
/// chain of calls on it the same.
const DEEPEST_SCOPE: usize = 9;
/// The module that the program imports.
const MODULE: &str = "gecode-iris/build";

/// The names that `gecode-iris/build` exports as values and a program may
/// use.
const VALUE_EXPORTS: &[&str] = &[
    "SpecBuilder",
    "all",
    "alphabet",
    "any",
    "atLeast",
    "atMost",
    "below",
    "bind",
    "blank",
    "count",
    "defined",
    "discard",
    "discardAs",
    "distinct",
    "div",
    "eol",
    "float",
    "floatLit",
    "gecode",
    "int",
    "lit",
    "max",
    "min",
    "num",
    "over",
    "p",
    "pow",
    "range",
    "ref",
    "rem",
    "rest",
    "restOfLine",
    "size",
    "sum",
    "validate",
    "word",
];

/// The names that `gecode-iris/build` exports as types and a program may
/// use in a signature.
const TYPE_EXPORTS: &[&str] = &[
    "Arms",
    "DynField",
    "Expr",
    "HandleDepth",
    "Lookup",
    "NestedScope",
    "Presence",
    "Record",
    "RecordExpr",
    "Scope",
    "Type",
];

/// A string as a TypeScript literal, in ASCII.
///
/// oxfmt uses double quotes unless the text has more of them than single
/// quotes, and keeps escapes otherwise as written. The round trip compares
/// bytes, so everything outside printable ASCII is a `\u` escape of its
/// UTF-16 units.
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
            character if character == quote => {
                literal.push('\\');
                literal.push(character);
            }
            ' '..='~' => literal.push(character),
            _ => {
                let mut units = [0; 2];
                for unit in character.encode_utf16(&mut units) {
                    literal.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    literal.push(quote);
    literal
}

pub(crate) struct TypeScriptBackend;

impl Backend for TypeScriptBackend {
    fn render(&self, program: &Program) -> String {
        Renderer::new(program).render()
    }
}

/// One line of output.
#[derive(Debug, Clone)]
struct Line {
    column: usize,
    text: String,
}

impl Line {
    fn new(column: usize, text: String) -> Self {
        Self { column, text }
    }
}

fn fits(column: usize, text: &str) -> bool {
    column + text.len() <= LINE_WIDTH
}

/// What a block's arrow function takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeKind {
    /// A `Scope`: the block of an `if`, an arm or `instances`.
    Scope,
    /// A `NestedScope`: the block of a repeat, which names its record.
    Nested,
    /// The `Arms` of `sections`.
    Arms,
}

impl ScopeKind {
    fn of(method: &str) -> Self {
        match method {
            "repeat" | "repeatUntil" | "repeatUntilEof" => Self::Nested,
            "sections" => Self::Arms,
            _ => Self::Scope,
        }
    }

    /// The first letter of the parameter.
    fn prefix(self) -> &'static str {
        match self {
            Self::Arms => "a",
            Self::Scope | Self::Nested => "s",
        }
    }

    /// The parameter type of a function that holds the block's statements,
    /// which a block at any depth passes.
    fn class(self) -> &'static str {
        match self {
            Self::Scope => "Scope<HandleDepth, Presence>",
            Self::Nested => "NestedScope<HandleDepth, Presence>",
            Self::Arms => "Arms<HandleDepth, Presence>",
        }
    }
}

/// Whether `method` may leave out an empty block's arrow function.
fn body_is_optional(method: &str) -> bool {
    matches!(
        method,
        "repeat" | "repeatUntil" | "repeatUntilEof" | "sections" | "instances"
    )
}

/// A type definition that a later type uses, while it is visible.
#[derive(Debug)]
struct VisibleType {
    name: String,
    variable: String,
    ts_type: &'static str,
    /// The number of block functions around the definition.
    function: usize,
}

/// A block whose statements go to a function of their own.
#[derive(Debug)]
struct Outlined {
    function: String,
    /// The handles, records and types that the block uses.
    parameters: Vec<Capture>,
}

/// The call that opens a block, written when the block closes.
#[derive(Debug)]
struct OpenBlock {
    method: &'static str,
    arguments: Vec<Value>,
    kind: ScopeKind,
    /// The parameter of the block's arrow function.
    parameter: String,
    /// The scope that the block's statements are calls on.
    receiver: String,
    /// The arrow functions around the block's statements in their function.
    depth: usize,
    /// The lines before the block's opener.
    outer: Vec<Line>,
    /// The column of the opener's statement.
    column: usize,
    /// The column of the arrow function's statements.
    body_column: usize,
    /// The number of visible types before the block.
    types: usize,
    outlined: Option<Outlined>,
    /// Whether the block is a link of an `if` chain.
    chained: bool,
}

/// An `if` with its `else` branches, written when its last link ends.
#[derive(Debug)]
struct Chain {
    receiver: String,
    column: usize,
    links: Vec<Link>,
    total: usize,
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

struct Renderer<'a> {
    program: &'a Program,
    value_names: Vec<String>,
    record_names: Vec<String>,
    variable_names: Vec<String>,
    identifiers: Identifiers,
    /// The definitions that later types use, while they are visible.
    visible_types: Vec<VisibleType>,
    lines: Vec<Line>,
    column: usize,
    blocks: Vec<OpenBlock>,
    chains: Vec<Chain>,
    closes: Vec<usize>,
    temporaries: usize,
    block_functions: usize,
    body_functions: usize,
    /// The functions for blocks and bodies, each with its name and its
    /// lines, in the order they end.
    functions: Vec<(String, Vec<Line>)>,
    /// The string constants, each with its lines.
    constants: Vec<Vec<Line>>,
    /// The types that signatures name.
    type_imports: BTreeSet<&'static str>,
    /// Whether a float literal is one that oxlint's `oxc/approx-constant`
    /// takes for a `Math` constant, so the program disables that rule.
    approx_constant: bool,
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
            visible_types: Vec::new(),
            lines: Vec::new(),
            column: BODY_COLUMN,
            blocks: Vec::new(),
            chains: Vec::new(),
            closes: matching_closes(&program.events),
            temporaries: 0,
            block_functions: 0,
            body_functions: 0,
            functions: Vec::new(),
            constants: Vec::new(),
            type_imports: BTreeSet::new(),
            approx_constant: false,
        }
    }

    fn render(mut self) -> String {
        let program = self.program;
        let (format, first) = match program.events.first() {
            Some(Event::Format(name)) => (Some(Value::string(name)), 1),
            _ => (None, 0),
        };
        self.builder_statement(format);
        self.handles();
        let mut index = first;
        while let Some(event) = program.events.get(index) {
            index = self.statement(event, index);
        }
        self.line("return b;".to_owned());
        let body = std::mem::take(&mut self.lines);
        self.finish(body)
    }

    /// `const b = new SpecBuilder(format);`.
    fn builder_statement(&mut self, mut format: Option<Value>) {
        loop {
            let text = match &format {
                Some(format) => format!("const b = new SpecBuilder({});", format.text),
                None => "const b = new SpecBuilder();".to_owned(),
            };
            match &mut format {
                Some(value) if !fits(self.column, &text) && !value.is_fixed() => self.hoist(value),
                _ => {
                    self.line(text);
                    return;
                }
            }
        }
    }

    /// The handles of value names and records, made before their definitions
    /// because a reference may come first in the text.
    fn handles(&mut self) {
        let program = self.program;
        for (index, name) in program.names.iter().enumerate() {
            let identifier = self.value_names[index].clone();
            let method = match name.kind {
                NameKind::Forward => "forward",
                NameKind::Undefined => "undefined",
            };
            self.handle_statement(&identifier, method, vec![Value::string(&name.text)]);
        }
        for (record, identifier) in program.records.iter().zip(self.record_names.clone()) {
            let arguments = vec![Value::string(&record.each), Value::string(&record.list)];
            self.handle_statement(&identifier, "forwardRecord", arguments);
        }
    }

    /// `const identifier = b.method(arguments);`, with the widest strings in
    /// constants until it fits.
    fn handle_statement(&mut self, identifier: &str, method: &str, mut arguments: Vec<Value>) {
        loop {
            let texts: Vec<&str> = arguments.iter().map(|value| value.text.as_str()).collect();
            let text = format!("const {identifier} = b.{method}({});", texts.join(", "));
            let widest = arguments
                .iter_mut()
                .filter(|value| !value.is_fixed())
                .max_by_key(|value| value.text.len());
            match widest {
                Some(widest) if !fits(self.column, &text) => self.hoist(widest),
                _ => {
                    self.line(text);
                    return;
                }
            }
        }
    }

    /// The program: its import, its constants, the builder function with
    /// `body`, the functions and the statement that prints the source.
    fn finish(self, body: Vec<Line>) -> String {
        let mut text = String::new();
        for lines in &self.constants {
            write_lines(&mut text, lines);
        }
        if !self.constants.is_empty() {
            text.push('\n');
        }
        text.push_str("export function builder(): SpecBuilder {\n");
        write_lines(&mut text, &body);
        text.push_str("}\n");
        // In the order of their numbers, which is that of the text for
        // blocks.
        let mut functions: Vec<&(String, Vec<Line>)> = self.functions.iter().collect();
        functions.sort_by_key(|(name, _)| {
            let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
            let number: usize = name[stem.len()..].parse().unwrap_or(0);
            (stem.to_owned(), number)
        });
        for (_, function) in functions {
            text.push('\n');
            write_lines(&mut text, function);
        }
        text.push_str(
            "\nif (import.meta.main) {\n  process.stdout.write(builder().source());\n}\n",
        );
        let used = identifiers_in(&text);
        let mut specifiers: Vec<String> = VALUE_EXPORTS
            .iter()
            .filter(|name| used.contains(**name))
            .map(|name| (*name).to_owned())
            .collect();
        specifiers.extend(self.type_imports.iter().map(|name| format!("type {name}")));
        let one_line = format!("import {{ {} }} from \"{MODULE}\";", specifiers.join(", "));
        // A spec's float is exact, so it stays a literal rather than
        // becoming the `Math` constant that the rule suggests.
        let mut program = if self.approx_constant {
            String::from("// oxlint-disable oxc/approx-constant\n")
        } else {
            String::new()
        };
        if fits(0, &one_line) {
            program.push_str(&format!("{one_line}\n"));
        } else {
            program.push_str("import {\n");
            for specifier in &specifiers {
                program.push_str(&format!("  {specifier},\n"));
            }
            program.push_str(&format!("}} from \"{MODULE}\";\n"));
        }
        program.push('\n');
        program.push_str(&text);
        program
    }

    fn line(&mut self, text: String) {
        self.lines.push(Line::new(self.column, text));
    }

    fn line_at(&mut self, column: usize, text: String) {
        self.lines.push(Line::new(column, text));
    }

    fn temporary(&mut self) -> String {
        self.temporaries += 1;
        format!("e{}", self.temporaries)
    }

    fn next_body(&mut self) -> String {
        self.body_functions += 1;
        format!("body{}", self.body_functions)
    }

    /// The widest expression the current block can hold on one line.
    fn budget(&self) -> usize {
        LINE_WIDTH
            .saturating_sub(self.column + LET_OVERHEAD)
            .max(NARROWEST_EXPRESSION)
    }

    /// A module constant that holds the string `literal`. oxfmt splits no
    /// string, so one too wide for its line stays whole on a line of its own.
    fn constant(&mut self, literal: &str) -> String {
        let name = format!("t{}", self.constants.len() + 1);
        let one_line = format!("const {name} = {literal};");
        let lines = if fits(0, &one_line) {
            vec![Line::new(0, one_line)]
        } else {
            vec![
                Line::new(0, format!("const {name} =")),
                Line::new(TAB, format!("{literal};")),
            ]
        };
        self.constants.push(lines);
        name
    }

    /// Move `value` into a local constant, or a string into a module
    /// constant, and leave its name in its place.
    fn hoist(&mut self, value: &mut Value) {
        let name = if matches!(value.form, Form::Str) {
            self.constant(&value.text)
        } else {
            let name = self.temporary();
            let hoisted = value.clone();
            self.let_statement(&name, &hoisted);
            name
        };
        *value = value.renamed(name);
    }

    /// A function of the module for a block or a body: its signature, then
    /// `body`. `tail` follows the parameters.
    fn function_declaration(
        &mut self,
        name: &str,
        parameters: &[Capture],
        tail: &str,
        body: Vec<Line>,
    ) {
        for parameter in parameters {
            for word in parameter
                .ts_type
                .split(|c: char| !c.is_ascii_alphanumeric())
            {
                if let Some(name) = TYPE_EXPORTS.iter().find(|name| **name == word) {
                    self.type_imports.insert(name);
                }
            }
        }
        let texts: Vec<String> = parameters
            .iter()
            .map(|parameter| format!("{}: {}", parameter.identifier, parameter.ts_type))
            .collect();
        let head = format!("function {name}");
        let tail = format!("{tail} {{");
        let mut lines = call_lines("", &head, &texts, &tail, 0)
            .unwrap_or_else(|| vec![Line::new(0, format!("{head}({}){tail}", texts.join(", ")))]);
        lines.extend(body);
        lines.push(Line::new(0, "}".to_owned()));
        self.functions.push((name.to_owned(), lines));
    }

    /// Note that the current statement uses `capture`, a constant of the
    /// function `function` levels of block functions in, so every block
    /// function inside that one takes it.
    fn use_local(&mut self, capture: &Capture, function: usize) {
        let outlined = self
            .blocks
            .iter_mut()
            .filter_map(|block| block.outlined.as_mut());
        for outlined in outlined.skip(function) {
            if !outlined.parameters.contains(capture) {
                outlined.parameters.push(capture.clone());
            }
        }
    }

    /// The handle of `name`, a constant of the builder function that every
    /// block function takes.
    fn handle(&mut self, name: NameId) -> Value {
        let identifier = self.value_names[name.0].clone();
        self.use_local(&Capture::new(&identifier, "DynField"), 0);
        Value::handle(identifier)
    }

    /// The record of `record`, a constant of the builder function that every
    /// block function takes.
    fn record(&mut self, record: RecordId) -> Value {
        let identifier = self.record_names[record.0].clone();
        self.use_local(&Capture::new(&identifier, "Record"), 0);
        Value::name(identifier, Class::Record)
    }

    /// The number of block functions around the current statement.
    fn function_depth(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block| block.outlined.is_some())
            .count()
    }

    /// The scope that the current statement is a call on.
    fn receiver(&self) -> String {
        self.blocks
            .last()
            .map_or_else(|| "b".to_owned(), |block| block.receiver.clone())
    }

    /// The arrow functions around the current statement in its function.
    fn depth(&self) -> usize {
        self.blocks.last().map_or(0, |block| block.depth)
    }

    /// The local constant of the visible type `name`.
    fn type_reference(&mut self, name: &str) -> String {
        let visible = self
            .visible_types
            .iter()
            .rev()
            .find(|ty| ty.name == name)
            .expect("a type that a type uses is visible");
        let capture = Capture {
            identifier: visible.variable.clone(),
            ts_type: visible.ts_type.to_owned(),
        };
        let function = visible.function;
        self.use_local(&capture, function);
        capture.identifier
    }

    fn target(&mut self, target: &Target) -> Value {
        match target {
            Target::Name(name) => self.handle(*name),
            Target::Discard => Value::call("discard", Vec::new(), Class::Other),
        }
    }

    fn strings(texts: &[String]) -> Vec<Value> {
        texts.iter().map(|text| Value::string(text)).collect()
    }

    /// `{ key: value, ... }`, with the widest strings in constants while it
    /// is wider than an expression's budget.
    fn object(&mut self, entries: Vec<(&'static str, Value)>) -> Value {
        self.object_within(entries, self.budget())
    }

    /// The object of `entries`, with the widest strings in constants while
    /// it is wider than `width`.
    fn object_within(&mut self, mut entries: Vec<(&'static str, Value)>, width: usize) -> Value {
        loop {
            let texts: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{key}: {}", value.text))
                .collect();
            let object = Value::object(texts);
            let widest = entries
                .iter_mut()
                .map(|(_, value)| value)
                .filter(|value| matches!(value.form, Form::Str))
                .max_by_key(|value| value.text.len());
            match widest {
                Some(widest) if object.text.len() > width => self.hoist(widest),
                _ => return object,
            }
        }
    }

    /// One string, or an array of several, with the widest strings in
    /// constants while it is wider than `width`.
    fn string_or_array(&mut self, texts: &[String], width: usize) -> Value {
        let mut values = Self::strings(texts);
        if values.len() == 1 {
            return values.remove(0);
        }
        loop {
            let array = Value::array(&values);
            let widest = values
                .iter_mut()
                .filter(|value| matches!(value.form, Form::Str))
                .max_by_key(|value| value.text.len());
            match widest {
                Some(widest) if array.text.len() > width => self.hoist(widest),
                _ => return array,
            }
        }
    }

    /// The entries of an `until` clause or of `repeatUntil`'s options.
    fn until(
        &mut self,
        terminators: &[String],
        store: Option<NameId>,
        width: usize,
    ) -> Vec<(&'static str, Value)> {
        let store = store.map(|store| self.handle(store));
        // The array of terminators shares the object's line with the rest
        // of it.
        let around = "{ until:  }".len()
            + store
                .as_ref()
                .map_or(0, |store| ", store: ".len() + store.text.len());
        let until = self.string_or_array(terminators, width.saturating_sub(around));
        let mut entries = vec![("until", until)];
        entries.extend(store.map(|store| ("store", store)));
        entries
    }

    /// Render the event at `index` and return the next event to render.
    fn statement(&mut self, event: &'a Event, index: usize) -> usize {
        match event {
            Event::Format(name) => self.call_statement("format", vec![Value::string(name)]),
            Event::Comment(markers) => self.call_statement("comment", Self::strings(markers)),
            Event::Stop(markers) => self.call_statement("stop", Self::strings(markers)),
            Event::Tokens(text) => self.call_statement("tokens", vec![Value::string(text)]),
            Event::Separators(text) => {
                self.call_statement("separators", vec![Value::string(text)]);
            }
            Event::IgnoreTrailing => self.call_statement("ignoreTrailing", Vec::new()),
            Event::Read { target, declared } => {
                let target = self.target(target);
                let reading = self.declared(declared);
                self.call_statement("read", vec![target, reading]);
            }
            Event::List {
                target,
                declared,
                end,
            } => {
                let target = self.target(target);
                let reading = self.list_reading(declared, end);
                self.call_statement("list", vec![target, reading]);
            }
            Event::Let { target, value } => {
                let target = self.handle(*target);
                let value = self.expr(*value);
                self.call_statement("let", vec![target, value]);
            }
            Event::Param { target, default } => {
                let target = self.handle(*target);
                let mut arguments = vec![target];
                if let Some(default) = default {
                    arguments.push(self.expr(*default));
                }
                self.call_statement("param", arguments);
            }
            Event::DefineType { name, ty, used } => self.define_type(name, ty, *used),
            Event::Check { condition, message } => {
                let mut arguments = vec![self.expr(*condition)];
                if let Some(message) = message {
                    arguments.push(Value::string(message));
                }
                self.call_statement("check", arguments);
            }
            Event::Expect(words) => self.call_statement("expect", Self::strings(words)),
            Event::Line(pattern) => {
                let pattern = self.pattern_argument(pattern);
                self.call_statement("line", vec![pattern]);
            }
            Event::Find(pattern) => {
                let pattern = self.pattern_argument(pattern);
                self.call_statement("find", vec![pattern]);
            }
            Event::SkipLine => self.call_statement("skipLine", Vec::new()),
            Event::SkipLines(count) => {
                let count = self.expr(*count).integral();
                self.call_statement("skipLines", vec![count]);
            }
            Event::Index(position) => {
                let position = self.expr(*position).integral();
                self.call_statement("index", vec![position]);
            }
            Event::Repeat { count, names } => return self.repeat(count, names, index),
            Event::Sections(clauses) => return self.sections(clauses, index),
            Event::Arm {
                pattern,
                optional,
                last,
                block,
            } => return self.arm(pattern, *optional, *last, *block, index),
            Event::If(condition) => {
                let condition = self.expr(*condition);
                let total = self.chain_length(index);
                let chained = total > 1;
                if chained {
                    self.chains.push(Chain {
                        receiver: self.receiver(),
                        column: self.column,
                        links: Vec::new(),
                        total,
                    });
                }
                return self.open_block("if", vec![condition], index, Vec::new(), chained);
            }
            Event::ElseIf(condition) => {
                let condition = self.expr(*condition);
                return self.open_block("elseIf", vec![condition], index, Vec::new(), true);
            }
            Event::Else => return self.open_block("else", Vec::new(), index, Vec::new(), true),
            Event::ElseRead { target, declared } => {
                let target = self.target(target);
                let reading = self.declared(declared);
                self.add_link(Link {
                    method: "elseReadIf",
                    arguments: vec![target, reading],
                    block: None,
                });
            }
            Event::Instances => {
                return self.open_block("instances", Vec::new(), index, Vec::new(), false);
            }
            Event::Close => self.close_block(),
        }
        index + 1
    }

    /// `type name = ty`, in a local constant if a later type uses it.
    fn define_type(&mut self, name: &str, ty: &crate::program::Type, used: bool) {
        let reading = self.reading(ty);
        let mut arguments = vec![Value::string(name), reading];
        if !used {
            self.call_statement("defineType", arguments);
            return;
        }
        // `iris check` refuses a definition of a visible type, so no local
        // constant hides another.
        let identifier = self.identifiers.named(Role::Type, name);
        let ts_type = self.type_class(ty);
        let callee = format!("{}.defineType", self.receiver());
        let head = format!("const {identifier} = ");
        loop {
            let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
            let one_line = format!("{head}{callee}({});", texts.join(", "));
            if fits(self.column, &one_line) {
                self.line(one_line);
                break;
            }
            if !self.shrink_widest(&mut arguments, self.column + TAB) {
                let column = self.column;
                let lines = call_lines(&head, &callee, &texts, ";", column)
                    .unwrap_or_else(|| vec![Line::new(column, one_line)]);
                self.lines.extend(lines);
                break;
            }
        }
        self.visible_types.push(VisibleType {
            name: name.to_owned(),
            variable: identifier,
            ts_type,
            function: self.function_depth(),
        });
    }

    /// The pattern of `line`, `find` or an arm.
    fn pattern_argument(&mut self, pattern: &[Piece]) -> Value {
        if pattern.is_empty() {
            Value::string("")
        } else {
            self.pattern(pattern)
        }
    }

    /// The links of the `if` at `index`: its own, one for each `else if`
    /// and `else`, and one for a declaration after `else`.
    fn chain_length(&self, index: usize) -> usize {
        let events = &self.program.events;
        let mut total = 1;
        let mut next = self.closes[index] + 1;
        loop {
            match events.get(next) {
                Some(Event::ElseIf(_) | Event::Else) => {
                    total += 1;
                    next = self.closes[next] + 1;
                }
                Some(Event::ElseRead { .. }) => return total + 1,
                _ => return total,
            }
        }
    }

    /// Add a link to the innermost chain, and write the chain if it was the
    /// last.
    fn add_link(&mut self, link: Link) {
        let chain = self.chains.last_mut().expect("an else is inside a chain");
        chain.links.push(link);
        if chain.links.len() == chain.total {
            let chain = self.chains.pop().expect("the chain is open");
            self.column = chain.column;
            self.chain_statement(&chain.receiver, chain.links);
        }
    }

    /// Open the block of the event at `index`, whose opener is
    /// `method(arguments, (scope) => { ... })`, and whose statements start
    /// with the calls `names`.
    fn open_block(
        &mut self,
        method: &'static str,
        arguments: Vec<Value>,
        index: usize,
        names: Vec<(&'static str, Value)>,
        chained: bool,
    ) -> usize {
        let close = self.closes[index];
        if close == index + 1 && names.is_empty() {
            let empty = Value::name("() => {}".to_owned(), Class::Other);
            if chained {
                self.add_link(Link {
                    method,
                    arguments,
                    block: Some(Block {
                        parameter: String::new(),
                        body: Vec::new(),
                    }),
                });
            } else if body_is_optional(method) {
                self.call_statement(method, arguments);
            } else {
                let mut arguments = arguments;
                arguments.push(empty);
                self.call_statement(method, arguments);
            }
            return close + 1;
        }
        let column = self.column;
        let kind = ScopeKind::of(method);
        let link = self.chains.last().map_or(0, |chain| chain.links.len());
        let total = self.chains.last().map_or(0, |chain| chain.total);
        let body_column = if chained && total > 2 && link > 0 {
            column + 2 * TAB
        } else {
            column + TAB
        };
        let depth = self.depth() + 1;
        let parameter = format!("{}{depth}", kind.prefix());
        let outlined = (depth > DEEPEST_SCOPE || body_column > DEEPEST_BODY).then(|| {
            self.block_functions += 1;
            Outlined {
                function: format!("block{}", self.block_functions),
                parameters: Vec::new(),
            }
        });
        let (receiver, inner_depth) = if outlined.is_some() {
            (format!("{}1", kind.prefix()), 1)
        } else {
            (parameter.clone(), depth)
        };
        self.column = if outlined.is_some() {
            BODY_COLUMN
        } else {
            body_column
        };
        self.blocks.push(OpenBlock {
            method,
            arguments,
            kind,
            parameter,
            receiver,
            depth: inner_depth,
            outer: std::mem::take(&mut self.lines),
            column,
            body_column,
            types: self.visible_types.len(),
            outlined,
            chained,
        });
        for (method, argument) in names {
            if argument.class == Class::Record {
                self.use_local(&Capture::new(&argument.text, "Record"), 0);
            }
            self.call_statement(method, vec![argument]);
        }
        index + 1
    }

    fn close_block(&mut self) {
        let block = self.blocks.pop().expect("every close has an open block");
        let body = std::mem::replace(&mut self.lines, block.outer);
        self.visible_types.truncate(block.types);
        let body = match block.outlined {
            None => body,
            Some(outlined) => {
                let mut parameters = vec![Capture {
                    identifier: format!("{}1", block.kind.prefix()),
                    ts_type: block.kind.class().to_owned(),
                }];
                parameters.extend(outlined.parameters);
                self.function_declaration(&outlined.function, &parameters, ": void", body);
                // The block's arrow function only calls the function.
                let mut arguments = vec![block.parameter.clone()];
                arguments.extend(
                    parameters[1..]
                        .iter()
                        .map(|parameter| parameter.identifier.clone()),
                );
                let column = block.body_column;
                call_lines("", &outlined.function, &arguments, ";", column).unwrap_or_else(|| {
                    vec![Line::new(
                        column,
                        format!("{}({});", outlined.function, arguments.join(", ")),
                    )]
                })
            }
        };
        self.column = block.column;
        let link = Link {
            method: block.method,
            arguments: block.arguments,
            block: Some(Block {
                parameter: block.parameter,
                body,
            }),
        };
        if block.chained {
            self.add_link(link);
        } else {
            let receiver = self.receiver();
            self.chain_statement(&receiver, vec![link]);
        }
    }

    fn repeat(&mut self, count: &RepeatCount, names: &RepeatNames, index: usize) -> usize {
        let (method, arguments) = match count {
            RepeatCount::Times(count) => ("repeat", vec![self.expr(*count).integral()]),
            RepeatCount::UntilEof => ("repeatUntilEof", Vec::new()),
            RepeatCount::Until { terminators, store } => {
                let mut arguments = vec![self.string_or_array(terminators, self.budget())];
                if let Some(store) = store {
                    let store = self.handle(*store);
                    arguments.push(self.object(vec![("store", store)]));
                }
                ("repeatUntil", arguments)
            }
        };
        let names = match names {
            // The record is a statement of the block, which takes it if it
            // is a function.
            RepeatNames::Forward(record) => {
                let record = Value::name(self.record_names[record.0].clone(), Class::Record);
                vec![("recordAs", record)]
            }
            RepeatNames::InPlace { each, list } => {
                let each = each.iter().map(|name| ("eachName", Value::string(name)));
                let list = list.iter().map(|name| ("asName", Value::string(name)));
                each.chain(list).collect()
            }
        };
        self.open_block(method, arguments, index, names, false)
    }

    fn sections(&mut self, clauses: &[SectionClause], index: usize) -> usize {
        let skip = || ("skipOther", Value::name("true".to_owned(), Class::Other));
        let arguments = match clauses {
            [] => Vec::new(),
            [SectionClause::SkipOther] => vec![self.object(vec![skip()])],
            [SectionClause::Until { terminators, store }] => {
                let width = self.budget();
                let entries = self.until(terminators, *store, width);
                vec![self.object_within(entries, width)]
            }
            [
                SectionClause::Until { terminators, store },
                SectionClause::SkipOther,
            ] => {
                let width = self.budget();
                let until = width.saturating_sub(", skipOther: true".len());
                let mut entries = self.until(terminators, *store, until);
                entries.push(skip());
                vec![self.object_within(entries, width)]
            }
            clauses => {
                let mut objects = Vec::new();
                // A list too wide for its line puts each object on a line of
                // its own, further in and with a comma.
                let width = self.budget().saturating_sub(TAB + 1);
                for clause in clauses {
                    let entries = match clause {
                        SectionClause::Until { terminators, store } => {
                            self.until(terminators, *store, width)
                        }
                        SectionClause::SkipOther => vec![skip()],
                    };
                    objects.push(self.object_within(entries, width));
                }
                vec![Value::array(&objects)]
            }
        };
        self.open_block("sections", arguments, index, Vec::new(), false)
    }

    /// An arm: `arm(pattern, options)` without a block, and with the block's
    /// arrow function last with one.
    fn arm(
        &mut self,
        pattern: &[Piece],
        optional: bool,
        last: bool,
        block: bool,
        index: usize,
    ) -> usize {
        let mut arguments = vec![self.pattern_argument(pattern)];
        let flag = || Value::name("true".to_owned(), Class::Other);
        let mut options = Vec::new();
        if optional {
            options.push(("optional", flag()));
        }
        if last {
            options.push(("last", flag()));
        }
        if !options.is_empty() {
            arguments.push(self.object(options));
        }
        if !block {
            self.call_statement("arm", arguments);
            return index + 1;
        }
        self.open_block("arm", arguments, index, Vec::new(), false)
    }
}

/// The identifiers of `text` that are not members, string contents or
/// template text, so that the program imports what it uses.
fn identifiers_in(text: &str) -> BTreeSet<&str> {
    #[derive(PartialEq)]
    enum State {
        Code,
        Template,
    }
    let bytes = text.as_bytes();
    let mut found = BTreeSet::new();
    // The states around the current one, and the braces open in each
    // substitution.
    let mut states = vec![State::Code];
    let mut braces = vec![0usize];
    let mut position = 0;
    let mut previous = b' ';
    while position < bytes.len() {
        let byte = bytes[position];
        match states.last().expect("a state is open") {
            State::Template => match byte {
                b'\\' => position += 1,
                b'`' => {
                    states.pop();
                    previous = b'`';
                }
                b'$' if bytes.get(position + 1) == Some(&b'{') => {
                    position += 1;
                    states.push(State::Code);
                    braces.push(0);
                    previous = b'{';
                }
                _ => {}
            },
            State::Code => match byte {
                b'"' | b'\'' => {
                    position += 1;
                    while position < bytes.len() && bytes[position] != byte {
                        if bytes[position] == b'\\' {
                            position += 1;
                        }
                        position += 1;
                    }
                    previous = byte;
                }
                b'`' => states.push(State::Template),
                b'{' => {
                    *braces.last_mut().expect("a count is open") += 1;
                    previous = byte;
                }
                b'}' => {
                    let open = braces.last_mut().expect("a count is open");
                    if *open == 0 && states.len() > 1 {
                        states.pop();
                        braces.pop();
                    } else {
                        *open = open.saturating_sub(1);
                    }
                    previous = byte;
                }
                byte if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' => {
                    let start = position;
                    while position + 1 < bytes.len()
                        && (bytes[position + 1].is_ascii_alphanumeric()
                            || bytes[position + 1] == b'_'
                            || bytes[position + 1] == b'$')
                    {
                        position += 1;
                    }
                    if previous != b'.' {
                        found.insert(&text[start..=position]);
                    }
                    previous = b'a';
                }
                byte if byte.is_ascii_whitespace() => {}
                byte => previous = byte,
            },
        }
        position += 1;
    }
    found
}

/// Append `lines` to `text`.
fn write_lines(text: &mut String, lines: &[Line]) {
    for line in lines {
        for _ in 0..line.column {
            text.push(' ');
        }
        text.push_str(&line.text);
        text.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::{identifiers_in, quoted};

    #[test]
    fn strings_are_ascii_with_unicode_escapes() {
        assert_eq!(quoted("a"), "\"a\"");
        assert_eq!(quoted("it's"), "\"it's\"");
        assert_eq!(quoted("say \"hi\""), "'say \"hi\"'");
        assert_eq!(quoted("\"a\" 'b'"), "\"\\\"a\\\" 'b'\"");
        assert_eq!(quoted("tab\there\\"), "\"tab\\there\\\\\"");
        assert_eq!(quoted("line\nfeed\r"), "\"line\\nfeed\\r\"");
        assert_eq!(quoted("é\u{7f}\0"), "\"\\u00e9\\u007f\\u0000\"");
        assert_eq!(quoted("😀"), "\"\\ud83d\\ude00\"");
        assert_eq!(quoted("$x ${y}"), "\"$x ${y}\"");
    }

    #[test]
    fn identifiers_skip_members_strings_and_template_text() {
        let text = "s1.line(p`int ${bind(n, int)} \\` lit`); x.sum(\"min\", 'max', { a: count });";
        let found: Vec<&str> = identifiers_in(text).into_iter().collect();
        assert_eq!(found, ["a", "bind", "count", "int", "n", "p", "s1", "x"]);
    }
}
