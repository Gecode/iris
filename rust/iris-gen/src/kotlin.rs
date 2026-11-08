//! The Kotlin backend: a program that calls the builder of
//! `dev.gecode.iris.kotlin.build`.
//!
//! The output is what ktfmt writes in its kotlinlang style, without ktfmt: a
//! statement that is too wide goes into the layouts that it gives calls, and
//! an argument too wide for those goes into a local value or, for a string,
//! a constant first. Blocks are lambdas with receivers, and a block whose
//! statements would start too far right becomes an extension function of its
//! scope class, which takes the local values it uses. A comprehension's body
//! that needs statements or is too wide for its lambda becomes a function
//! that takes the variables it uses.
//! The events are walked with a loop and a stack of open blocks, so no
//! recursion depends on the spec.
mod expr;
mod imports;
mod layout;
mod names;
mod types;

use std::collections::BTreeSet;

use crate::Backend;
use crate::program::{
    Event, NameId, NameKind, Piece, Program, RecordId, RepeatCount, RepeatNames, SectionClause,
    Target,
};
use expr::{Capture, Class, Form, Value};
use imports::builder_imports;
use layout::{assignment_lines, call_lines};
use names::{Identifiers, Role};

/// ktfmt's line width.
const LINE_WIDTH: usize = 100;
/// One indentation level, and ktfmt's continuation indent.
const TAB: usize = 4;
/// The column of a function's statements.
const BODY_COLUMN: usize = 4;
/// What `val eN = ` takes from a line.
const LET_OVERHEAD: usize = "val e0000 = ".len();
/// The narrowest budget for an expression, however deep the block.
const NARROWEST_EXPRESSION: usize = 24;
/// The column after which a block's statements go to a function of their own.
const DEEPEST_BODY: usize = 36;
/// The first line of the builder function.
const SPEC_BUILDER: &str = "fun specBuilder(): IrisBuilder<Unit> =";

/// A string as a Kotlin literal, in ASCII.
///
/// ktfmt measures a line in characters, and the round trip compares bytes,
/// so everything outside printable ASCII is a `\u` escape of its UTF-16
/// units. A `$` would start a template, so it has an escape of its own.
fn quoted(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for character in text.chars() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
            '$' => literal.push_str("\\$"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            ' '..='~' => literal.push(character),
            _ => {
                let mut units = [0; 2];
                for unit in character.encode_utf16(&mut units) {
                    literal.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    literal.push('"');
    literal
}

pub(crate) struct KotlinBackend;

impl Backend for KotlinBackend {
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

/// The receiver of a block's lambda: what the builder's scope classes say a
/// definition inside it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// `SpecScope`, the top level.
    Spec,
    /// `Scope0` to `Scope3` and `DeepScope`, or their optional forms.
    Block { depth: usize, optional: bool },
    /// The arm list of `sections`, which is no scope of its own.
    Arms { depth: usize, optional: bool },
}

impl Scope {
    fn depth_and_optional(self) -> (usize, bool) {
        match self {
            Self::Spec => (0, false),
            Self::Block { depth, optional } | Self::Arms { depth, optional } => (depth, optional),
        }
    }

    /// The scope of a block that `callee` opens inside this one.
    fn inner(self, callee: &str) -> Self {
        let (depth, optional) = self.depth_and_optional();
        match callee {
            "repeat" | "repeatUntil" | "repeatUntilEof" => Self::Block {
                depth: depth + 1,
                optional,
            },
            "ifThen" | "elseIf" | "otherwise" | "optionalArm" => Self::Block {
                depth,
                optional: true,
            },
            "sections" => Self::Arms { depth, optional },
            "instances" => Self::Block {
                depth: 0,
                optional: false,
            },
            _ => Self::Block { depth, optional },
        }
    }

    /// The Kotlin class of the scope.
    fn class(self) -> String {
        let (depth, optional) = self.depth_and_optional();
        let prefix = if optional { "Optional" } else { "" };
        match self {
            Self::Spec => "SpecScope".to_owned(),
            Self::Block { .. } | Self::Arms { .. } if depth > 3 => format!("{prefix}DeepScope"),
            Self::Block { .. } | Self::Arms { .. } => format!("{prefix}Scope{depth}"),
        }
    }
}

/// A type definition that a later type uses, while it is visible.
#[derive(Debug)]
struct VisibleType {
    name: String,
    variable: String,
    kotlin_type: &'static str,
    /// The number of block functions around the definition.
    function: usize,
}

/// A block whose statements go to a function of their own.
#[derive(Debug)]
struct Outlined {
    function: String,
    /// The local values of enclosing blocks that the block uses.
    parameters: Vec<Capture>,
}

/// The call that opens a block, written when the block closes.
#[derive(Debug)]
struct OpenBlock {
    callee: &'static str,
    arguments: Vec<Value>,
    /// The scope of the block's statements.
    scope: Scope,
    /// The lines before the block's opener.
    outer: Vec<Line>,
    /// The column of the opener.
    column: usize,
    /// The number of visible types before the block.
    types: usize,
    outlined: Option<Outlined>,
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
    /// Whether each value name is a local value of the builder's lambda
    /// rather than a top-level property.
    locals: Vec<bool>,
    /// The definitions that later types use, while they are visible.
    visible_types: Vec<VisibleType>,
    lines: Vec<Line>,
    column: usize,
    blocks: Vec<OpenBlock>,
    closes: Vec<usize>,
    temporaries: usize,
    block_functions: usize,
    body_functions: usize,
    /// The functions for blocks and bodies, each with its lines, in the order
    /// they end.
    functions: Vec<Vec<Line>>,
    /// The string constants, each with its lines.
    constants: Vec<Vec<Line>>,
    /// The handle properties, each with its lines.
    properties: Vec<Vec<Line>>,
    /// The simple names of the classes of `dev.gecode.iris.build` that the
    /// program names.
    imports: BTreeSet<String>,
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
        let locals = program
            .names
            .iter()
            .map(|name| name.kind == NameKind::Undefined || name.text == "_")
            .collect();
        Self {
            program,
            value_names,
            record_names,
            variable_names,
            identifiers,
            locals,
            visible_types: Vec::new(),
            lines: Vec::new(),
            column: BODY_COLUMN,
            blocks: Vec::new(),
            closes: matching_closes(&program.events),
            temporaries: 0,
            block_functions: 0,
            body_functions: 0,
            functions: Vec::new(),
            constants: Vec::new(),
            properties: Vec::new(),
            imports: BTreeSet::new(),
        }
    }

    fn render(mut self) -> String {
        let program = self.program;
        let (format, first) = match program.events.first() {
            Some(Event::Format(name)) => (Some(Value::string(name)), 1),
            _ => (None, 0),
        };
        // With a format, ktfmt puts the call on a line of its own.
        let empty = program.events.len() == first;
        if format.is_some() && !empty {
            self.column += TAB;
        }
        self.handles();
        let mut index = first;
        while let Some(event) = program.events.get(index) {
            index = self.statement(event, index);
        }
        let body = std::mem::take(&mut self.lines);
        let start = self.start(format, body);
        self.finish(start)
    }

    /// The properties of the handles of value names and records, made before
    /// their definitions because a reference may come first in the text, and
    /// the local values of names that nothing defines.
    fn handles(&mut self) {
        let program = self.program;
        for (index, name) in program.names.iter().enumerate() {
            let identifier = self.value_names[index].clone();
            let text = Value::string(&name.text);
            if name.kind == NameKind::Undefined {
                let value = Value::call("undefined", vec![text], Class::Handle);
                self.let_statement(&identifier, value);
            } else if self.locals[index] {
                let value = Value::call("forwardDynamic", vec![text], Class::Handle);
                self.let_statement(&identifier, value);
            } else {
                self.property(&identifier, "dynamic", vec![text]);
            }
        }
        for (record, identifier) in program.records.iter().zip(self.record_names.clone()) {
            let arguments = vec![Value::string(&record.each), Value::string(&record.list)];
            self.property(&identifier, "recordName", arguments);
        }
    }

    /// `private val identifier = callee(arguments)`, with the widest strings
    /// in constants until it fits.
    fn property(&mut self, identifier: &str, callee: &str, mut arguments: Vec<Value>) {
        let head = format!("private val {identifier} = ");
        loop {
            let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
            let widest = arguments
                .iter_mut()
                .filter(|value| matches!(value.form, Form::Str))
                .max_by_key(|value| value.text.len());
            match assignment_lines(&head, callee, &texts, 0) {
                Some(lines) if lines.len() <= 2 => {
                    self.properties.push(lines);
                    return;
                }
                found => match widest {
                    Some(widest) => self.hoist(widest),
                    None => {
                        let lines = found.unwrap_or_else(|| {
                            vec![Line::new(
                                0,
                                format!("{head}{callee}({})", texts.join(", ")),
                            )]
                        });
                        self.properties.push(lines);
                        return;
                    }
                },
            }
        }
    }

    /// The lines of `specBuilder`, around `body`.
    fn start(&mut self, format: Option<Value>, body: Vec<Line>) -> Vec<Line> {
        let Some(mut format) = format else {
            if body.is_empty() {
                return vec![Line::new(0, format!("{SPEC_BUILDER} irisBuilder {{}}"))];
            }
            let mut lines = vec![Line::new(0, format!("{SPEC_BUILDER} irisBuilder {{"))];
            lines.extend(body);
            lines.push(Line::new(0, "}".to_owned()));
            return lines;
        };
        if body.is_empty() {
            loop {
                let text = format!("{SPEC_BUILDER} irisBuilder({}) {{}}", format.text);
                if fits(0, &text) || !matches!(format.form, Form::Str) {
                    return vec![Line::new(0, text)];
                }
                self.hoist(&mut format);
            }
        }
        loop {
            let call = format!("irisBuilder({}) {{", format.text);
            if fits(TAB, &call) || !matches!(format.form, Form::Str) {
                let mut lines = vec![Line::new(0, SPEC_BUILDER.to_owned()), Line::new(TAB, call)];
                lines.extend(body);
                lines.push(Line::new(TAB, "}".to_owned()));
                return lines;
            }
            self.hoist(&mut format);
        }
    }

    /// The program: its imports, then its members, `specBuilder` with the
    /// lines `start`, its functions and `main`.
    fn finish(self, start: Vec<Line>) -> String {
        let mut text = String::new();
        let mut members = false;
        for lines in self.constants.iter().chain(&self.properties) {
            write_lines(&mut text, lines);
            members = true;
        }
        if members {
            text.push('\n');
        }
        write_lines(&mut text, &start);
        for function in &self.functions {
            text.push('\n');
            write_lines(&mut text, function);
        }
        text.push_str("\nfun main() {\n    print(specBuilder().source())\n}\n");
        let mut program = String::new();
        for name in &self.imports {
            program.push_str(&format!("import dev.gecode.iris.build.{name}\n"));
        }
        for name in builder_imports(&text) {
            program.push_str(&format!("import dev.gecode.iris.kotlin.build.{name}\n"));
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

    /// The widest expression a block at `column` can hold on one line.
    fn budget_at(column: usize) -> usize {
        LINE_WIDTH
            .saturating_sub(column + LET_OVERHEAD)
            .max(NARROWEST_EXPRESSION)
    }

    /// The widest expression the current block can hold on one line.
    fn budget(&self) -> usize {
        Self::budget_at(self.column)
    }

    /// A constant that holds the string `literal`. ktfmt splits no string,
    /// so one too wide for its line stays whole on a line of its own.
    fn constant(&mut self, literal: &str) -> String {
        let name = format!("T{}", self.constants.len() + 1);
        let head = format!("private const val {name} =");
        let one_line = format!("{head} {literal}");
        let lines = if fits(0, &one_line) {
            vec![Line::new(0, one_line)]
        } else {
            vec![Line::new(0, head), Line::new(TAB, literal.to_owned())]
        };
        self.constants.push(lines);
        name
    }

    /// Move `value` into a local value, or a string into a constant, and
    /// leave its name in its place.
    fn hoist(&mut self, value: &mut Value) {
        // `lit("…")` keeps its call and takes a constant for the string.
        if let Form::Call { callee, arguments } = &value.form
            && callee == "lit"
            && let [argument] = arguments.as_slice()
            && matches!(argument.form, Form::Str)
        {
            let name = self.constant(&argument.text);
            *value = Value::call("lit", vec![Value::name(name, Class::Str)], value.class);
            return;
        }
        let name = if matches!(value.form, Form::Str) {
            self.constant(&value.text)
        } else {
            let name = self.temporary();
            let hoisted = value.clone();
            self.let_statement(&name, hoisted);
            name
        };
        let literal = value.literal;
        *value = Value {
            literal,
            ..Value::name(name, value.class)
        };
    }

    /// A function for a block or a body: its signature, then `body`. `tail`
    /// follows the parameters.
    fn function_declaration(
        &mut self,
        head: &str,
        parameters: &[Capture],
        tail: &str,
        body: Vec<Line>,
    ) {
        for parameter in parameters {
            self.imports.insert(parameter.kotlin_type.clone());
        }
        let texts: Vec<String> = parameters
            .iter()
            .map(|parameter| format!("{}: {}", parameter.identifier, parameter.kotlin_type))
            .collect();
        let mut lines = call_lines("", head, &texts, tail, 0)
            .unwrap_or_else(|| vec![Line::new(0, format!("{head}({}){tail}", texts.join(", ")))]);
        lines.extend(body);
        lines.push(Line::new(0, "}".to_owned()));
        self.functions.push(lines);
    }

    /// Note that the current statement uses the local value `capture` of the
    /// function `function` levels of block functions in, so every block
    /// function between them takes it.
    fn use_local(&mut self, capture: &Capture, function: usize) {
        let outlined = self
            .blocks
            .iter_mut()
            .filter_map(|block| block.outlined.as_mut());
        for method in outlined.skip(function) {
            if !method.parameters.contains(capture) {
                method.parameters.push(capture.clone());
            }
        }
    }

    /// The local value of `name`, if it is one.
    fn local_capture(&self, name: NameId) -> Option<Capture> {
        self.locals[name.0].then(|| Capture {
            identifier: self.value_names[name.0].clone(),
            kotlin_type: "DynField".to_owned(),
        })
    }

    fn handle(&mut self, name: NameId) -> Value {
        if let Some(capture) = self.local_capture(name) {
            self.use_local(&capture, 0);
        }
        Value::name(self.value_names[name.0].clone(), Class::Handle)
    }

    /// The number of block functions around the current statement.
    fn function_depth(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block| block.outlined.is_some())
            .count()
    }

    /// The scope of the current statement.
    fn scope(&self) -> Scope {
        self.blocks.last().map_or(Scope::Spec, |block| block.scope)
    }

    /// The local value of the visible type `name`.
    fn type_reference(&mut self, name: &str) -> String {
        let visible = self
            .visible_types
            .iter()
            .rev()
            .find(|ty| ty.name == name)
            .expect("a type that a type uses is visible");
        let capture = Capture {
            identifier: visible.variable.clone(),
            kotlin_type: visible.kotlin_type.to_owned(),
        };
        let function = visible.function;
        self.use_local(&capture, function);
        capture.identifier
    }

    fn target(&mut self, target: &Target) -> Value {
        match target {
            Target::Name(name) => self.handle(*name),
            Target::Discard => Value::new("discard()".to_owned(), Class::Other),
        }
    }

    fn strings(texts: &[String]) -> Vec<Value> {
        texts.iter().map(|text| Value::string(text)).collect()
    }

    /// `method(arguments)`.
    fn simple(&mut self, method: &str, arguments: Vec<Value>) {
        self.call_statement(method, arguments, None);
    }

    /// Render the event at `index` and return the next event to render.
    fn statement(&mut self, event: &'a Event, index: usize) -> usize {
        match event {
            Event::Format(name) => self.simple("format", vec![Value::string(name)]),
            Event::Comment(markers) => self.simple("comment", Self::strings(markers)),
            Event::Stop(markers) => self.simple("stop", Self::strings(markers)),
            Event::Tokens(text) => self.simple("tokens", vec![Value::string(text)]),
            Event::Separators(text) => self.simple("separators", vec![Value::string(text)]),
            Event::IgnoreTrailing => self.simple("ignoreTrailing", Vec::new()),
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
                let value = self.expr(*value).expr();
                self.simple("let", vec![target, value]);
            }
            Event::Param { target, default } => {
                let target = self.handle(*target);
                let mut arguments = vec![target];
                if let Some(default) = default {
                    arguments.push(self.expr(*default).expr());
                }
                self.simple("param", arguments);
            }
            Event::DefineType { name, ty, used } => {
                let reading = self.reading(ty);
                let arguments = vec![Value::string(name), reading];
                if *used {
                    // `iris check` refuses a definition of a visible type, so
                    // no local value hides another.
                    let identifier = self.identifiers.named(Role::Type, name);
                    let kotlin_type = self.type_class(ty);
                    let value = Value::call("defineType", arguments, Class::Other);
                    self.let_statement(&identifier, value);
                    self.visible_types.push(VisibleType {
                        name: name.clone(),
                        variable: identifier,
                        kotlin_type,
                        function: self.function_depth(),
                    });
                } else {
                    self.simple("defineType", arguments);
                }
            }
            Event::Check { condition, message } => {
                let condition = self.expr(*condition).expr();
                let mut arguments = vec![condition];
                if let Some(message) = message {
                    arguments.push(Value::string(message));
                }
                self.simple("check", arguments);
            }
            Event::Expect(words) => self.simple("expect", Self::strings(words)),
            Event::Line(pattern) => {
                let parts = self.pattern_parts(pattern);
                self.simple("line", parts);
            }
            Event::Find(pattern) => {
                let parts = self.pattern_parts(pattern);
                self.simple("find", parts);
            }
            Event::SkipLine => self.simple("skipLine", Vec::new()),
            Event::SkipLines(count) => {
                let count = self.expr(*count).long_or_expr();
                self.simple("skipLines", vec![count]);
            }
            Event::Index(position) => {
                let position = self.expr(*position).long_or_expr();
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
            Event::If(condition) => {
                let condition = self.expr(*condition).expr();
                return self.open_block("ifThen", vec![condition], index);
            }
            Event::ElseIf(condition) => {
                let condition = self.expr(*condition).expr();
                return self.open_block("elseIf", vec![condition], index);
            }
            Event::Else => return self.open_block("otherwise", Vec::new(), index),
            Event::ElseRead { target, declared } => {
                let target = self.target(target);
                let reading = self.declared(declared);
                self.simple("elseReadIf", vec![target, reading]);
            }
            Event::Instances => return self.open_block("instances", Vec::new(), index),
            Event::Close => self.close_block(),
        }
        index + 1
    }

    /// Open the block of the event at `index`, whose opener is
    /// `callee(arguments) { ... }`.
    fn open_block(&mut self, callee: &'static str, arguments: Vec<Value>, index: usize) -> usize {
        let close = self.closes[index];
        if close == index + 1 {
            self.call_statement(callee, arguments, Some(Vec::new()));
            return close + 1;
        }
        let column = self.column;
        let scope = self.scope().inner(callee);
        let body_column = column + TAB;
        let outlined =
            (body_column > DEEPEST_BODY && !matches!(scope, Scope::Arms { .. })).then(|| {
                self.block_functions += 1;
                Outlined {
                    function: format!("block{}", self.block_functions),
                    parameters: Vec::new(),
                }
            });
        self.column = if outlined.is_some() {
            BODY_COLUMN
        } else {
            body_column
        };
        self.blocks.push(OpenBlock {
            callee,
            arguments,
            scope,
            outer: std::mem::take(&mut self.lines),
            column,
            types: self.visible_types.len(),
            outlined,
        });
        index + 1
    }

    fn close_block(&mut self) {
        let block = self.blocks.pop().expect("every close has an open block");
        let body = std::mem::replace(&mut self.lines, block.outer);
        self.column = block.column;
        self.visible_types.truncate(block.types);
        match block.outlined {
            None => self.call_statement(block.callee, block.arguments, Some(body)),
            Some(outlined) => {
                let head = format!("private fun {}.{}", block.scope.class(), outlined.function);
                let parameters = outlined.parameters;
                self.function_declaration(&head, &parameters, " {", body);
                let arguments = parameters
                    .iter()
                    .map(|parameter| Value::name(parameter.identifier.clone(), Class::Other))
                    .collect();
                // The block's lambda only calls the function.
                let outer = std::mem::take(&mut self.lines);
                self.column = block.column + TAB;
                self.call_statement(&outlined.function, arguments, None);
                let call = std::mem::replace(&mut self.lines, outer);
                self.column = block.column;
                self.call_statement(block.callee, block.arguments, Some(call));
            }
        }
    }

    /// `until(terminators)`, with `.store(store)` if it is set.
    fn until(&mut self, terminators: &[String], store: Option<NameId>) -> Value {
        let mut operands = Self::strings(terminators);
        let count = operands.len();
        if let Some(store) = store {
            operands.push(self.handle(store));
        }
        self.combine(operands, |values| {
            let until = Value::call("until", values[..count].to_vec(), Class::Other);
            match values.get(count) {
                Some(store) => until.method("store", std::slice::from_ref(store), Class::Other),
                None => until,
            }
        })
    }

    fn repeat(&mut self, count: &RepeatCount, names: &RepeatNames, index: usize) -> usize {
        let (callee, mut arguments) = match count {
            RepeatCount::Times(count) => ("repeat", vec![self.expr(*count).long_or_expr()]),
            RepeatCount::UntilEof => ("repeatUntilEof", Vec::new()),
            RepeatCount::Until { terminators, store } => {
                ("repeatUntil", vec![self.until(terminators, *store)])
            }
        };
        match names {
            RepeatNames::Forward(RecordId(record)) => {
                let record = Value::name(self.record_names[*record].clone(), Class::Record);
                arguments.push(record);
            }
            RepeatNames::InPlace {
                each: Some(each),
                list: Some(list),
            } => {
                let names = vec![Value::string(each), Value::string(list)];
                arguments.push(Value::call("recordName", names, Class::Record));
            }
            RepeatNames::InPlace { .. } => {}
        }
        self.open_block(callee, arguments, index)
    }

    fn sections(&mut self, clauses: &[SectionClause], index: usize) -> usize {
        let mut arguments = Vec::new();
        for clause in clauses {
            arguments.push(match clause {
                SectionClause::Until { terminators, store } => self.until(terminators, *store),
                SectionClause::SkipOther => Value::new("skipOther()".to_owned(), Class::Other),
            });
        }
        self.open_block("sections", arguments, index)
    }

    /// An arm: `arm(parts, optional = true, last = true)` without a block,
    /// and `arm` or `optionalArm` with one.
    fn arm(
        &mut self,
        pattern: &[Piece],
        optional: bool,
        last: bool,
        block: bool,
        index: usize,
    ) -> usize {
        let mut arguments = if pattern.is_empty() {
            vec![Value::string("")]
        } else {
            self.pattern_parts(pattern)
        };
        if optional && !block {
            arguments.push(Value::name("optional = true".to_owned(), Class::Other));
        }
        if last {
            arguments.push(Value::name("last = true".to_owned(), Class::Other));
        }
        if !block {
            self.call_statement("arm", arguments, None);
            return index + 1;
        }
        let callee = if optional { "optionalArm" } else { "arm" };
        self.open_block(callee, arguments, index)
    }
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
    use super::quoted;

    #[test]
    fn strings_are_ascii_with_unicode_escapes() {
        assert_eq!(quoted("a"), "\"a\"");
        assert_eq!(quoted("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(quoted("tab\there\\"), "\"tab\\there\\\\\"");
        assert_eq!(quoted("line\nfeed\r"), "\"line\\nfeed\\r\"");
        assert_eq!(quoted("é\u{7f}\0"), "\"\\u00e9\\u007f\\u0000\"");
        assert_eq!(quoted("😀"), "\"\\ud83d\\ude00\"");
        assert_eq!(quoted("$x ${y}"), "\"\\$x \\${y}\"");
    }
}
