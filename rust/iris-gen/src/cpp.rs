//! The C++ backend: a program that calls `iris/build.hpp`.
//!
//! The output is what `clang-format` writes in LLVM style, without
//! `clang-format`: a statement that is too wide goes into the layouts that it
//! gives calls and lambdas, and an argument too wide for those goes into a
//! temporary or, for a string, a namespace constant first. A block deeper than
//! [`MAX_INDENT`] becomes a lambda at the top of `builder` with the concrete
//! type of its scope, which bounds the nesting of brackets and of template
//! instantiations. The events are walked with a loop and a stack of open
//! blocks, so no recursion depends on the spec.
mod expr;
mod layout;
mod names;
mod types;

use std::collections::HashMap;

use crate::Backend;
use crate::program::{
    Event, NameId, NameKind, Piece, Program, RecordId, RepeatCount, RepeatNames, SectionClause,
    Target,
};
use expr::{Bracket, Class, Form, Value};
use names::{Identifiers, Role};

/// clang-format's `ColumnLimit` in LLVM style.
const LINE_WIDTH: usize = 80;
/// One indentation level.
const TAB: usize = 2;
/// clang-format's `ContinuationIndentWidth` in LLVM style.
const CONTINUATION: usize = 4;
/// What `auto eN = ` and `;` take from a line.
const LET_OVERHEAD: usize = "auto e0000 = ;".len();
/// The narrowest budget for an expression, however deep the block.
const NARROWEST_EXPRESSION: usize = 24;
/// The deepest indentation of a block's statements in one function. Deeper
/// blocks and comprehension bodies become lambdas at the top of `builder`.
const MAX_INDENT: usize = 10;
/// The indentation of a helper lambda's statements.
const HELPER_BODY_INDENT: usize = 2;

/// A string as a C++ literal, in ASCII.
///
/// clang-format measures a line in columns, and only ASCII has a width that
/// is its length, so every other byte is a three-digit octal escape, which no
/// following digit can extend. A string with a NUL is a `std::string_view`
/// literal, which keeps it whole.
fn quoted(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() + 4);
    literal.push('"');
    for byte in text.bytes() {
        match byte {
            b'\\' => literal.push_str("\\\\"),
            b'"' => literal.push_str("\\\""),
            b'\n' => literal.push_str("\\n"),
            b'\r' => literal.push_str("\\r"),
            b'\t' => literal.push_str("\\t"),
            b' '..=b'~' => literal.push(char::from(byte)),
            _ => literal.push_str(&format!("\\{byte:03o}")),
        }
    }
    literal.push('"');
    if text.contains('\0') {
        literal.push_str("sv");
    }
    literal
}

/// The escapes and characters of a literal's text, which a split keeps whole.
fn literal_units(inner: &str) -> Vec<&str> {
    let bytes = inner.as_bytes();
    let mut units = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let length = match bytes[start] {
            b'\\' if bytes.get(start + 1).is_some_and(u8::is_ascii_digit) => 4,
            b'\\' => 2,
            _ => 1,
        };
        units.push(&inner[start..start + length]);
        start += length;
    }
    units
}

pub(crate) struct CppBackend;

impl Backend for CppBackend {
    fn render(&self, program: &Program) -> String {
        Renderer::new(program).render()
    }
}

/// One line of output; an empty one is a blank line.
#[derive(Debug, Clone)]
struct Line {
    column: usize,
    text: String,
}

impl Line {
    fn blank() -> Self {
        Self {
            column: 0,
            text: String::new(),
        }
    }

    fn is_blank(&self) -> bool {
        self.text.is_empty()
    }
}

/// An argument of a call, or a designated initializer of a braced list.
#[derive(Debug, Clone)]
struct Arg {
    designator: Option<&'static str>,
    value: Value,
}

impl Arg {
    fn new(value: Value) -> Self {
        Self {
            designator: None,
            value,
        }
    }

    fn keyword(designator: &'static str, value: Value) -> Self {
        Self {
            designator: Some(designator),
            value,
        }
    }

    fn text(&self) -> String {
        match self.designator {
            Some(designator) => format!("{designator} = {}", self.value.text),
            None => self.value.text.clone(),
        }
    }
}

/// A type definition that a later type uses, while it is visible.
#[derive(Debug)]
struct VisibleType {
    name: String,
    variable: String,
    /// The number of helper lambdas around the definition: 0 is `builder`.
    function: usize,
}

/// A block whose statements go to a helper lambda.
#[derive(Debug)]
struct Outlined {
    function: String,
    /// The types of enclosing blocks that the block uses, which the helper
    /// takes after its scope.
    types: Vec<String>,
}

/// The call that opens a block, written when the block closes.
#[derive(Debug)]
struct Opener {
    /// `s1.repeat`, or `auto c1 = s1.if_`.
    head: String,
    arguments: Vec<Arg>,
}

#[derive(Debug)]
struct OpenBlock {
    /// The scope or arms that the block's statements call.
    receiver: String,
    /// The number of repeats around the block.
    depth: usize,
    /// Whether the block's scope has `Presence::Required`.
    required: bool,
    opener: Opener,
    /// The lines before the block's opener.
    outer: Vec<Line>,
    /// The indentation of the opener.
    indent: usize,
    /// The number of visible types before the block.
    types: usize,
    outlined: Option<Outlined>,
    /// The `if` variable that a following branch continues.
    chain: Option<String>,
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

fn fits(column: usize, text: &str) -> bool {
    column + text.len() <= LINE_WIDTH
}

/// `texts` separated by `, ` and packed greedily as clang-format bin-packs
/// arguments: the first line has `first` columns taken, the others start at
/// `column`, and the last text is followed by `tail`. None if some text does
/// not fit a line, or the first does not fit the first line.
fn pack(first: usize, column: usize, texts: &[String], tail: &str) -> Option<Vec<String>> {
    if texts.is_empty() {
        return (first + tail.len() <= LINE_WIDTH).then(|| vec![tail.to_owned()]);
    }
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut start = first;
    for (index, text) in texts.iter().enumerate() {
        let end = if index + 1 == texts.len() { tail } else { "," };
        let own = format!("{text}{end}");
        if line.is_empty() {
            if start + own.len() > LINE_WIDTH {
                return None;
            }
            line = own;
        } else if start + line.len() + 1 + own.len() <= LINE_WIDTH {
            line.push(' ');
            line.push_str(&own);
        } else {
            lines.push(std::mem::take(&mut line));
            start = column;
            if start + own.len() > LINE_WIDTH {
                return None;
            }
            line = own;
        }
    }
    lines.push(line);
    Some(lines)
}

/// Whether clang-format could break a line between the parts of `value`.
fn is_splittable(value: &Value) -> bool {
    match &value.form {
        Form::Call { arguments, .. } => arguments.len() > 1,
        Form::Comprehension { .. } => true,
        _ => false,
    }
}

/// Whether a temporary can shorten `value`, or the widest of its items for a
/// braced list that cannot go into one.
fn can_shrink(value: &Value) -> bool {
    match &value.form {
        Form::Call {
            callee,
            arguments,
            hoist_as: None,
            ..
        } if callee.is_empty() => arguments.iter().any(|argument| can_shrink(&argument.value)),
        _ => !value.is_fixed(),
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
    chains: usize,
    functions: usize,
    /// Helper lambdas, each with its lines, inner ones first.
    helpers: Vec<Vec<Line>>,
    /// The namespace constants, each with its lines.
    constants: Vec<Vec<Line>>,
    /// Whether a string literal has the `sv` suffix.
    views: bool,
    /// The `if` variable for the branch that follows the current one.
    chain: Option<String>,
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
            chains: 0,
            functions: 0,
            helpers: Vec::new(),
            constants: Vec::new(),
            views: false,
            chain: None,
        }
    }

    fn render(mut self) -> String {
        let program = self.program;
        let first = match program.events.first() {
            Some(Event::Format(name)) => {
                let name = self.string_value(name);
                self.call("SpecBuilder b", Bracket::Paren, vec![Arg::new(name)], ";");
                1
            }
            _ => {
                self.line("SpecBuilder b;".to_owned());
                0
            }
        };
        self.handles();
        let handles_end = self.lines.len();
        let mut index = first;
        while let Some(event) = program.events.get(index) {
            index = self.statement(event, index);
        }
        self.line("return b;".to_owned());
        self.finish(handles_end)
    }

    /// Handles for every value name and record, made before their definitions
    /// because a reference may come first in the text.
    fn handles(&mut self) {
        let program = self.program;
        for (name, identifier) in program.names.iter().zip(self.value_names.clone()) {
            let head = format!("auto {identifier} = ");
            let text = self.string_value(&name.text);
            let value = match name.kind {
                NameKind::Forward => Value::call_of("b.forward<DynField>", &[text], Class::Handle),
                NameKind::Undefined => Value::call_of("b.undefined", &[text], Class::Handle),
            };
            self.let_statement(&head, value);
        }
        for (record, identifier) in program.records.iter().zip(self.record_names.clone()) {
            let arguments = [
                self.string_value(&record.each),
                self.string_value(&record.list),
            ];
            let value = Value::call_of("b.forward_record", &arguments, Class::Record);
            self.let_statement(&format!("auto {identifier} = "), value);
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
        let uses = |name: &str| lines.iter().any(|line| line.text.contains(name));
        let mut text = String::from("#include <iris/build.hpp>\n\n");
        if uses("std::int64_t") {
            text.push_str("#include <cstdint>\n");
        }
        text.push_str("#include <exception>\n#include <iostream>\n");
        if uses("std::string>") {
            text.push_str("#include <string>\n");
        }
        if !self.constants.is_empty() || self.views {
            text.push_str("#include <string_view>\n");
        }
        text.push_str("\nnamespace {\n\nusing namespace iris::build;\n");
        if self.views {
            text.push_str("using namespace std::string_view_literals;\n");
        }
        text.push('\n');
        if !self.constants.is_empty() {
            for constant in &self.constants {
                write_lines(&mut text, constant);
            }
            text.push('\n');
        }
        text.push_str("SpecBuilder builder() {\n");
        write_lines(&mut text, &lines);
        text.push_str("}\n\n} // namespace\n\n");
        text.push_str(
            "int main() {\n  try {\n    std::cout << builder().source();\n  } catch (const \
             std::exception &error) {\n    std::cerr << error.what() << '\\n';\n    return 1;\n  \
             }\n  return 0;\n}\n",
        );
        text
    }

    fn line(&mut self, text: String) {
        self.lines.push(Line {
            column: self.indent * TAB,
            text,
        });
    }

    fn line_at(&mut self, column: usize, text: String) {
        self.lines.push(Line { column, text });
    }

    fn temporary(&mut self) -> String {
        self.temporaries += 1;
        format!("e{}", self.temporaries)
    }

    fn next_function(&mut self) -> usize {
        self.functions += 1;
        self.functions
    }

    fn column(&self) -> usize {
        self.indent * TAB
    }

    /// The widest expression the current block can hold on one line.
    fn budget(&self) -> usize {
        LINE_WIDTH
            .saturating_sub(self.column() + LET_OVERHEAD)
            .max(NARROWEST_EXPRESSION)
    }

    /// A string literal.
    fn string_value(&mut self, text: &str) -> Value {
        self.views |= text.contains('\0');
        Value::string(text)
    }

    /// A namespace constant that holds the string `literal`, split into
    /// pieces that clang-format keeps when it is too wide for one line.
    fn constant(&mut self, literal: &str) -> String {
        let name = format!("t{}", self.constants.len() + 1);
        let head = format!("constexpr std::string_view {name} =");
        let one_line = format!("{head} {literal};");
        let mut lines = Vec::new();
        if fits(0, &one_line) {
            lines.push(Line {
                column: 0,
                text: one_line,
            });
        } else {
            lines.push(Line {
                column: 0,
                text: head,
            });
            let (body, suffix) = match literal.strip_suffix("sv") {
                Some(body) => (body, "sv"),
                None => (literal, ""),
            };
            let inner = &body[1..body.len() - 1];
            let units = literal_units(inner);
            let mut piece = String::new();
            let mut pieces = Vec::new();
            for (index, unit) in units.iter().enumerate() {
                let last = index + 1 == units.len();
                let tail = if last { suffix.len() + 1 } else { 0 };
                if CONTINUATION + piece.len() + unit.len() + 2 + tail > LINE_WIDTH {
                    pieces.push(std::mem::take(&mut piece));
                }
                piece.push_str(unit);
            }
            pieces.push(piece);
            let count = pieces.len();
            for (index, piece) in pieces.into_iter().enumerate() {
                let end = if index + 1 == count {
                    format!("{suffix};")
                } else {
                    String::new()
                };
                lines.push(Line {
                    column: CONTINUATION,
                    text: format!("\"{piece}\"{end}"),
                });
            }
        }
        self.constants.push(lines);
        name
    }

    /// Move `value` into a temporary, or a string into a namespace constant,
    /// and leave its name in its place.
    fn hoist(&mut self, value: &mut Value) {
        if matches!(value.form, Form::Str) {
            let name = self.constant(&value.text);
            *value = Value {
                literal: true,
                ..Value::name(name, value.class)
            };
            return;
        }
        let name = self.temporary();
        let mut hoisted = std::mem::replace(value, Value::name(name.clone(), value.class));
        value.literal = hoisted.literal;
        if let Form::Call {
            callee,
            arguments,
            hoist_as: Some(ty),
            bracket: Bracket::Brace,
        } = &hoisted.form
            && callee.is_empty()
        {
            hoisted = Value::braced(ty, arguments.clone(), None, hoisted.class);
        }
        self.let_statement(&format!("auto {name} = "), hoisted);
    }

    /// Shorten `value` with a temporary, or the widest item of a braced list
    /// that cannot go into one. Returns whether anything changed.
    fn shrink(&mut self, value: &mut Value) -> bool {
        if let Form::Call {
            callee,
            arguments,
            hoist_as: None,
            bracket,
        } = &mut value.form
            && callee.is_empty()
        {
            let widest = arguments
                .iter_mut()
                .filter(|argument| can_shrink(&argument.value))
                .max_by_key(|argument| argument.text().len());
            let Some(widest) = widest else {
                return false;
            };
            let mut item = widest.value.clone();
            if !self.shrink(&mut item) {
                return false;
            }
            widest.value = item;
            let (bracket, arguments) = (*bracket, arguments.clone());
            *value = Value::bracketed("", bracket, arguments, None, value.class);
            return true;
        }
        if value.is_fixed() {
            return false;
        }
        self.hoist(value);
        true
    }

    /// A helper lambda at the top of `builder`: one line if it fits, else
    /// its parameters, then its statements.
    fn helper(&mut self, function: &str, parameters: &[String], body: Vec<Line>) {
        let column = TAB;
        let head = format!("auto {function} = [&]");
        let mut lines = Vec::new();
        if let [only] = body.as_slice() {
            let one_line = format!("{head}({}) {{ {} }};", parameters.join(", "), only.text);
            if fits(column, &one_line) {
                lines.push(Line {
                    column,
                    text: one_line,
                });
                self.helpers.push(lines);
                return;
            }
        }
        let texts: Vec<String> = parameters.to_vec();
        let one_line = format!("{head}({}) {{", texts.join(", "));
        if fits(column, &one_line) {
            lines.push(Line {
                column,
                text: one_line,
            });
        } else {
            let align = column + head.len() + 1;
            let packed = pack(align, align, &texts, ") {")
                .unwrap_or_else(|| vec![format!("{}) {{", texts.join(", "))]);
            let mut packed = packed.into_iter();
            lines.push(Line {
                column,
                text: format!("{head}({}", packed.next().expect("a first line")),
            });
            for line in packed {
                lines.push(Line {
                    column: align,
                    text: line,
                });
            }
        }
        lines.extend(body);
        lines.push(Line {
            column,
            text: "};".to_owned(),
        });
        self.helpers.push(lines);
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

    fn required(&self) -> bool {
        self.blocks.last().is_none_or(|block| block.required)
    }

    /// `receiver.method(arguments);`.
    fn simple(&mut self, method: &str, arguments: Vec<Value>) {
        let callee = format!("{}.{method}", self.receiver());
        let arguments = arguments.into_iter().map(Arg::new).collect();
        self.call(&callee, Bracket::Paren, arguments, ";");
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

    /// The variable of the visible type `name`. Helpers are lambdas at the
    /// top of `builder`, before any statement, so every helper between the
    /// definition and this use takes the type as a parameter.
    fn type_reference(&mut self, name: &str) -> String {
        let visible = self
            .visible_types
            .iter()
            .rev()
            .find(|ty| ty.name == name)
            .expect("a type that a type uses is visible");
        let (variable, function) = (visible.variable.clone(), visible.function);
        let helpers = self
            .blocks
            .iter_mut()
            .filter_map(|block| block.outlined.as_mut());
        for helper in helpers.skip(function) {
            if !helper.types.contains(&variable) {
                helper.types.push(variable.clone());
            }
        }
        variable
    }

    /// The number of helper lambdas around the current statement.
    fn helper_depth(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block| block.outlined.is_some())
            .count()
    }

    fn handle(&self, name: NameId) -> Value {
        Value::name(self.value_names[name.0].clone(), Class::Handle)
    }

    fn target(&self, target: &Target) -> Value {
        match target {
            Target::Name(name) => self.handle(*name),
            Target::Discard => Value::atom("discard".to_owned(), Class::Other),
        }
    }

    /// One string, or a braced list of any other number of them.
    fn strings(&mut self, texts: &[String]) -> Value {
        let values: Vec<Value> = texts.iter().map(|text| self.string_value(text)).collect();
        match <[Value; 1]>::try_from(values) {
            Ok([only]) => only,
            Err(values) => {
                let arguments = values.into_iter().map(Arg::new).collect();
                Value::braced("", arguments, Some("Strings"), Class::Other)
            }
        }
    }

    /// Render the event at `index` and return the next event to render.
    fn statement(&mut self, event: &'a Event, index: usize) -> usize {
        match event {
            Event::Format(name) => {
                let name = self.string_value(name);
                self.simple("format", vec![name]);
            }
            Event::Comment(markers) => {
                let markers = self.strings(markers);
                self.simple("comment", vec![markers]);
            }
            Event::Stop(markers) => {
                let markers = self.strings(markers);
                self.simple("stop", vec![markers]);
            }
            Event::Tokens(text) => {
                let text = self.string_value(text);
                self.simple("tokens", vec![text]);
            }
            Event::Separators(text) => {
                let text = self.string_value(text);
                self.simple("separators", vec![text]);
            }
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
                let name_text = self.string_value(name);
                let callee = format!("{}.define_type", self.receiver());
                if *used {
                    let identifier = self.type_name(name);
                    let value = Value::call_of(&callee, &[name_text, reading], Class::Other);
                    self.let_statement(&format!("auto {identifier} = "), value);
                    self.visible_types.push(VisibleType {
                        name: name.clone(),
                        variable: identifier,
                        function: self.helper_depth(),
                    });
                } else {
                    let arguments = vec![Arg::new(name_text), Arg::new(reading)];
                    self.call(&callee, Bracket::Paren, arguments, ";");
                }
            }
            Event::Check { condition, message } => {
                let condition = self.expr(*condition);
                let mut arguments = vec![condition];
                if let Some(message) = message {
                    arguments.push(self.string_value(message));
                }
                self.simple("check", arguments);
            }
            Event::Expect(words) => {
                let words = self.strings(words);
                self.simple("expect", vec![words]);
            }
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
                let count = self.expr(*count).integral();
                self.simple("skip_lines", vec![count]);
            }
            Event::Index(position) => {
                let position = self.expr(*position).integral();
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
                let condition = self.expr(*condition);
                let continued = matches!(
                    self.program.events.get(self.closes[index] + 1),
                    Some(Event::ElseIf(_) | Event::Else | Event::ElseRead { .. })
                );
                let chain = continued.then(|| {
                    self.chains += 1;
                    format!("c{}", self.chains)
                });
                let head = match &chain {
                    Some(chain) => format!("auto {chain} = {}.if_", self.receiver()),
                    None => format!("{}.if_", self.receiver()),
                };
                let opener = Opener {
                    head,
                    arguments: vec![Arg::new(condition)],
                };
                let (depth, required) = (self.depth(), false);
                return self.open_block(opener, Vec::new(), depth, required, chain, index);
            }
            Event::ElseIf(condition) => {
                let chain = self.chain.take().expect("a branch follows its `if`");
                let condition = self.expr(*condition);
                let opener = Opener {
                    head: format!("{chain}.else_if"),
                    arguments: vec![Arg::new(condition)],
                };
                let depth = self.depth();
                return self.open_block(opener, Vec::new(), depth, false, Some(chain), index);
            }
            Event::Else => {
                let chain = self.chain.take().expect("a branch follows its `if`");
                let opener = Opener {
                    head: format!("{chain}.else_"),
                    arguments: Vec::new(),
                };
                let depth = self.depth();
                return self.open_block(opener, Vec::new(), depth, false, None, index);
            }
            Event::ElseRead { target, declared } => {
                let chain = self.chain.take().expect("a branch follows its `if`");
                let target = self.target(target);
                let reading = self.declared(declared);
                let arguments = vec![Arg::new(target), Arg::new(reading)];
                self.call(
                    &format!("{chain}.else_read_if"),
                    Bracket::Paren,
                    arguments,
                    ";",
                );
            }
            Event::Instances => {
                let opener = Opener {
                    head: "b.instances".to_owned(),
                    arguments: Vec::new(),
                };
                return self.open_block(opener, Vec::new(), 0, true, None, index);
            }
            Event::Close => self.close_block(),
        }
        index + 1
    }

    /// Open the block of the event at `index`, whose scope has `depth`
    /// repeats around it, then write `header`.
    fn open_block(
        &mut self,
        opener: Opener,
        header: Vec<(&'static str, Value)>,
        depth: usize,
        required: bool,
        chain: Option<String>,
        index: usize,
    ) -> usize {
        self.open(opener, header, depth, required, chain, index, "s", true)
    }

    #[allow(clippy::too_many_arguments)]
    fn open(
        &mut self,
        opener: Opener,
        header: Vec<(&'static str, Value)>,
        depth: usize,
        required: bool,
        chain: Option<String>,
        index: usize,
        prefix: &str,
        scope: bool,
    ) -> usize {
        let close = self.closes[index];
        if close == index + 1 && header.is_empty() {
            self.block_call(&opener.head, opener.arguments, "auto &", Vec::new());
            self.chain = chain;
            return close + 1;
        }
        let receiver = format!("{prefix}{}", self.blocks.len() + 1);
        let outlined = (scope && self.indent + 1 > MAX_INDENT).then(|| Outlined {
            function: format!("block_{}", self.next_function()),
            types: Vec::new(),
        });
        let outer = std::mem::take(&mut self.lines);
        let indent = self.indent;
        self.indent = if outlined.is_some() {
            HELPER_BODY_INDENT
        } else {
            indent + 1
        };
        self.blocks.push(OpenBlock {
            receiver,
            depth,
            required,
            opener,
            outer,
            indent,
            types: self.visible_types.len(),
            outlined,
            chain,
        });
        for (method, argument) in header {
            self.simple(method, vec![argument]);
        }
        index + 1
    }

    fn close_block(&mut self) {
        let block = self.blocks.pop().expect("every close has an open block");
        let body = std::mem::replace(&mut self.lines, block.outer);
        self.indent = block.indent;
        self.visible_types.truncate(block.types);
        let parameter = format!("auto &{}", block.receiver);
        match block.outlined {
            None => self.block_call(&block.opener.head, block.opener.arguments, &parameter, body),
            Some(outlined) => {
                let presence = if block.required { "Required" } else { "Maybe" };
                let mut parameters = vec![format!(
                    "Scope<{}, Presence::{presence}> &{}",
                    block.depth, block.receiver
                )];
                let mut arguments = vec![block.receiver.clone()];
                for ty in &outlined.types {
                    parameters.push(format!("const auto &{ty}"));
                    arguments.push(ty.clone());
                }
                self.helper(&outlined.function, &parameters, body);
                if outlined.types.is_empty() {
                    let mut opener = block.opener;
                    opener
                        .arguments
                        .push(Arg::new(Value::name(outlined.function, Class::Other)));
                    let head = opener.head;
                    self.call(&head, Bracket::Paren, opener.arguments, ";");
                } else {
                    let call = Line {
                        column: (self.indent + 1) * TAB,
                        text: format!("{}({});", outlined.function, arguments.join(", ")),
                    };
                    self.block_call(
                        &block.opener.head,
                        block.opener.arguments,
                        &parameter,
                        vec![call],
                    );
                }
            }
        }
        self.chain = block.chain;
    }

    fn repeat(&mut self, count: &RepeatCount, names: &RepeatNames, index: usize) -> usize {
        let receiver = self.receiver();
        let opener = match count {
            RepeatCount::Times(count) => {
                let count = self.expr(*count).integral();
                Opener {
                    head: format!("{receiver}.repeat"),
                    arguments: vec![Arg::new(count)],
                }
            }
            RepeatCount::UntilEof => Opener {
                head: format!("{receiver}.repeat_until_eof"),
                arguments: Vec::new(),
            },
            RepeatCount::Until { terminators, store } => {
                let mut arguments = vec![Arg::new(self.strings(terminators))];
                if let Some(store) = store {
                    let store = Arg::keyword(".store", self.handle(*store));
                    arguments.push(Arg::new(Value::braced(
                        "",
                        vec![store],
                        Some("UntilOptions"),
                        Class::Other,
                    )));
                }
                Opener {
                    head: format!("{receiver}.repeat_until"),
                    arguments,
                }
            }
        };
        let header = match names {
            RepeatNames::Forward(RecordId(record)) => {
                let record = Value::name(self.record_names[*record].clone(), Class::Record);
                vec![("record_as", record)]
            }
            RepeatNames::InPlace { each, list } => {
                let mut header = Vec::new();
                if let Some(each) = each {
                    header.push(("each_name", self.string_value(each)));
                }
                if let Some(list) = list {
                    header.push(("as_name", self.string_value(list)));
                }
                header
            }
        };
        let (depth, required) = (self.depth() + 1, self.required());
        self.open_block(opener, header, depth, required, None, index)
    }

    fn sections(&mut self, clauses: &[SectionClause], index: usize) -> usize {
        let mut options = Vec::new();
        for clause in clauses {
            let fields = match clause {
                SectionClause::Until { terminators, store } => {
                    let mut fields = vec![Arg::keyword(".until", self.strings(terminators))];
                    if let Some(store) = store {
                        fields.push(Arg::keyword(".store", self.handle(*store)));
                    }
                    fields
                }
                SectionClause::SkipOther => {
                    let yes = Value::atom("true".to_owned(), Class::Other);
                    vec![Arg::keyword(".skip_other", yes)]
                }
            };
            options.push(Value::braced(
                "",
                fields,
                Some("SectionsOptions"),
                Class::Other,
            ));
        }
        let arguments = match <[Value; 1]>::try_from(options) {
            Ok([only]) => vec![Arg::new(only)],
            Err(options) if options.is_empty() => Vec::new(),
            Err(options) => {
                let items = options.into_iter().map(Arg::new).collect();
                vec![Arg::new(Value::braced("", items, None, Class::Other))]
            }
        };
        let opener = Opener {
            head: format!("{}.sections", self.receiver()),
            arguments,
        };
        let (depth, required) = (self.depth(), self.required());
        self.open(opener, Vec::new(), depth, required, None, index, "a", false)
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
        let mut arguments = vec![Arg::new(pattern)];
        if last {
            let yes = Value::atom("true".to_owned(), Class::Other);
            arguments.push(Arg::new(Value::braced(
                "",
                vec![Arg::keyword(".last", yes)],
                Some("ArmOptions"),
                Class::Other,
            )));
        }
        let method = if optional { "optional_arm" } else { "arm" };
        let head = format!("{}.{method}", self.receiver());
        if !block {
            self.call(&head, Bracket::Paren, arguments, ";");
            return index + 1;
        }
        let opener = Opener { head, arguments };
        let (depth, required) = (self.depth(), self.required() && !optional);
        self.open_block(opener, Vec::new(), depth, required, None, index)
    }
}

/// Append `lines` to `text`, with at most one blank line in a row.
fn write_lines(text: &mut String, lines: &[Line]) {
    let mut previous_blank = true;
    for line in lines {
        if line.is_blank() {
            if !previous_blank {
                text.push('\n');
            }
            previous_blank = true;
            continue;
        }
        previous_blank = false;
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
    fn strings_are_ascii_with_octal_escapes() {
        assert_eq!(quoted("a"), "\"a\"");
        assert_eq!(quoted("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(quoted("tab\there\\"), "\"tab\\there\\\\\"");
        assert_eq!(quoted("é\u{7f}1"), "\"\\303\\251\\1771\"");
        assert_eq!(quoted("a\0b"), "\"a\\000b\"sv");
    }

    #[test]
    fn splits_keep_escapes_whole() {
        assert_eq!(literal_units("a\\n\\0011"), ["a", "\\n", "\\001", "1"]);
    }
}
