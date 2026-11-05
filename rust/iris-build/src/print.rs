//! The printer. It writes what `iris fmt` writes with no line limit, so the
//! text a builder emits is already formatted.
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use crate::error::{Issue, IssueKind};
use crate::expr::{BinaryOp, BindingId, Op, UnaryOp};
use crate::pattern::{CaptureSpec, Pattern, Piece};
use crate::statement::{
    Definition, Mention, NameState, RepeatCount, SectionClause, State, Stmt, is_identifier,
};
use crate::types::{AlphabetOption, Declared, ListTerminator, TypeSpec};

/// The words that end a range with no upper bound, from the Specification's
/// `upper bound ahead` predicate.
const STOP_SET: [&str; 23] = [
    "comment",
    "stop",
    "expect",
    "tokens",
    "separators",
    "ignore",
    "param",
    "list",
    "line",
    "find",
    "skip",
    "index",
    "repeat",
    "each",
    "instances",
    "sections",
    "if",
    "let",
    "check",
    "type",
    "layout",
    "offset",
    "until",
];

/// The words the type dispatch takes before a range.
const TYPE_WORDS: [&str; 6] = ["int", "float", "num", "word", "rest", "alphabet"];

// The Specification's ten precedence levels, loosest first.
const DISJUNCTION: u8 = 1;
const CONJUNCTION: u8 = 2;
const EQUALITY: u8 = 3;
const RELATION: u8 = 4;
const ADDITIVE: u8 = 5;
const MULTIPLICATIVE: u8 = 6;
const UNARY: u8 = 7;
const POWER: u8 = 8;
const POSTFIX: u8 = 9;
const PRIMARY: u8 = 10;

fn binary_level(op: BinaryOp) -> (u8, &'static str) {
    match op {
        BinaryOp::Or => (DISJUNCTION, " || "),
        BinaryOp::And => (CONJUNCTION, " && "),
        BinaryOp::Equal => (EQUALITY, " == "),
        BinaryOp::NotEqual => (EQUALITY, " != "),
        BinaryOp::Less => (RELATION, " < "),
        BinaryOp::LessEqual => (RELATION, " <= "),
        BinaryOp::Greater => (RELATION, " > "),
        BinaryOp::GreaterEqual => (RELATION, " >= "),
        BinaryOp::Add => (ADDITIVE, " + "),
        BinaryOp::Subtract => (ADDITIVE, " - "),
        BinaryOp::Multiply => (MULTIPLICATIVE, " * "),
        BinaryOp::Divide => (MULTIPLICATIVE, " / "),
        BinaryOp::Remainder => (MULTIPLICATIVE, " % "),
        BinaryOp::Power => (POWER, " ^ "),
    }
}

fn level(op: &Op) -> u8 {
    match op {
        Op::Int(value) if *value < 0 && *value != i64::MIN => UNARY,
        Op::Float(value) if value.is_sign_negative() => UNARY,
        Op::Unary(_) => UNARY,
        Op::Binary(op) => binary_level(*op).0,
        Op::Index | Op::Member(_) | Op::ListMember(_) => POSTFIX,
        _ => PRIMARY,
    }
}

/// Where each subexpression starts, indexed by the operation that ends it.
fn extents(ops: &[Op]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(ops.len());
    let mut stack: Vec<usize> = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        let first = stack.len() - op.operands();
        let start = stack.get(first).copied().unwrap_or(index);
        stack.truncate(first);
        stack.push(start);
        starts.push(start);
    }
    starts
}

/// The last operation of each of the `count` operands before `end`, in order.
fn operands(starts: &[usize], end: usize, count: usize) -> Vec<usize> {
    let mut ends = Vec::with_capacity(count);
    let mut next = end;
    for _ in 0..count {
        next -= 1;
        ends.push(next);
        next = starts[next];
    }
    ends.reverse();
    ends
}

enum Task {
    Expression {
        end: usize,
        minimum: u8,
    },
    Text(&'static str),
    /// `.name` after the operand just written.
    Member(&'static str),
    /// `.records` after the operand just written, with a repeat's `as` name.
    ListMember(usize),
}

/// Internal names, chosen after the whole stream is known.
#[derive(Default)]
struct Names {
    each: HashMap<usize, String>,
    list: HashMap<usize, String>,
    bindings: HashMap<BindingId, String>,
}

struct Fresh {
    taken: HashSet<String>,
    next: HashMap<&'static str, usize>,
}

impl Fresh {
    fn name(&mut self, prefix: &'static str) -> String {
        let next = self.next.entry(prefix).or_insert(1);
        loop {
            let name = format!("{prefix}_{next}");
            *next += 1;
            if self.taken.insert(name.clone()) {
                return name;
            }
        }
    }
}

impl Names {
    /// Choose names in stream order, skipping every name in the spec, so the
    /// same calls always give the same text.
    fn assign(state: &State) -> Self {
        let mut needs_each = HashSet::new();
        let mut needs_list = HashSet::new();
        let mut taken: HashSet<String> = state
            .names
            .keys()
            .chain(state.types.iter())
            .map(|name| (*name).to_owned())
            .collect();
        for record in &state.records {
            taken.extend(record.each.iter().map(|name| (*name).to_owned()));
            taken.extend(record.list.iter().map(|name| (*name).to_owned()));
        }
        let mut visit_chosen = |ops: &[Op]| {
            for op in ops {
                match op {
                    Op::Record(record) => {
                        needs_each.insert(state.record_id(record.id));
                    }
                    Op::RecordList(record) | Op::ListMember(record) => {
                        needs_list.insert(state.record_id(record.id));
                    }
                    Op::Comprehension {
                        name: Some(name), ..
                    } => {
                        taken.insert((*name).to_owned());
                    }
                    _ => {}
                }
            }
        };
        for stmt in &state.stmts {
            stmt.mentions(&mut |mention| {
                if let Mention::Ops(ops) = mention {
                    visit_chosen(ops);
                }
            });
        }

        let mut fresh = Fresh {
            taken,
            next: HashMap::new(),
        };
        let mut names = Self::default();
        for stmt in &state.stmts {
            if let Stmt::Repeat { record, .. } = stmt {
                let id = state.record_id(*record);
                let info = &state.records[id];
                if info.each.is_none() && needs_each.contains(&id) {
                    names.each.insert(id, fresh.name("record"));
                }
                if info.list.is_none() && needs_list.contains(&id) {
                    names.list.insert(id, fresh.name("records"));
                }
            }
            stmt.mentions(&mut |mention| {
                if let Mention::Ops(ops) = mention {
                    names.name_bindings(ops, &mut fresh);
                }
            });
        }
        names
    }

    fn name_bindings(&mut self, ops: &[Op], fresh: &mut Fresh) {
        for op in ops {
            if let Op::Comprehension {
                binding,
                name: Some(name),
                ..
            } = op
            {
                self.bindings.insert(*binding, (*name).to_owned());
            }
        }
        for op in ops {
            if let Op::Comprehension { binding, .. } | Op::Binding(binding) = op {
                self.bindings
                    .entry(*binding)
                    .or_insert_with(|| fresh.name("element"));
            }
        }
    }
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
}

fn first_word(text: &str) -> &str {
    let end = text
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .unwrap_or(text.len());
    &text[..end]
}

/// Whether a format name can be written without quotes: names and integers
/// glued with `-` or `.`, starting with a name.
fn is_bare_format(name: &str) -> bool {
    let mut rest = name;
    let mut first = true;
    loop {
        let end = rest.find(['-', '.']).unwrap_or(rest.len());
        let (part, tail) = rest.split_at(end);
        let integer = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        let valid = is_identifier(part)
            || (!first && integer && part.parse::<i64>().is_ok() && !tail.starts_with('.'));
        if !valid {
            return false;
        }
        match tail.chars().next() {
            None => return true,
            Some(_) => rest = &tail[1..],
        }
        first = false;
    }
}

struct Printer<'a> {
    state: &'a State,
    names: Names,
    out: String,
    indent: usize,
    issue: Option<Issue>,
}

/// Print the statements as text.
pub(crate) fn print(state: &State) -> Result<String, Issue> {
    let mut printer = Printer {
        state,
        names: Names::assign(state),
        out: String::new(),
        indent: 0,
        issue: None,
    };
    printer.statements();
    match printer.issue {
        Some(issue) => Err(issue),
        None => Ok(printer.out),
    }
}

impl Printer<'_> {
    fn unprintable(&mut self, message: impl Into<String>) {
        self.issue
            .get_or_insert(Issue::new(IssueKind::Unprintable, message));
    }

    fn start_line(&mut self) {
        for _ in 0..self.indent {
            self.out.push_str("  ");
        }
    }

    fn statements(&mut self) {
        let stmts = &self.state.stmts;
        let mut index = 0;
        let mut continuing = false;
        while let Some(stmt) = stmts.get(index) {
            index += 1;
            if matches!(stmt, Stmt::Close) {
                self.indent -= 1;
                self.start_line();
                self.out.push('}');
                continuing = self.after_close(stmts.get(index));
                continue;
            }
            if !continuing {
                self.start_line();
            }
            continuing = false;
            if let Stmt::Format(_) = stmt
                && index >= 2
                && ends_open(&stmts[index - 2])
            {
                self.unprintable(
                    "a range with no upper bound cannot come directly before `format`: \
                     `format` would be its upper bound",
                );
            }
            self.statement(stmt);
            if !stmt.opens_block() {
                self.out.push('\n');
                continue;
            }
            let each = self.each_line(stmt);
            if each.is_none() && matches!(stmts.get(index), Some(Stmt::Close)) {
                self.out.push_str(" {}");
                index += 1;
                continuing = self.after_close(stmts.get(index));
                continue;
            }
            self.out.push_str(" {\n");
            self.indent += 1;
            if let Some(each) = each {
                self.start_line();
                self.out.push_str("each ");
                self.out.push_str(&each);
                self.out.push('\n');
            }
        }
    }

    /// End the line after `}`, unless an `else` continues it.
    fn after_close(&mut self, next: Option<&Stmt>) -> bool {
        let continues = next.is_some_and(Stmt::continues_if);
        self.out.push(if continues { ' ' } else { '\n' });
        continues
    }

    fn record_each(&self, id: usize) -> Option<&str> {
        let id = self.state.record_id(id);
        match self.state.records[id].each {
            Some(name) => Some(name),
            None => self.names.each.get(&id).map(String::as_str),
        }
    }

    fn record_list(&self, id: usize) -> Option<&str> {
        let id = self.state.record_id(id);
        match self.state.records[id].list {
            Some(name) => Some(name),
            None => self.names.list.get(&id).map(String::as_str),
        }
    }

    fn each_line(&self, stmt: &Stmt) -> Option<String> {
        match stmt {
            Stmt::Repeat { record, .. } => self.record_each(*record).map(str::to_owned),
            _ => None,
        }
    }

    fn statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Format(name) => {
                self.out.push_str("format ");
                if is_bare_format(name) {
                    self.out.push_str(name);
                } else {
                    write_string(&mut self.out, name);
                }
            }
            Stmt::Comment(markers) => self.strings_statement("comment", markers),
            Stmt::Stop(markers) => self.strings_statement("stop", markers),
            Stmt::Tokens(characters) => {
                self.strings_statement("tokens", std::slice::from_ref(characters));
            }
            Stmt::Separators(characters) => {
                self.strings_statement("separators", std::slice::from_ref(characters));
            }
            Stmt::IgnoreTrailing => self.out.push_str("ignore trailing"),
            Stmt::Expect(words) => self.strings_statement("expect", words),
            Stmt::Param {
                definition,
                default,
            } => {
                self.out.push_str("param ");
                self.target(*definition);
                if let Some(default) = default {
                    self.out.push_str(" = ");
                    self.expression(default);
                }
            }
            Stmt::Declaration {
                definition,
                declared,
            } => {
                if definition.name == Some("format") && self.indent == 0 {
                    self.unprintable(
                        "an entry named `format` has no spelling at the top level: \
                         use a `let` or a `list`, or another name",
                    );
                }
                self.declaration(*definition, declared);
            }
            Stmt::ElseDeclaration {
                definition,
                declared,
            } => {
                self.out.push_str("else ");
                self.declaration(*definition, declared);
            }
            Stmt::List {
                definition,
                declared,
                end,
            } => self.list(*definition, declared, end),
            Stmt::TypeDefinition { name, declared } => {
                let _ = write!(self.out, "type {name} = ");
                self.type_with_options(declared);
            }
            Stmt::Line(pattern) => {
                self.out.push_str("line ");
                self.pattern(pattern);
            }
            Stmt::Find(pattern) => {
                self.out.push_str("find ");
                self.pattern(pattern);
            }
            Stmt::SkipLine => self.out.push_str("skip line"),
            Stmt::SkipLines(count) => {
                self.out.push_str("skip ");
                self.guarded(count, &["line"]);
                self.out.push_str(" lines");
            }
            Stmt::Index(position) => {
                self.out.push_str("index ");
                self.expression(position);
            }
            Stmt::Let { definition, value } => {
                self.out.push_str("let ");
                self.target(*definition);
                self.out.push_str(" = ");
                self.expression(value);
            }
            Stmt::Check { condition, message } => {
                self.out.push_str("check ");
                self.expression(condition);
                if let Some(message) = message {
                    self.out.push(' ');
                    write_string(&mut self.out, message);
                }
            }
            Stmt::Repeat { count, record } => self.repeat(count, *record),
            Stmt::Sections(clauses) => self.sections(clauses),
            Stmt::Arm { pattern, options } => {
                self.pattern(pattern);
                if options.optional {
                    self.out.push_str(" optional");
                }
                if options.last {
                    self.out.push_str(" last");
                }
            }
            Stmt::If(condition) => {
                self.out.push_str("if ");
                self.expression(condition);
            }
            Stmt::ElseIf(condition) => {
                self.out.push_str("else if ");
                self.expression(condition);
            }
            Stmt::Else => self.out.push_str("else"),
            Stmt::Instances => self.out.push_str("instances"),
            Stmt::Close => {}
        }
    }

    fn strings_statement(&mut self, keyword: &str, strings: &[String]) {
        self.out.push_str(keyword);
        self.strings(strings);
    }

    fn strings(&mut self, strings: &[String]) {
        for text in strings {
            self.out.push(' ');
            write_string(&mut self.out, text);
        }
    }

    fn target(&mut self, definition: Definition) {
        self.out.push_str(definition.name.unwrap_or("_"));
    }

    fn declaration(&mut self, definition: Definition, declared: &Declared) {
        self.target(definition);
        for dimension in &declared.dimensions {
            self.out.push('[');
            self.expression(dimension);
            self.out.push(']');
        }
        self.out.push_str(": ");
        self.type_with_options(declared);
    }

    fn list(&mut self, definition: Definition, declared: &Declared, end: &ListTerminator) {
        if declared.layout.is_some() {
            self.unprintable("a `list` has no `layout`");
        }
        self.out.push_str("list ");
        self.target(definition);
        self.out.push_str(": ");
        self.type_name(declared);
        self.out.push_str(" until ");
        match end {
            ListTerminator::Eol => self.out.push_str("eol"),
            ListTerminator::Sentinel(sentinel) => self.guarded(sentinel, &["eol"]),
        }
        self.options(declared);
    }

    fn type_with_options(&mut self, declared: &Declared) {
        self.type_name(declared);
        self.options(declared);
    }

    fn options(&mut self, declared: &Declared) {
        for (keyword, option) in [
            (" layout ", &declared.layout),
            (" offset ", &declared.offset),
        ] {
            if let Some(option) = option {
                self.out.push_str(keyword);
                self.expression(option);
            }
        }
    }

    fn type_name(&mut self, declared: &Declared) {
        match &declared.ty {
            TypeSpec::Base(name) => self.out.push_str(name),
            TypeSpec::Named(named) => self.out.push_str(named.name.text),
            TypeSpec::Alphabet { symbols, options } => {
                self.out.push_str("alphabet ");
                write_string(&mut self.out, symbols);
                for option in options {
                    self.alphabet_option(option);
                }
            }
            TypeSpec::Range {
                lower,
                upper,
                inclusive,
            } => {
                if let Some(lower) = lower {
                    self.lower_bound(lower);
                }
                self.out.push_str(if *inclusive { ".." } else { "..<" });
                if let Some(upper) = upper {
                    self.guarded(upper, &STOP_SET);
                }
            }
        }
    }

    fn alphabet_option(&mut self, option: &AlphabetOption) {
        match option {
            AlphabetOption::Blank(characters) => {
                self.out.push_str(" blank ");
                write_string(&mut self.out, characters);
            }
            AlphabetOption::Strip(characters) => {
                self.out.push_str(" strip ");
                write_string(&mut self.out, characters);
            }
            AlphabetOption::Base(i64::MIN) => {
                self.unprintable("the alphabet base -9223372036854775808 has no spelling");
            }
            AlphabetOption::Base(value) => {
                let _ = write!(self.out, " base {value}");
            }
        }
    }

    fn lower_bound(&mut self, ops: &[Op]) {
        let text = self.expression_text(ops);
        let word = first_word(&text);
        if TYPE_WORDS.contains(&word) || self.state.types.contains(word) {
            let _ = write!(self.out, "({text})");
        } else {
            self.out.push_str(&text);
        }
    }

    /// Write an expression in parentheses if its first word is one of `words`,
    /// which would otherwise be read as a keyword.
    fn guarded(&mut self, ops: &[Op], words: &[&str]) {
        let text = self.expression_text(ops);
        if words.contains(&first_word(&text)) {
            let _ = write!(self.out, "({text})");
        } else {
            self.out.push_str(&text);
        }
    }

    fn expression_text(&mut self, ops: &[Op]) -> String {
        let written = self.out.len();
        self.expression(ops);
        self.out.split_off(written)
    }

    fn repeat(&mut self, count: &RepeatCount, record: usize) {
        self.out.push_str("repeat ");
        match count {
            RepeatCount::Times(count) => self.guarded(count, &["until"]),
            RepeatCount::UntilEof => self.out.push_str("until eof"),
            RepeatCount::Until { terminators, store } => {
                self.out.push_str("until");
                self.strings(terminators);
                self.store(*store);
            }
        }
        if let Some(list) = self.record_list(record).map(str::to_owned) {
            let _ = write!(self.out, " as {list}");
        }
    }

    fn store(&mut self, store: Option<Definition>) {
        if let Some(store) = store {
            self.out.push_str(" store ");
            self.target(store);
        }
    }

    fn sections(&mut self, clauses: &[SectionClause]) {
        self.out.push_str("sections");
        for clause in clauses {
            match clause {
                SectionClause::SkipOther => self.out.push_str(" skip other"),
                SectionClause::Until { terminators, store } => {
                    self.out.push_str(" until");
                    self.strings(terminators);
                    self.store(*store);
                }
            }
        }
    }

    fn capture_type(&mut self, capture: &CaptureSpec) -> Option<String> {
        let CaptureSpec::Typed(declared) = capture else {
            return None;
        };
        let written = self.out.len();
        self.type_with_options(declared);
        let text = self.out.split_off(written);
        if text.contains('}') {
            self.unprintable(format!("the capture type `{text}` contains `}}`"));
        }
        Some(text)
    }

    fn pattern(&mut self, pattern: &Pattern) {
        let mut text = String::new();
        for piece in &pattern.pieces {
            match piece {
                Piece::Text(literal) => {
                    if literal.contains('{') || literal.contains("...") {
                        self.unprintable(format!(
                            "the pattern text `{literal}` contains `{{` or `...`, \
                             which have no literal spelling"
                        ));
                    }
                    text.push_str(literal);
                }
                Piece::Capture(name) => {
                    let spec = match self.state.names.get(name.text) {
                        Some(NameState::Capture { spec, .. }) => spec,
                        _ => &CaptureSpec::Untyped,
                    };
                    self.named_capture(&mut text, name.text, spec);
                }
                Piece::Bind { name, capture, .. } => {
                    self.named_capture(&mut text, name.text, capture);
                }
                Piece::Reference(name) => {
                    let _ = write!(text, "{{${}}}", name.text);
                }
                Piece::Validate(capture) => self.named_capture(&mut text, "", capture),
                Piece::Blank => text.push_str("{}"),
                Piece::Discard => text.push_str("{_}"),
                Piece::DiscardAs(capture) => self.named_capture(&mut text, "_", capture),
                Piece::RestOfLine => text.push_str("..."),
            }
        }
        write_string(&mut self.out, &text);
    }

    fn named_capture(&mut self, text: &mut String, name: &str, capture: &CaptureSpec) {
        match self.capture_type(capture) {
            Some(ty) => {
                let _ = write!(text, "{{{name}:{ty}}}");
            }
            None => {
                let _ = write!(text, "{{{name}}}");
            }
        }
    }

    fn literal(&mut self, op: &Op) {
        match op {
            Op::Int(i64::MIN) => self.out.push_str("(-9223372036854775807 - 1)"),
            Op::Int(value) => {
                let _ = write!(self.out, "{value}");
            }
            Op::Float(value) if value.is_finite() => {
                let _ = write!(self.out, "{value:?}");
            }
            Op::Float(value) => self.unprintable(format!("the float {value} has no spelling")),
            Op::Str(text) => write_string(&mut self.out, text),
            _ => unreachable!("only literals reach this"),
        }
    }

    /// Write `.name` after the operand just written. `gecode.x` is one host
    /// constant token, so after a whole name `gecode` the dot keeps its spaces.
    fn member(&mut self, name: &str) {
        let before = self.out.strip_suffix("gecode");
        let whole = before.is_some_and(|before| {
            !before
                .chars()
                .next_back()
                .is_some_and(|last| last.is_ascii_alphanumeric() || last == '_')
        });
        self.out.push_str(if whole { " . " } else { "." });
        self.out.push_str(name);
    }

    fn expression(&mut self, ops: &[Op]) {
        let starts = extents(ops);
        let mut tasks = vec![Task::Expression {
            end: ops.len() - 1,
            minimum: DISJUNCTION,
        }];
        while let Some(task) = tasks.pop() {
            match task {
                Task::Text(text) => self.out.push_str(text),
                Task::Member(name) => self.member(name),
                Task::ListMember(record) => {
                    let name = self.record_list(record).unwrap_or_default().to_owned();
                    self.member(&name);
                }
                Task::Expression { end, minimum } => {
                    if level(&ops[end]) < minimum {
                        self.out.push('(');
                        tasks.push(Task::Text(")"));
                    }
                    self.operation(ops, &starts, end, &mut tasks);
                }
            }
        }
    }

    fn operation(&mut self, ops: &[Op], starts: &[usize], end: usize, tasks: &mut Vec<Task>) {
        let op = &ops[end];
        match op {
            Op::Int(_) | Op::Float(_) | Op::Str(_) => self.literal(op),
            Op::Host(name) => self.out.push_str(name),
            Op::Name(name) => self.out.push_str(name.text),
            Op::Binding(binding) => {
                let name = &self.names.bindings[binding];
                self.out.push_str(name);
            }
            Op::Record(record) => {
                let name = self.record_each(record.id).unwrap_or_default().to_owned();
                self.out.push_str(&name);
            }
            Op::RecordList(record) => {
                let name = self.record_list(record.id).unwrap_or_default().to_owned();
                self.out.push_str(&name);
            }
            Op::Unary(unary) => {
                self.out.push(match unary {
                    UnaryOp::Minus => '-',
                    UnaryOp::Not => '!',
                });
                tasks.push(Task::Expression {
                    end: end - 1,
                    minimum: UNARY,
                });
            }
            Op::Binary(binary) => {
                let [left, right] = operands(starts, end, 2)[..] else {
                    unreachable!("a binary operation has two operands");
                };
                let (operator_level, text) = binary_level(*binary);
                let (left_minimum, right_minimum) = if *binary == BinaryOp::Power {
                    (POSTFIX, UNARY)
                } else {
                    (operator_level, operator_level + 1)
                };
                tasks.push(Task::Expression {
                    end: right,
                    minimum: right_minimum,
                });
                tasks.push(Task::Text(text));
                tasks.push(Task::Expression {
                    end: left,
                    minimum: left_minimum,
                });
            }
            Op::Index => {
                let [base, index] = operands(starts, end, 2)[..] else {
                    unreachable!("an index has two operands");
                };
                tasks.push(Task::Text("]"));
                tasks.push(Task::Expression {
                    end: index,
                    minimum: DISJUNCTION,
                });
                tasks.push(Task::Text("["));
                tasks.push(Task::Expression {
                    end: base,
                    minimum: POSTFIX,
                });
            }
            Op::Member(name) => {
                tasks.push(Task::Member(name.text));
                self.member_base(ops, end - 1, tasks);
            }
            Op::ListMember(record) => {
                tasks.push(Task::ListMember(record.id));
                self.member_base(ops, end - 1, tasks);
            }
            Op::Call {
                function,
                arguments,
            } => {
                self.out.push_str(function);
                self.out.push('(');
                tasks.push(Task::Text(")"));
                let arguments = operands(starts, end, *arguments);
                for (index, argument) in arguments.iter().enumerate().rev() {
                    tasks.push(Task::Expression {
                        end: *argument,
                        minimum: DISJUNCTION,
                    });
                    if index > 0 {
                        tasks.push(Task::Text(", "));
                    }
                }
            }
            Op::Comprehension {
                function, binding, ..
            } => {
                let [domain, body] = operands(starts, end, 2)[..] else {
                    unreachable!("a comprehension has a domain and a body");
                };
                let name = &self.names.bindings[binding];
                if !is_identifier(name) {
                    let message = format!("`{name}` is not an Iris name");
                    self.unprintable(message);
                }
                let _ = write!(self.out, "{function}({} in ", self.names.bindings[binding]);
                tasks.push(Task::Text(")"));
                tasks.push(Task::Expression {
                    end: body,
                    minimum: DISJUNCTION,
                });
                tasks.push(Task::Text(": "));
                tasks.push(Task::Expression {
                    end: domain,
                    minimum: DISJUNCTION,
                });
            }
        }
    }

    /// A number before `.` would lex as a malformed number, so it takes
    /// parentheses at any level.
    fn member_base(&mut self, ops: &[Op], base: usize, tasks: &mut Vec<Task>) {
        let minimum = if matches!(ops[base], Op::Int(_) | Op::Float(_)) {
            PRIMARY + 1
        } else {
            POSTFIX
        };
        tasks.push(Task::Expression { end: base, minimum });
    }
}

fn ends_open(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Declaration { declared, .. }
        | Stmt::ElseDeclaration { declared, .. }
        | Stmt::TypeDefinition { declared, .. } => declared.is_open_ended(),
        _ => false,
    }
}
