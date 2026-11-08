//! The Java backend: a program that calls `dev.gecode.iris.build`.
//!
//! The output is what google-java-format writes, without google-java-format:
//! a statement that is too wide goes into the layouts that it gives calls, and
//! an argument too wide for those goes into a temporary or, for a string, a
//! class constant first. A block whose statements would start too far right
//! becomes a method of its own, which takes the local variables it uses, and
//! so does a comprehension's body that needs statements or is too wide for a
//! lambda. The events are walked with a loop and a stack of open blocks, so no
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
use layout::{assignment_lines, call_lines};
use names::{Identifiers, Role};

/// google-java-format's line width.
const LINE_WIDTH: usize = 100;
/// One indentation level.
const TAB: usize = 2;
/// google-java-format's continuation indent.
const CONTINUATION: usize = 4;
/// The column of a member of the class.
const MEMBER_COLUMN: usize = 2;
/// The column of a method's statements.
const BODY_COLUMN: usize = 4;
/// What `var eN = ` and `;` take from a line.
const LET_OVERHEAD: usize = "var e0000 = ;".len();
/// The narrowest budget for an expression, however deep the block.
const NARROWEST_EXPRESSION: usize = 24;
/// The column after which a block's statements go to a method of their own.
const DEEPEST_BODY: usize = 36;

/// A string as a Java literal, in ASCII.
///
/// google-java-format measures a line in characters, and the round trip
/// compares bytes, so everything outside printable ASCII is a `\u` escape of
/// its UTF-16 units. Java reads those escapes before it reads literals, so a
/// line break, a quote and a backslash have the escapes of their own.
fn quoted(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 2);
    literal.push('"');
    for character in text.chars() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '"' => literal.push_str("\\\""),
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

/// The escapes and characters of a literal's text, which a split keeps whole.
fn literal_units(inner: &str) -> Vec<&str> {
    let bytes = inner.as_bytes();
    let mut units = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let length = match (bytes[start], bytes.get(start + 1)) {
            (b'\\', Some(b'u')) => 6,
            (b'\\', _) => 2,
            _ => 1,
        };
        units.push(&inner[start..start + length]);
        start += length;
    }
    units
}

pub(crate) struct JavaBackend;

impl Backend for JavaBackend {
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

/// A type definition that a later type uses, while it is visible.
#[derive(Debug)]
struct VisibleType {
    name: String,
    variable: String,
    java_type: &'static str,
    /// The number of block methods around the definition.
    function: usize,
}

/// A block whose statements go to a method of their own.
#[derive(Debug)]
struct Outlined {
    function: String,
    /// The local variables of enclosing blocks that the block uses.
    parameters: Vec<Capture>,
}

/// The call that opens a block, written when the block closes.
#[derive(Debug)]
struct OpenBlock {
    callee: &'static str,
    arguments: Vec<Value>,
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
    /// Whether each value name is a local variable of `body` rather than a
    /// field.
    locals: Vec<bool>,
    /// The definitions that later types use, while they are visible.
    visible_types: Vec<VisibleType>,
    lines: Vec<Line>,
    column: usize,
    blocks: Vec<OpenBlock>,
    closes: Vec<usize>,
    temporaries: usize,
    block_methods: usize,
    body_methods: usize,
    /// The methods for blocks and bodies, each with its lines, in the order
    /// they end.
    methods: Vec<Vec<Line>>,
    /// The string constants, each with its lines.
    constants: Vec<Vec<Line>>,
    /// The handle fields, each with its lines.
    fields: Vec<Vec<Line>>,
    /// The simple names of the classes that the program uses.
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
            block_methods: 0,
            body_methods: 0,
            methods: Vec::new(),
            constants: Vec::new(),
            fields: Vec::new(),
            imports: BTreeSet::from(["Builder".to_owned()]),
        }
    }

    fn render(mut self) -> String {
        let program = self.program;
        let (format, first) = match program.events.first() {
            Some(Event::Format(name)) => (Some(Value::string(name)), 1),
            _ => (None, 0),
        };
        self.handles();
        let mut index = first;
        while let Some(event) = program.events.get(index) {
            index = self.statement(event, index);
        }
        let body = std::mem::take(&mut self.lines);
        let start = self.start(format);
        self.finish(start, body)
    }

    /// The fields of the handles of value names and records, made before
    /// their definitions because a reference may come first in the text, and
    /// the local variables of names that nothing defines.
    fn handles(&mut self) {
        let program = self.program;
        for (index, name) in program.names.iter().enumerate() {
            let identifier = self.value_names[index].clone();
            let text = Value::string(&name.text);
            if name.kind == NameKind::Undefined {
                let value = Value::call("undefined", vec![text], Class::Handle);
                self.let_statement(&identifier, value);
            } else if self.locals[index] {
                self.imports.insert("Dyn".to_owned());
                let kind = Value::name("Dyn.class".to_owned(), Class::Other);
                let value = Value::call("forward", vec![text, kind], Class::Handle);
                self.let_statement(&identifier, value);
            } else {
                self.imports.insert("DynField".to_owned());
                self.field("DynField", &identifier, "dynamic", vec![text]);
            }
        }
        for (record, identifier) in program.records.iter().zip(self.record_names.clone()) {
            self.imports.insert("RecordName".to_owned());
            let arguments = vec![Value::string(&record.each), Value::string(&record.list)];
            self.field("RecordName", &identifier, "recordName", arguments);
        }
    }

    /// `private static final java_type identifier = callee(arguments);`,
    /// with the widest strings in constants until it fits.
    fn field(
        &mut self,
        java_type: &str,
        identifier: &str,
        callee: &str,
        mut arguments: Vec<Value>,
    ) {
        let head = format!("private static final {java_type} {identifier} = ");
        loop {
            let texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
            let widest = arguments
                .iter_mut()
                .filter(|value| matches!(value.form, Form::Str))
                .max_by_key(|value| value.text.len());
            match assignment_lines(&head, callee, &texts, MEMBER_COLUMN) {
                Some(lines) if lines.len() <= 2 => {
                    self.fields.push(lines);
                    return;
                }
                found => match widest {
                    Some(widest) => self.hoist(widest),
                    None => {
                        let lines = found.unwrap_or_else(|| {
                            vec![Line::new(
                                MEMBER_COLUMN,
                                format!("{head}{callee}({});", texts.join(", ")),
                            )]
                        });
                        self.fields.push(lines);
                        return;
                    }
                },
            }
        }
    }

    /// The lines of `specBuilder`.
    fn start(&mut self, format: Option<Value>) -> Vec<Line> {
        let mut arguments: Vec<Value> = format.into_iter().collect();
        loop {
            let mut texts: Vec<String> = arguments.iter().map(|value| value.text.clone()).collect();
            texts.push("SpecBuilder::body".to_owned());
            if let Some(lines) = call_lines("return ", "builder", &texts, ";", BODY_COLUMN)
                && lines.len() == 1
            {
                return lines;
            }
            match arguments.first_mut() {
                Some(format) if matches!(format.form, Form::Str) => self.hoist(format),
                _ => {
                    let text = format!("return builder({});", texts.join(", "));
                    return vec![Line::new(BODY_COLUMN, text)];
                }
            }
        }
    }

    fn finish(self, start: Vec<Line>, body: Vec<Line>) -> String {
        let mut text = String::from("import static dev.gecode.iris.build.Iris.*;\n\n");
        let mut imports: Vec<String> = self
            .imports
            .iter()
            .map(|name| match name.as_str() {
                "List" => "java.util.List".to_owned(),
                name => format!("dev.gecode.iris.build.{name}"),
            })
            .collect();
        imports.sort();
        for import in imports {
            text.push_str(&format!("import {import};\n"));
        }
        text.push_str("\nfinal class SpecBuilder {\n");
        let mut members = false;
        for lines in self.constants.iter().chain(&self.fields) {
            write_lines(&mut text, lines);
            members = true;
        }
        if members {
            text.push('\n');
        }
        text.push_str("  private SpecBuilder() {}\n\n  static Builder specBuilder() {\n");
        write_lines(&mut text, &start);
        text.push_str("  }\n\n");
        if body.is_empty() {
            text.push_str("  private static void body() {}\n");
        } else {
            text.push_str("  private static void body() {\n");
            write_lines(&mut text, &body);
            text.push_str("  }\n");
        }
        for method in &self.methods {
            text.push('\n');
            write_lines(&mut text, method);
        }
        text.push_str(
            "\n  public static void main(String[] args) {\n    \
             System.out.print(specBuilder().source());\n  }\n}\n",
        );
        text
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
        self.body_methods += 1;
        format!("body{}", self.body_methods)
    }

    /// The widest expression the current block can hold on one line.
    fn budget(&self) -> usize {
        LINE_WIDTH
            .saturating_sub(self.column + LET_OVERHEAD)
            .max(NARROWEST_EXPRESSION)
    }

    /// A class constant that holds the string `literal`, in pieces joined by
    /// `+` if it is too wide for one line.
    fn constant(&mut self, literal: &str) -> String {
        let name = format!("T{}", self.constants.len() + 1);
        let head = format!("private static final String {name} =");
        let one_line = format!("{head} {literal};");
        let column = MEMBER_COLUMN + CONTINUATION;
        let mut lines = Vec::new();
        if fits(MEMBER_COLUMN, &one_line) {
            lines.push(Line::new(MEMBER_COLUMN, one_line));
        } else {
            lines.push(Line::new(MEMBER_COLUMN, head));
            if fits(column, &format!("{literal};")) {
                lines.push(Line::new(column, format!("{literal};")));
            } else {
                let inner = &literal[1..literal.len() - 1];
                let units = literal_units(inner);
                let mut pieces = Vec::new();
                let mut piece = String::new();
                for (index, unit) in units.iter().enumerate() {
                    let last = index + 1 == units.len();
                    // The quotes, and `+ ` before every piece but the first
                    // and `;` after the last.
                    let taken = if pieces.is_empty() {
                        column
                    } else {
                        column + CONTINUATION + 2
                    };
                    let tail = if last { 1 } else { 0 };
                    if !piece.is_empty() && taken + piece.len() + unit.len() + 2 + tail > LINE_WIDTH
                    {
                        pieces.push(std::mem::take(&mut piece));
                    }
                    piece.push_str(unit);
                }
                pieces.push(piece);
                let count = pieces.len();
                for (index, piece) in pieces.into_iter().enumerate() {
                    let end = if index + 1 == count { ";" } else { "" };
                    if index == 0 {
                        lines.push(Line::new(column, format!("\"{piece}\"{end}")));
                    } else {
                        lines.push(Line::new(
                            column + CONTINUATION,
                            format!("+ \"{piece}\"{end}"),
                        ));
                    }
                }
            }
        }
        self.constants.push(lines);
        name
    }

    /// Move `value` into a temporary, or a string into a class constant, and
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

    /// A method for a block or a body: its signature, then `body`.
    fn method_declaration(&mut self, head: &str, parameters: &[Capture], body: Vec<Line>) {
        for parameter in parameters {
            self.imports.insert(parameter.java_type.clone());
        }
        let texts: Vec<String> = parameters
            .iter()
            .map(|parameter| format!("{} {}", parameter.java_type, parameter.identifier))
            .collect();
        let mut lines = call_lines("", head, &texts, " {", MEMBER_COLUMN).unwrap_or_else(|| {
            vec![Line::new(
                MEMBER_COLUMN,
                format!("{head}({}) {{", texts.join(", ")),
            )]
        });
        lines.extend(body);
        lines.push(Line::new(MEMBER_COLUMN, "}".to_owned()));
        self.methods.push(lines);
    }

    /// Note that the current statement uses the local variable `capture` of
    /// the function `function` levels of block methods in, so every block
    /// method between them takes it.
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

    /// The local variable of `name`, if it is one.
    fn local_capture(&self, name: NameId) -> Option<Capture> {
        self.locals[name.0].then(|| Capture {
            identifier: self.value_names[name.0].clone(),
            java_type: "DynField".to_owned(),
        })
    }

    fn handle(&mut self, name: NameId) -> Value {
        if let Some(capture) = self.local_capture(name) {
            self.use_local(&capture, 0);
        }
        Value::name(self.value_names[name.0].clone(), Class::Handle)
    }

    /// The number of block methods around the current statement.
    fn method_depth(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block| block.outlined.is_some())
            .count()
    }

    /// The variable of the visible type `name`.
    fn type_reference(&mut self, name: &str) -> String {
        let visible = self
            .visible_types
            .iter()
            .rev()
            .find(|ty| ty.name == name)
            .expect("a type that a type uses is visible");
        let capture = Capture {
            identifier: visible.variable.clone(),
            java_type: visible.java_type.to_owned(),
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

    /// `method(arguments);`.
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
                    // no local variable hides another.
                    let identifier = self.identifiers.named(Role::Type, name);
                    let java_type = self.type_class(ty);
                    let value = Value::call("defineType", arguments, Class::Other);
                    self.let_statement(&identifier, value);
                    self.visible_types.push(VisibleType {
                        name: name.clone(),
                        variable: identifier,
                        java_type,
                        function: self.method_depth(),
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
    /// `callee(arguments, body)`.
    fn open_block(
        &mut self,
        callee: &'static str,
        mut arguments: Vec<Value>,
        index: usize,
    ) -> usize {
        let close = self.closes[index];
        if close == index + 1 {
            arguments.push(Value::name("() -> {}".to_owned(), Class::Other));
            self.call_statement(callee, arguments, None);
            return close + 1;
        }
        let column = self.column;
        let body_column = if arguments.is_empty() {
            column + TAB
        } else {
            column + CONTINUATION + TAB
        };
        let outlined = (body_column > DEEPEST_BODY).then(|| {
            self.block_methods += 1;
            Outlined {
                function: format!("block{}", self.block_methods),
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
                let head = format!("private static void {}", outlined.function);
                self.method_declaration(&head, &outlined.parameters, body);
                let runnable = if outlined.parameters.is_empty() {
                    format!("SpecBuilder::{}", outlined.function)
                } else {
                    let arguments: Vec<&str> = outlined
                        .parameters
                        .iter()
                        .map(|parameter| parameter.identifier.as_str())
                        .collect();
                    format!("() -> {}({})", outlined.function, arguments.join(", "))
                };
                let mut arguments = block.arguments;
                arguments.push(Value::name(runnable, Class::Other));
                self.call_statement(block.callee, arguments, None);
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
        let mut options = Vec::new();
        for clause in clauses {
            options.push(match clause {
                SectionClause::Until { terminators, store } => self.until(terminators, *store),
                SectionClause::SkipOther => Value::new("skipOther()".to_owned(), Class::Other),
            });
        }
        let arguments = if options.len() > 3 {
            self.imports.insert("List".to_owned());
            vec![Value::call("List.of", options, Class::Other)]
        } else {
            options
        };
        self.open_block("sections", arguments, index)
    }

    fn arm(
        &mut self,
        pattern: &[Piece],
        optional: bool,
        last: bool,
        block: bool,
        index: usize,
    ) -> usize {
        let mut arguments = vec![self.pattern(pattern)];
        if optional {
            arguments.push(Value::new("optional()".to_owned(), Class::Other));
        }
        if last {
            arguments.push(Value::new("last()".to_owned(), Class::Other));
        }
        if !block {
            self.call_statement("arm", arguments, None);
            return index + 1;
        }
        self.open_block("arm", arguments, index)
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
    use super::{literal_units, quoted};

    #[test]
    fn strings_are_ascii_with_unicode_escapes() {
        assert_eq!(quoted("a"), "\"a\"");
        assert_eq!(quoted("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(quoted("tab\there\\"), "\"tab\\there\\\\\"");
        assert_eq!(quoted("line\nfeed\r"), "\"line\\nfeed\\r\"");
        assert_eq!(quoted("é\u{7f}\0"), "\"\\u00e9\\u007f\\u0000\"");
        assert_eq!(quoted("😀"), "\"\\ud83d\\ude00\"");
        assert_eq!(quoted("\\u0041"), "\"\\\\u0041\"");
    }

    #[test]
    fn splits_keep_escapes_whole() {
        assert_eq!(
            literal_units("a\\n\\u00e91\\\\u"),
            ["a", "\\n", "\\u00e9", "1", "\\\\", "u"]
        );
    }
}
