//! The `generate` strategy: a reader spec built one statement per line, with
//! input that matches it.
//!
//! Generation plans each statement, writes its spec lines, and records the
//! names it defines with their value types and depths, so that later
//! expressions, sizes and patterns use names that exist. A plan then writes
//! the statement's input, once at the top level and once per iteration in a
//! repeat. Each input token is perturbed with a small probability, so most
//! reads succeed while some reach the error paths. Counts stay small and
//! inputs short, far from the work budgets of Specification chapter 10.

use std::collections::BTreeMap;

use super::rng::Rng;

mod expr;
mod input;

/// A generated spec, its input, and the parameters as a JSON object.
pub(super) struct Generated {
    pub(super) spec: String,
    pub(super) input: Vec<u8>,
    pub(super) params: Option<String>,
}

pub(super) fn generate(rng: Rng) -> Generated {
    let mut generator = Generator::new(rng);
    generator.settings();
    let statements = generator.rng.between(1, 7);
    for _ in 0..statements {
        let plan = generator.top_statement();
        generator.write_top(&plan);
    }
    if generator.rng.chance(25) {
        let plan = generator.final_statement();
        generator.write_top(&plan);
    }
    generator.finish()
}

/// The value type of a name, with `num` reads kept apart: they produce an
/// int or a float depending on the input.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Int,
    Float,
    Num,
    Str,
}

impl Kind {
    fn is_numeric(self) -> bool {
        self != Self::Str
    }
}

/// A defined name. `repeats` are the identities of its enclosing repeats,
/// outermost first, and `own_depth` its depth within one iteration.
#[derive(Clone)]
struct Name {
    ident: String,
    kind: Kind,
    own_depth: usize,
    repeats: Vec<usize>,
    /// Defined only when an optional sections arm matched
    optional: bool,
    /// An int whose value the generator fixes and keeps small, usable as
    /// a size or bound
    small: bool,
}

/// The record list of `repeat ... as ident`, with its scalar fields.
#[derive(Clone)]
struct Records {
    ident: String,
    each: Option<String>,
    repeat: usize,
    repeats: Vec<usize>,
    fields: Vec<(String, Kind)>,
}

/// A name as an expression sees it from the current statement.
#[derive(Clone)]
struct View {
    ident: String,
    kind: Kind,
    depth: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Ordinary,
    Check,
}

/// A value the generator knows while writing input, for sizes, conditions
/// and `{$x}` references.
#[derive(Clone, PartialEq)]
enum Value {
    Int(i64),
    Word(String),
    Other,
}

/// An integer known when the plan writes its input: a literal or a name.
#[derive(Clone)]
enum Count {
    Literal(i64),
    Name(String),
}

impl Count {
    fn text(&self) -> String {
        match self {
            Self::Literal(value) => value.to_string(),
            Self::Name(name) => name.clone(),
        }
    }
}

#[derive(Clone)]
enum ReadType {
    Int,
    Float,
    Num,
    Word,
    Rest,
    /// An integer range; `None` bounds are open.
    Range {
        low: Option<Count>,
        high: Option<Count>,
        exclusive: bool,
    },
}

impl ReadType {
    fn kind(&self) -> Kind {
        match self {
            Self::Int | Self::Range { .. } => Kind::Int,
            Self::Float => Kind::Float,
            Self::Num => Kind::Num,
            Self::Word | Self::Rest => Kind::Str,
        }
    }

    /// Whether every value of this type is a small count.
    fn is_small(&self) -> bool {
        matches!(
            self,
            Self::Range {
                low: Some(Count::Literal(0..=5)),
                high: Some(Count::Literal(0..=5)),
                ..
            }
        )
    }

    fn text(&self) -> String {
        match self {
            Self::Int => "int".into(),
            Self::Float => "float".into(),
            Self::Num => "num".into(),
            Self::Word => "word".into(),
            Self::Rest => "rest".into(),
            Self::Range {
                low,
                high,
                exclusive,
            } => {
                let low = low.as_ref().map(Count::text).unwrap_or_default();
                let high = high.as_ref().map(Count::text).unwrap_or_default();
                let dots = if *exclusive { "..<" } else { ".." };
                format!("{low}{dots}{high}")
            }
        }
    }
}

/// A type as written in a declaration: a reading type, or a named type
/// standing for one.
#[derive(Clone)]
struct TypeUse {
    text: String,
    read: ReadType,
}

#[derive(Clone, Copy)]
enum Layout {
    Full,
    UpperRow,
    LowerRow,
    UpperDiagRow,
    LowerDiagRow,
    UpperCol,
    LowerDiagCol,
}

impl Layout {
    const ALL: [Self; 7] = [
        Self::Full,
        Self::UpperRow,
        Self::LowerRow,
        Self::UpperDiagRow,
        Self::LowerDiagRow,
        Self::UpperCol,
        Self::LowerDiagCol,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Full => "FULL_MATRIX",
            Self::UpperRow => "UPPER_ROW",
            Self::LowerRow => "LOWER_ROW",
            Self::UpperDiagRow => "UPPER_DIAG_ROW",
            Self::LowerDiagRow => "LOWER_DIAG_ROW",
            Self::UpperCol => "UPPER_COL",
            Self::LowerDiagCol => "LOWER_DIAG_COL",
        }
    }

    /// The number of cells an n by n matrix reads (Specification §6.8).
    fn cells(self, n: usize) -> usize {
        let triangle = n * n.saturating_sub(1) / 2;
        match self {
            Self::Full => n * n,
            Self::UpperRow | Self::LowerRow | Self::UpperCol => triangle,
            Self::UpperDiagRow | Self::LowerDiagRow | Self::LowerDiagCol => triangle + n,
        }
    }
}

#[derive(Clone)]
enum PatternPart {
    Literal(String),
    /// A capture of `read`, defining `name` if it has one; `glued` puts no
    /// blank before it.
    Capture {
        name: Option<String>,
        read: ReadType,
        glued: bool,
    },
    Reference(String),
    Ellipsis,
}

/// The statements of a repeat, and the columns it defines even when it
/// iterates zero times.
#[derive(Clone)]
struct RepeatBody {
    plans: Vec<Plan>,
    columns: Vec<String>,
}

#[derive(Clone)]
struct Arm {
    head: String,
    parts: Vec<PatternPart>,
    optional: bool,
    last: bool,
    body: Vec<Plan>,
}

/// A condition of an `if`, which the generator evaluates while writing
/// input to choose the branch whose input to write.
#[derive(Clone)]
enum Condition {
    Constant(bool),
    Compare {
        name: String,
        op: Compare,
        value: i64,
    },
    Defined(String),
}

#[derive(Clone, Copy)]
enum Compare {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
}

impl Compare {
    const ALL: [Self; 6] = [
        Self::Less,
        Self::LessEqual,
        Self::Greater,
        Self::GreaterEqual,
        Self::Equal,
        Self::NotEqual,
    ];

    fn text(self) -> &'static str {
        match self {
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::Equal => "==",
            Self::NotEqual => "!=",
        }
    }

    fn holds(self, left: i64, right: i64) -> bool {
        match self {
            Self::Less => left < right,
            Self::LessEqual => left <= right,
            Self::Greater => left > right,
            Self::GreaterEqual => left >= right,
            Self::Equal => left == right,
            Self::NotEqual => left != right,
        }
    }
}

/// How a planned statement consumes input. Plans that read nothing but
/// define a known value still update the generator's values.
#[derive(Clone)]
enum Plan {
    Silent,
    Let {
        name: String,
        value: Value,
    },
    Value {
        name: String,
        read: ReadType,
        offset: i64,
    },
    Array {
        name: String,
        read: ReadType,
        shape: Vec<Count>,
        layout: Option<Layout>,
    },
    Alphabet {
        name: String,
        rows: Count,
        columns: Count,
        symbols: Vec<u8>,
        strip: Option<u8>,
    },
    ListEol {
        name: String,
        read: ReadType,
    },
    ListSentinel {
        name: String,
        read: ReadType,
        sentinel: String,
    },
    Expect(Vec<String>),
    Line {
        find: bool,
        parts: Vec<PatternPart>,
    },
    Skip(usize),
    Index(i64),
    Repeat {
        count: Count,
        body: RepeatBody,
    },
    RepeatUntil {
        terminator: String,
        store: Option<String>,
        body: RepeatBody,
    },
    RepeatEof {
        body: RepeatBody,
    },
    Sections {
        arms: Vec<Arm>,
        terminator: Option<String>,
        skip_other: bool,
    },
    If {
        branches: Vec<(Condition, Vec<Plan>)>,
        otherwise: Vec<Plan>,
    },
    Instances {
        body: Vec<Plan>,
        count: usize,
    },
}

struct Settings {
    comment: Option<&'static str>,
    stop: Option<&'static str>,
    separator: Option<&'static str>,
    ignore_trailing: bool,
}

struct Generator {
    rng: Rng,
    spec: Vec<String>,
    input: Vec<u8>,
    params: BTreeMap<String, String>,
    settings: Settings,
    indent: usize,
    names: Vec<Name>,
    records: Vec<Records>,
    /// Named types with their visible reading types
    types: Vec<(String, ReadType)>,
    /// Active repeat identities, outermost first
    active: Vec<usize>,
    next_name: usize,
    next_repeat: usize,
    /// Values known while writing input
    values: BTreeMap<String, Value>,
    /// Iteration numbers of the repeats being written, innermost last
    iterations: Vec<i64>,
    /// Whether a statement already consumes the input up to its end
    reads_to_eof: bool,
}

const LETTERS: [&str; 12] = ["a", "b", "c", "k", "m", "n", "p", "q", "s", "w", "x", "y"];
const WORDS: [&str; 8] = ["ab", "x", "foo", "bar", "q7", "zed", "é", "a_b"];
const HEADS: [&str; 10] = [
    "NAME", "SIZE", "EDGES", "NODES", "DATA", "TYPE", "BOX", "ROW", "COLS", "P",
];

impl Generator {
    fn new(rng: Rng) -> Self {
        Self {
            rng,
            spec: Vec::new(),
            input: Vec::new(),
            params: BTreeMap::new(),
            settings: Settings {
                comment: None,
                stop: None,
                separator: None,
                ignore_trailing: false,
            },
            indent: 0,
            names: Vec::new(),
            records: Vec::new(),
            types: Vec::new(),
            active: Vec::new(),
            next_name: 0,
            next_repeat: 0,
            values: BTreeMap::new(),
            iterations: Vec::new(),
            reads_to_eof: false,
        }
    }

    fn finish(mut self) -> Generated {
        if self.settings.ignore_trailing && !self.reads_to_eof {
            self.input.extend_from_slice(b"trailing junk 1\n");
        }
        if let Some(stop) = self.settings.stop {
            self.input
                .extend_from_slice(format!("{stop}\nafter the stop\n").as_bytes());
        }
        if self.rng.chance(3) && !self.input.is_empty() {
            let end = self.rng.below(self.input.len());
            self.input.truncate(end);
        }
        let params = (!self.params.is_empty() || self.rng.chance(10))
            .then(|| serde_json::to_string(&self.params).expect("string maps serialize to JSON"));
        let mut spec = self.spec.join("\n");
        spec.push('\n');
        Generated {
            spec,
            input: self.input,
            params,
        }
    }

    fn line(&mut self, text: impl AsRef<str>) {
        self.spec
            .push(format!("{}{}", "  ".repeat(self.indent), text.as_ref()));
    }

    fn fresh_name(&mut self) -> String {
        self.next_name += 1;
        format!("{}{}", self.rng.pick(&LETTERS), self.next_name)
    }

    fn define(&mut self, ident: &str, kind: Kind, own_depth: usize, optional: bool) {
        if ident == "_" {
            return;
        }
        self.names.push(Name {
            ident: ident.into(),
            kind,
            own_depth,
            repeats: self.active.clone(),
            optional,
            small: false,
        });
    }

    /// Define an int scalar whose value the generator keeps small.
    fn define_small(&mut self, ident: &str) {
        self.define(ident, Kind::Int, 0, false);
        if let Some(name) = self.names.last_mut()
            && name.ident == ident
        {
            name.small = true;
        }
    }

    /// The small int scalars visible from the current statement.
    fn small_names(&self) -> Vec<String> {
        self.names
            .iter()
            .filter(|name| name.small && name.repeats == self.active)
            .map(|name| name.ident.clone())
            .collect()
    }

    // Settings

    fn settings(&mut self) {
        if self.rng.chance(25) {
            let name = *self
                .rng
                .pick(&["gen", "gen-1", "gen.v2-3", "\"generated\""]);
            self.line(format!("format {name}"));
        }
        if self.rng.chance(35) {
            let prefix = *self.rng.pick(&["#", "//"]);
            self.settings.comment = Some(prefix);
            self.line(format!("comment \"{prefix}\""));
        }
        if self.rng.chance(12) {
            let stop = *self.rng.pick(&["%", "EOF"]);
            self.settings.stop = Some(stop);
            self.line(format!("stop \"{stop}\""));
        }
        if self.rng.chance(12) {
            self.settings.separator = Some(",");
            self.line("separators \",\"");
        }
        if self.rng.chance(8) {
            self.line("tokens \";\"");
        }
        if self.rng.chance(12) {
            self.settings.ignore_trailing = true;
            self.line("ignore trailing");
        }
    }

    // Statements

    fn top_statement(&mut self) -> Plan {
        match self.rng.below(100) {
            0..=19 => self.declaration(),
            20..=27 => self.array(),
            28..=33 => self.list(),
            34..=41 => self.let_statement(),
            42..=47 => self.check(),
            48..=55 => self.line_statement(),
            56..=58 => self.skip(),
            59..=60 => self.expect(),
            61..=72 => self.repeat(),
            73..=76 => self.repeat_until(),
            77..=81 => self.sections(false),
            82..=86 => self.if_statement(),
            87..=89 => self.type_statement(),
            90..=93 => self.param(),
            _ => self.alphabet(),
        }
    }

    /// A statement that ends the spec, as it reads up to the end of input.
    fn final_statement(&mut self) -> Plan {
        self.reads_to_eof = true;
        match self.rng.below(3) {
            0 => self.instances(),
            1 => self.sections(true),
            _ => {
                self.line("repeat until eof {");
                let body = self.repeat_body(true);
                self.line("}");
                Plan::RepeatEof { body }
            }
        }
    }

    fn body_statement(&mut self) -> Plan {
        match self.rng.below(100) {
            0..=34 => self.declaration(),
            35..=44 => self.array(),
            45..=54 => self.list(),
            55..=62 => self.let_statement(),
            63..=68 => self.check(),
            69..=76 => self.line_statement(),
            77..=82 if !self.active.is_empty() => {
                let base = self.rng.between(0, 2);
                self.line(format!("index {base}"));
                Plan::Index(base)
            }
            83..=90 if self.active.len() < 2 => self.repeat(),
            91..=94 => self.if_statement(),
            _ => self.declaration(),
        }
    }

    fn declaration(&mut self) -> Plan {
        let type_use = self.type_use(true);
        let name = if self.rng.chance(5) {
            "_".to_string()
        } else {
            self.fresh_name()
        };
        let offset = self.offset_option(&type_use.read);
        self.line(format!("{name}: {}{}", type_use.text, offset_text(offset)));
        if type_use.read.is_small() && offset == 0 {
            self.define_small(&name);
        } else {
            self.define(&name, type_use.read.kind(), 0, false);
        }
        Plan::Value {
            name,
            read: type_use.read,
            offset,
        }
    }

    fn offset_option(&mut self, read: &ReadType) -> i64 {
        if read.kind() == Kind::Int && self.rng.chance(15) {
            self.rng.between(-3, 10)
        } else {
            0
        }
    }

    fn array(&mut self) -> Plan {
        let name = self.fresh_name();
        let rows = self.count(3);
        if self.rng.chance(50) {
            let type_use = self.type_use(false);
            let offset = self.offset_option(&type_use.read);
            self.line(format!(
                "{name}[{}]: {}{}",
                rows.text(),
                type_use.text,
                offset_text(offset)
            ));
            self.define(&name, type_use.read.kind(), 1, false);
            return Plan::Array {
                name,
                read: type_use.read,
                shape: vec![rows],
                layout: None,
            };
        }
        let numeric = [ReadType::Int, ReadType::Float, ReadType::Num, small_range()];
        let read = self.rng.pick(&numeric).clone();
        let (columns, layout) = if self.rng.chance(40) {
            (rows.clone(), Some(*self.rng.pick(&Layout::ALL)))
        } else {
            (self.count(3), None)
        };
        let offset = self.offset_option(&read);
        let layout_text = layout
            .map(|layout| format!(" layout \"{}\"", layout.name()))
            .unwrap_or_default();
        self.line(format!(
            "{name}[{}][{}]: {}{layout_text}{}",
            rows.text(),
            columns.text(),
            read.text(),
            offset_text(offset)
        ));
        self.define(&name, read.kind(), 2, false);
        Plan::Array {
            name,
            read,
            shape: vec![rows, columns],
            layout,
        }
    }

    fn alphabet(&mut self) -> Plan {
        let name = self.fresh_name();
        let rows = self.count(3);
        let columns = self.count(4);
        let alphabet = *self.rng.pick(&["01", "ab", "123456789", ".#"]);
        let mut text = format!(
            "{name}[{}][{}]: alphabet \"{alphabet}\"",
            rows.text(),
            columns.text()
        );
        let mut symbols = alphabet.as_bytes().to_vec();
        if self.rng.chance(40) {
            text.push_str(" blank \"-\"");
            symbols.push(b'-');
        }
        let strip = self.rng.chance(25).then_some(b'|');
        if strip.is_some() {
            text.push_str(" strip \"|\"");
        }
        if self.rng.chance(30) {
            text.push_str(&format!(" base {}", self.rng.between(-1, 2)));
        }
        self.line(text);
        self.define(&name, Kind::Int, 2, false);
        Plan::Alphabet {
            name,
            rows,
            columns,
            symbols,
            strip,
        }
    }

    fn list(&mut self) -> Plan {
        let name = self.fresh_name();
        if self.rng.chance(50) {
            let type_use = self.type_use(false);
            let offset = self.offset_option(&type_use.read);
            self.line(format!(
                "list {name}: {} until eol{}",
                type_use.text,
                offset_text(offset)
            ));
            self.define(&name, type_use.read.kind(), 1, false);
            return Plan::ListEol {
                name,
                read: type_use.read,
            };
        }
        let (read, sentinel, sentinel_text) = match self.rng.below(4) {
            0 => (small_range(), "-1".to_string(), "-1".to_string()),
            1 => (ReadType::Int, "END".into(), "\"END\"".into()),
            2 => (ReadType::Word, "0".into(), "0".into()),
            _ => (ReadType::Float, "-1".into(), "-1.0".into()),
        };
        self.line(format!(
            "list {name}: {} until {sentinel_text}",
            read.text()
        ));
        self.define(&name, read.kind(), 1, false);
        Plan::ListSentinel {
            name,
            read,
            sentinel,
        }
    }

    fn let_statement(&mut self) -> Plan {
        let name = self.fresh_name();
        let (text, kind, value) = match self.rng.below(10) {
            0..=2 => {
                let value = self.rng.between(0, 4);
                (value.to_string(), Kind::Int, Value::Int(value))
            }
            3..=5 => (self.int_expr(Context::Ordinary, 2), Kind::Int, Value::Other),
            6 => (
                self.numeric_expr(Context::Ordinary, 2),
                Kind::Float,
                Value::Other,
            ),
            7 => (self.string_expr(Context::Ordinary), Kind::Str, Value::Other),
            _ => (
                self.bool_expr(Context::Ordinary, 2),
                Kind::Int,
                Value::Other,
            ),
        };
        self.line(format!("let {name} = {text}"));
        if matches!(value, Value::Int(_)) {
            self.define_small(&name);
        } else {
            // A float expression may still be an int; only its use as a number matters.
            let kind = if kind == Kind::Float { Kind::Num } else { kind };
            self.define(&name, kind, 0, false);
        }
        Plan::Let { name, value }
    }

    fn check(&mut self) -> Plan {
        let condition = self.bool_expr(Context::Check, 2);
        let message = if self.rng.chance(60) {
            " \"the check failed\""
        } else {
            ""
        };
        self.line(format!("check {condition}{message}"));
        Plan::Silent
    }

    fn type_statement(&mut self) -> Plan {
        let name = format!("t{}", self.next_name + 1);
        self.next_name += 1;
        let read = match self.rng.below(3) {
            0 => small_range(),
            1 => ReadType::Word,
            _ => ReadType::Range {
                low: Some(Count::Literal(-5)),
                high: Some(Count::Literal(5)),
                exclusive: true,
            },
        };
        self.line(format!("type {name} = {}", read.text()));
        self.types.push((name, read));
        Plan::Silent
    }

    fn param(&mut self) -> Plan {
        let name = format!("p{}", self.next_name + 1);
        self.next_name += 1;
        let default = self.rng.chance(70).then(|| self.rng.between(0, 3));
        match default {
            Some(value) => self.line(format!("param {name} = {value}")),
            None => self.line(format!("param {name}")),
        }
        let supplied = (default.is_none() || self.rng.chance(50)).then(|| {
            if self.rng.chance(85) {
                self.rng.between(0, 3).to_string()
            } else {
                (*self
                    .rng
                    .pick(&["+002", "-0", "x", "", "1.5", "99999999999999999999"]))
                .into()
            }
        });
        let value = match &supplied {
            Some(text) => text.parse().map_or(Value::Other, Value::Int),
            None => default.map_or(Value::Other, Value::Int),
        };
        if let Some(text) = supplied {
            self.params.insert(name.clone(), text);
        }
        self.define_small(&name);
        Plan::Let { name, value }
    }

    fn skip(&mut self) -> Plan {
        if self.rng.chance(50) {
            self.line("skip line");
            return Plan::Skip(1);
        }
        let lines = self.rng.below(3);
        self.line(format!("skip {lines} lines"));
        Plan::Skip(lines)
    }

    fn expect(&mut self) -> Plan {
        let count = self.rng.between(1, 2) as usize;
        let tokens: Vec<String> = (0..count)
            .map(|_| (*self.rng.pick(&HEADS)).to_string())
            .collect();
        let quoted: Vec<String> = tokens.iter().map(|token| format!("\"{token}\"")).collect();
        self.line(format!("expect {}", quoted.join(" ")));
        Plan::Expect(tokens)
    }

    fn line_statement(&mut self) -> Plan {
        let find = self.rng.chance(30);
        let head = (*self.rng.pick(&HEADS)).to_string();
        let references = self.references();
        let (pattern, parts) = self.pattern(&head, false, &references);
        let keyword = if find { "find" } else { "line" };
        self.line(format!("{keyword} \"{pattern}\""));
        Plan::Line { find, parts }
    }

    /// The names a pattern may reference with `{$x}`: scalars defined
    /// before it.
    fn references(&self) -> Vec<View> {
        self.views(Context::Ordinary, |view| {
            view.depth == 0 && matches!(view.kind, Kind::Int | Kind::Str)
        })
    }

    /// A line pattern starting with the literal `head`, and its parts. The
    /// captures' names are `optional` when the pattern may not match.
    fn pattern(
        &mut self,
        head: &str,
        optional: bool,
        references: &[View],
    ) -> (String, Vec<PatternPart>) {
        let mut text = head.to_string();
        let mut parts = vec![PatternPart::Literal(head.into())];
        for _ in 0..self.rng.below(4) {
            match self.rng.below(10) {
                0 => {
                    let literal = *self.rng.pick(&[":", "x", "=", "cnf"]);
                    text.push(' ');
                    text.push_str(literal);
                    parts.push(PatternPart::Literal(literal.into()));
                }
                1 => {
                    if let Some(view) = self.pick_view(references) {
                        text.push_str(&format!(" {{${}}}", view.ident));
                        parts.push(PatternPart::Reference(view.ident));
                    }
                }
                _ => {
                    let glued = self.rng.chance(20);
                    if glued {
                        text.push_str(" size=");
                        parts.push(PatternPart::Literal("size=".into()));
                    } else {
                        text.push(' ');
                    }
                    let read = match self.rng.below(6) {
                        0 => ReadType::Word,
                        1 => ReadType::Float,
                        2 => small_range(),
                        3 => ReadType::Num,
                        _ => ReadType::Int,
                    };
                    let (capture, name) = match self.rng.below(10) {
                        0 => (format!("{{:{}}}", read.text()), None),
                        1 => ("{_}".to_string(), None),
                        2 => {
                            let name = self.fresh_name();
                            (format!("{{{name}}}"), Some((name, ReadType::Word)))
                        }
                        _ => {
                            let name = self.fresh_name();
                            (
                                format!("{{{name}:{}}}", read.text()),
                                Some((name, read.clone())),
                            )
                        }
                    };
                    text.push_str(&capture);
                    let (name, read) = match name {
                        Some((name, read)) => {
                            self.define(&name, read.kind(), 0, optional);
                            (Some(name), read)
                        }
                        None if capture == "{_}" => (None, ReadType::Word),
                        None => (None, read),
                    };
                    parts.push(PatternPart::Capture { name, read, glued });
                }
            }
        }
        if self.rng.chance(15) {
            text.push_str(" ...");
            parts.push(PatternPart::Ellipsis);
        } else if self.rng.chance(10) {
            let name = self.fresh_name();
            text.push_str(&format!(" {{{name}:rest}}"));
            self.define(&name, Kind::Str, 0, optional);
            parts.push(PatternPart::Capture {
                name: Some(name),
                read: ReadType::Rest,
                glued: false,
            });
        }
        (text, parts)
    }

    fn repeat(&mut self) -> Plan {
        let count = self.count(3);
        let repeat = self.next_repeat;
        let records = self.rng.chance(40).then(|| self.fresh_name());
        let as_text = records
            .as_ref()
            .map(|name| format!(" as {name}"))
            .unwrap_or_default();
        self.line(format!("repeat {}{as_text} {{", count.text()));
        let names_before = self.names.len();
        let each = (records.is_some() && self.rng.chance(50)).then(|| self.fresh_name());
        if let Some(each) = &each {
            self.indent += 1;
            self.line(format!("each {each}"));
            self.indent -= 1;
        }
        let body = self.repeat_body(false);
        self.line("}");
        if let Some(ident) = records {
            let fields = self.names[names_before..]
                .iter()
                .filter(|name| name.own_depth == 0 && name.repeats.last() == Some(&repeat))
                .map(|name| (name.ident.clone(), name.kind))
                .collect();
            self.records.push(Records {
                ident,
                each,
                repeat,
                repeats: self.active.clone(),
                fields,
            });
        }
        Plan::Repeat { count, body }
    }

    /// Plan a repeat body in a new frame. The body always reads input first,
    /// so that every iteration advances.
    fn repeat_body(&mut self, eof: bool) -> RepeatBody {
        let repeat = self.next_repeat;
        self.next_repeat += 1;
        self.active.push(repeat);
        self.indent += 1;
        let names = self.names.len();
        let types = self.types.len();
        let mut plans = vec![self.declaration()];
        let limit = if eof { 2 } else { 3 };
        for _ in 0..self.rng.below(limit) {
            plans.push(self.body_statement());
        }
        self.types.truncate(types);
        self.indent -= 1;
        self.active.pop();
        let columns = self.names[names..]
            .iter()
            .map(|name| name.ident.clone())
            .collect();
        RepeatBody { plans, columns }
    }

    fn repeat_until(&mut self) -> Plan {
        let terminator = (*self.rng.pick(&["END", "STOP", "-1"])).to_string();
        let mut text = format!("repeat until \"{terminator}\"");
        if self.rng.chance(30) {
            text.push_str(" \"DONE\"");
        }
        let store = self.rng.chance(40).then(|| self.fresh_name());
        if let Some(store) = &store {
            text.push_str(&format!(" store {store}"));
        }
        self.line(format!("{text} {{"));
        let body = self.repeat_body(false);
        self.line("}");
        if let Some(store) = &store {
            self.define(store, Kind::Str, 0, false);
        }
        Plan::RepeatUntil {
            terminator,
            store,
            body,
        }
    }

    /// A sections statement. Unless `to_eof`, a terminator or a last arm
    /// ends it, so statements may follow.
    fn sections(&mut self, to_eof: bool) -> Plan {
        let terminator = (!to_eof && self.rng.chance(60)).then(|| "END".to_string());
        let skip_other = self.rng.chance(30);
        let mut header = "sections".to_string();
        if let Some(terminator) = &terminator {
            header.push_str(&format!(" until \"{terminator}\""));
        }
        if skip_other {
            header.push_str(" skip other");
        }
        self.line(format!("{header} {{"));
        self.indent += 1;
        let mut heads: Vec<&str> = HEADS.to_vec();
        let count = self.rng.between(1, 4) as usize;
        let needs_last = !to_eof && terminator.is_none();
        // Arms match in input order, so they reference no sibling's captures.
        let references = self.references();
        let mut arms = Vec::new();
        for position in 0..count {
            let head = heads.remove(self.rng.below(heads.len())).to_string();
            let last = (needs_last && position + 1 == count) || self.rng.chance(5);
            let optional = !last && self.rng.chance(30);
            let (pattern, parts) = self.pattern(&head, optional, &references);
            let mut line = format!("\"{pattern}\"");
            if optional {
                line.push_str(" optional");
            }
            if last {
                line.push_str(" last");
            }
            let body = if self.rng.chance(35) {
                self.line(format!("{line} {{"));
                self.indent += 1;
                let names = self.names.len();
                let types = self.types.len();
                let body = vec![self.body_statement()];
                self.names.truncate(names);
                self.types.truncate(types);
                self.indent -= 1;
                self.line("}");
                body
            } else {
                self.line(line);
                Vec::new()
            };
            arms.push(Arm {
                head,
                parts,
                optional,
                last,
                body,
            });
        }
        self.indent -= 1;
        self.line("}");
        Plan::Sections {
            arms,
            terminator,
            skip_other,
        }
    }

    fn if_statement(&mut self) -> Plan {
        let mut branches = Vec::new();
        let (text, condition) = self.condition();
        self.line(format!("if {text} {{"));
        branches.push((condition, self.block()));
        while self.rng.chance(25) {
            let (text, condition) = self.condition();
            self.line(format!("}} else if {text} {{"));
            branches.push((condition, self.block()));
        }
        let otherwise = if self.rng.chance(50) {
            self.line("} else {");
            self.block()
        } else {
            Vec::new()
        };
        self.line("}");
        Plan::If {
            branches,
            otherwise,
        }
    }

    /// The statements of an if or else body, whose names stay local to the
    /// generator, as only one branch defines them.
    fn block(&mut self) -> Vec<Plan> {
        self.indent += 1;
        let names = self.names.len();
        let types = self.types.len();
        let body = (0..self.rng.below(3))
            .map(|_| self.body_statement())
            .collect();
        self.names.truncate(names);
        self.types.truncate(types);
        self.indent -= 1;
        body
    }

    fn condition(&mut self) -> (String, Condition) {
        let ints = self.views(Context::Ordinary, |view| {
            view.depth == 0 && view.kind == Kind::Int
        });
        match self.rng.below(6) {
            0 => {
                let value = self.rng.chance(50);
                (u8::from(value).to_string(), Condition::Constant(value))
            }
            1 | 2 if !self.names.is_empty() => {
                let name = self.names[self.rng.below(self.names.len())].ident.clone();
                (format!("defined({name})"), Condition::Defined(name))
            }
            _ => match self.pick_view(&ints) {
                Some(view) => {
                    let op = *self.rng.pick(&Compare::ALL);
                    let value = self.rng.between(0, 4);
                    (
                        format!("{} {} {value}", view.ident, op.text()),
                        Condition::Compare {
                            name: view.ident,
                            op,
                            value,
                        },
                    )
                }
                None => ("1".into(), Condition::Constant(true)),
            },
        }
    }

    fn instances(&mut self) -> Plan {
        self.line("instances {");
        self.indent += 1;
        let names = self.names.len();
        let mut body = vec![self.declaration()];
        for _ in 0..self.rng.below(3) {
            body.push(self.body_statement());
        }
        self.names.truncate(names);
        self.indent -= 1;
        self.line("}");
        let count = self.rng.between(1, 3) as usize;
        if self.rng.chance(40) {
            let selected = if self.rng.chance(85) {
                self.rng.between(1, count as i64 + 1).to_string()
            } else {
                (*self.rng.pick(&["0", "+01", "x"])).to_string()
            };
            self.params.insert("instance".into(), selected);
        }
        Plan::Instances { body, count }
    }

    // Types and counts

    /// A type for a declaration, sometimes a visible named type. `rest`
    /// only when `scalar`, as a rest array would read whole lines.
    fn type_use(&mut self, scalar: bool) -> TypeUse {
        if !self.types.is_empty() && self.rng.chance(20) {
            let (name, read) = self.rng.pick(&self.types).clone();
            if scalar || !matches!(read, ReadType::Rest) {
                return TypeUse { text: name, read };
            }
        }
        let read = match self.rng.below(12) {
            0..=2 => ReadType::Int,
            3 => ReadType::Float,
            4 => ReadType::Num,
            5 | 6 => ReadType::Word,
            7 if scalar => ReadType::Rest,
            _ => self.range(),
        };
        TypeUse {
            text: read.text(),
            read,
        }
    }

    fn range(&mut self) -> ReadType {
        let low = self.rng.between(-3, 2);
        let high = low + self.rng.between(1, 20);
        match self.rng.below(6) {
            0 => ReadType::Range {
                low: Some(Count::Literal(low)),
                high: None,
                exclusive: false,
            },
            1 => ReadType::Range {
                low: None,
                high: Some(Count::Literal(high)),
                exclusive: false,
            },
            2 => ReadType::Range {
                low: Some(Count::Literal(low)),
                high: Some(Count::Literal(high)),
                exclusive: true,
            },
            3 => {
                let names = self.small_names();
                if names.is_empty() {
                    small_range()
                } else {
                    ReadType::Range {
                        low: Some(Count::Literal(0)),
                        high: Some(Count::Name(self.rng.pick(&names).clone())),
                        exclusive: false,
                    }
                }
            }
            _ => ReadType::Range {
                low: Some(Count::Literal(low)),
                high: Some(Count::Literal(high)),
                exclusive: false,
            },
        }
    }

    /// A count for a size or repeat: a literal up to `limit`, or an int
    /// scalar whose value the input will fix.
    fn count(&mut self, limit: i64) -> Count {
        let names = self.small_names();
        if names.is_empty() || self.rng.chance(60) {
            return Count::Literal(self.rng.between(0, limit));
        }
        Count::Name(self.rng.pick(&names).clone())
    }
}

fn small_range() -> ReadType {
    ReadType::Range {
        low: Some(Count::Literal(0)),
        high: Some(Count::Literal(3)),
        exclusive: false,
    }
}

fn offset_text(offset: i64) -> String {
    if offset == 0 {
        String::new()
    } else {
        format!(" offset {offset}")
    }
}

/// Wrap a compound expression in parentheses.
fn parenthesize(expression: String) -> String {
    if expression
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.')
    {
        expression
    } else {
        format!("({expression})")
    }
}
