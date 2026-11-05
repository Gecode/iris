//! Compare two reader specs as syntax trees, ignoring only what cannot change
//! their meaning: layout, comments, redundant parentheses, how numbers,
//! strings and format names are spelled, and the names of records and
//! comprehension variables.
//!
//! Each side lowers to a flat stream of items, so the comparison and the walk
//! need no recursion. Expressions become postfix: parentheses vanish and
//! associativity stays. Names resolve without any implementation crate, from
//! the record names a first pass collects and the comprehensions around them.
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::grammar::{self, Token, TokenKind, TreeEvent};

/// Which reader spec of a comparison a failure belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl fmt::Display for Side {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Left => "left",
            Self::Right => "right",
        })
    }
}

/// A reader spec that could not be compared at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub side: Side,
    pub message: String,
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} spec: {}", self.side, self.message)
    }
}

impl std::error::Error for Failure {}

/// The first place where two reader specs differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub left_line: usize,
    pub right_line: usize,
    pub message: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "left line {}, right line {}: {}",
            self.left_line, self.right_line, self.message
        )
    }
}

/// Returns the first difference between two reader specs, or `None`.
pub fn compare(left: &[u8], right: &[u8]) -> Result<Option<Difference>, Failure> {
    let left = lower_side(left, Side::Left)?;
    let right = lower_side(right, Side::Right)?;
    Ok(first_difference(&left, &right))
}

/// The result of a manifest: one process compares a whole corpus.
#[derive(Debug, Default)]
pub struct Batch {
    pub compared: usize,
    pub skipped: usize,
    pub differences: Vec<ManifestDifference>,
}

/// A manifest line whose printed spec is missing, unrecognized or different.
#[derive(Debug)]
pub struct ManifestDifference {
    pub original: PathBuf,
    pub printed: PathBuf,
    pub message: String,
}

impl fmt::Display for ManifestDifference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} and {}: {}",
            self.original.display(),
            self.printed.display(),
            self.message
        )
    }
}

impl Batch {
    pub fn summary(&self) -> String {
        format!(
            "Compare summary: compared={} skipped={} differences={}",
            self.compared,
            self.skipped,
            self.differences.len()
        )
    }
}

/// Compares every line of a manifest: `ORIGINAL TAB PRINTED`, or `ORIGINAL`
/// alone for a skip. Relative paths start at the manifest's directory. A
/// printed spec that is missing or unrecognized is a difference; an error
/// here means the manifest or an original spec is unusable.
pub fn compare_manifest(manifest: &Path) -> Result<Batch, String> {
    let text =
        fs::read_to_string(manifest).map_err(|error| format!("{}: {error}", manifest.display()))?;
    let directory = manifest.parent().unwrap_or(Path::new(""));
    let mut batch = Batch::default();
    for (index, line) in text.lines().enumerate() {
        let mut fields = line.split('\t');
        let original = fields.next().unwrap_or_default();
        let printed = fields.next();
        if original.is_empty() || printed == Some("") || fields.next().is_some() {
            return Err(format!(
                "{}:{}: expected ORIGINAL, or ORIGINAL TAB PRINTED",
                manifest.display(),
                index + 1
            ));
        }
        let original = directory.join(original);
        let Some(printed) = printed else {
            batch.skipped += 1;
            continue;
        };
        let printed = directory.join(printed);
        let bytes =
            fs::read(&original).map_err(|error| format!("{}: {error}", original.display()))?;
        let stream = lower_side(&bytes, Side::Left)
            .map_err(|failure| format!("{}: {}", original.display(), failure.message))?;
        batch.compared += 1;
        if let Some(message) = printed_difference(&stream, &printed) {
            batch.differences.push(ManifestDifference {
                original,
                printed,
                message,
            });
        }
    }
    Ok(batch)
}

fn printed_difference(original: &[Located], printed: &Path) -> Option<String> {
    let bytes = match fs::read(printed) {
        Ok(bytes) => bytes,
        Err(error) => return Some(format!("cannot read the printed spec: {error}")),
    };
    match lower_side(&bytes, Side::Right) {
        Ok(stream) => first_difference(original, &stream).map(|d| d.to_string()),
        Err(failure) => Some(format!("the printed spec is rejected: {}", failure.message)),
    }
}

fn lower_side(bytes: &[u8], side: Side) -> Result<Vec<Located>, Failure> {
    let failure = |message| Failure { side, message };
    let tree = grammar::syntax_tree(bytes).map_err(failure)?;
    let records = Records::collect(&tree).map_err(failure)?;
    Ok(Lowering::new(&records).lower(&tree))
}

fn first_difference(left: &[Located], right: &[Located]) -> Option<Difference> {
    let last_line = |items: &[Located]| items.last().map_or(1, |item| item.line);
    let position = left
        .iter()
        .zip(right)
        .position(|(left, right)| left.item != right.item)
        .or_else(|| (left.len() != right.len()).then(|| left.len().min(right.len())))?;
    let describe = |items: &[Located]| {
        items.get(position).map_or_else(
            || "the end of the spec".into(),
            |item| item.item.to_string(),
        )
    };
    Some(Difference {
        left_line: left.get(position).map_or(last_line(left), |item| item.line),
        right_line: right
            .get(position)
            .map_or(last_line(right), |item| item.line),
        message: format!("{} differs from {}", describe(left), describe(right)),
    })
}

/// One element of a canonical reader spec.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Item {
    /// A keyword, or the name of a definition, type or capture.
    Word(String),
    /// Punctuation outside expressions.
    Punctuation(String),
    Integer(i64),
    /// A float by its bits, so that spelling is ignored and `Eq` holds.
    Float(u64),
    String(String),
    FormatName(String),
    PatternStart,
    /// One literal or blank character of a pattern, or `...`.
    PatternText(String),
    PatternEnd,
    /// A name that is neither a record nor a comprehension variable.
    Value(String),
    /// A comprehension variable, counted outwards from the innermost.
    Bound(usize),
    /// Each record of the repeat with this index in text order.
    Each(usize),
    /// The record list of the repeat with this index in text order.
    List(usize),
    Unary(String),
    Binary(String),
    Index,
    Member(String),
    /// A field that is the list of the repeat with this index in text order.
    ListMember(usize),
    Call {
        callee: String,
        arguments: usize,
    },
    ComprehensionBody,
    Comprehension(String),
}

impl fmt::Display for Item {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Word(word) => write!(formatter, "'{word}'"),
            Self::Punctuation(text) => write!(formatter, "'{text}'"),
            Self::Integer(value) => write!(formatter, "integer {value}"),
            Self::Float(bits) => write!(formatter, "float {:?}", f64::from_bits(*bits)),
            Self::String(text) => write!(formatter, "string {text:?}"),
            Self::FormatName(name) => write!(formatter, "format name {name:?}"),
            Self::PatternStart => formatter.write_str("the start of a pattern"),
            Self::PatternText(text) => write!(formatter, "pattern text {text:?}"),
            Self::PatternEnd => formatter.write_str("the end of a pattern"),
            Self::Value(name) => write!(formatter, "value '{name}'"),
            Self::Bound(depth) => write!(formatter, "comprehension variable {depth}"),
            Self::Each(repeat) => write!(formatter, "each record of repeat {repeat}"),
            Self::List(repeat) => write!(formatter, "the records of repeat {repeat}"),
            Self::Unary(operator) => write!(formatter, "unary '{operator}'"),
            Self::Binary(operator) => write!(formatter, "operator '{operator}'"),
            Self::Index => formatter.write_str("an index"),
            Self::Member(name) => write!(formatter, "field '{name}'"),
            Self::ListMember(repeat) => write!(formatter, "the field list of repeat {repeat}"),
            Self::Call { callee, arguments } => {
                write!(formatter, "call of '{callee}' with {arguments} arguments")
            }
            Self::ComprehensionBody => formatter.write_str("a comprehension body"),
            Self::Comprehension(callee) => write!(formatter, "comprehension '{callee}'"),
        }
    }
}

#[derive(Debug)]
struct Located {
    item: Item,
    line: usize,
}

#[derive(Clone, Copy)]
enum RecordName {
    Each(usize),
    List(usize),
}

/// Every `each` and `as` name, by the repeat that declares it.
#[derive(Default)]
struct Records<'t> {
    names: HashMap<&'t str, RecordName>,
}

impl<'t> Records<'t> {
    fn collect(tree: &'t [TreeEvent<'static>]) -> Result<Self, String> {
        let mut records = Self::default();
        let mut rules = Vec::new();
        let mut repeats = Vec::new();
        let mut next_repeat = 0;
        let mut list_name_follows = false;
        for event in tree {
            match event {
                TreeEvent::Open(rule) => {
                    if *rule == "repeat_stmt" {
                        repeats.push(next_repeat);
                        next_repeat += 1;
                    }
                    rules.push(*rule);
                }
                TreeEvent::Close => {
                    if rules.pop() == Some("repeat_stmt") {
                        repeats.pop();
                    }
                }
                TreeEvent::Token(token) => match rules.as_slice() {
                    [.., "repeat_stmt"] if token.text == "as" => list_name_follows = true,
                    [.., "repeat_stmt", "ident"] if list_name_follows => {
                        list_name_follows = false;
                        let repeat = *repeats.last().expect("repeat_stmt is open");
                        records.declare(token, RecordName::List(repeat))?;
                    }
                    [.., "each_stmt", "ident"] => {
                        let repeat = *repeats.last().expect("each_stmt is inside a repeat");
                        records.declare(token, RecordName::Each(repeat))?;
                    }
                    _ => {}
                },
            }
        }
        Ok(records)
    }

    fn declare(&mut self, token: &'t Token, name: RecordName) -> Result<(), String> {
        match self.names.insert(&token.text, name) {
            None => Ok(()),
            Some(_) => Err(format!(
                "line {}: the record name '{}' is declared twice",
                token.line, token.text
            )),
        }
    }
}

/// Rules that wrap exactly one token.
const LEAF_RULES: [&str; 4] = ["ident", "integer", "float", "string"];

/// Rules whose tokens and children flow straight into the stream.
#[cfg(test)]
const PASS_THROUGH_RULES: &[&str] = &[
    "reader_spec",
    "top_statement",
    "statement_or_setting",
    "statement",
    "block",
    "repeat_block",
    "instances_stmt",
    "setting",
    "ordinary_stmt",
    "expect_stmt",
    "param_stmt",
    "declaration",
    "list_stmt",
    "list_end",
    "type_stmt",
    "type",
    "base_type",
    "alphabet_type",
    "range",
    "upper_bound",
    "line_stmt",
    "skip_stmt",
    "index_stmt",
    "repeat_count",
    "repeat_end",
    "terminators",
    "sections_stmt",
    "arm",
    "if_stmt",
    "else_body",
    "let_stmt",
    "check_stmt",
    "strings",
    "expression",
    "end",
    "pattern",
    "capture",
    "capture_type",
];

/// Rules of the source layer, which makes the tokens a tree starts from.
#[cfg(test)]
const SOURCE_RULES: &[&str] = &[
    "source",
    "whitespace",
    "source_comment",
    "comment_character",
    "lexical_token",
    "source_string",
    "string_character",
    "escape",
    "name_token",
    "host_name",
    "number_token",
    "integer_text",
    "float_text",
    "exponent",
    "punctuation",
];

/// What the walk knows about one open rule.
enum Frame {
    /// A rule whose tokens and children flow straight into the stream.
    PassThrough,
    /// `format_setting`, whose name collects the tokens after the keyword.
    Format {
        name: Option<String>,
    },
    FormatJoin,
    /// `repeat_stmt`: the keyword stays and its `as` clause goes.
    Repeat {
        keyword_seen: bool,
    },
    /// `each_stmt`: all of it goes.
    Each,
    AlphabetOption {
        negative: bool,
    },
    /// A chain rule or `power`, holding the operator whose right operand is
    /// still open.
    Infix {
        operator: Option<String>,
    },
    Unary {
        operator: Option<String>,
    },
    Postfix,
    Primary(Primary),
    CallBody {
        arguments: usize,
    },
    Comprehension {
        variable: Option<String>,
    },
    Pattern,
    PatternPiece,
    /// `ordinary_name` in a capture, which arrives one character at a time.
    CaptureName {
        text: String,
    },
    NameCharacter,
}

impl Frame {
    /// The frame of a rule with a meaning of its own in the stream.
    fn lowering(rule: &str) -> Option<Self> {
        Some(match rule {
            "format_setting" => Self::Format { name: None },
            "format_join" => Self::FormatJoin,
            "repeat_stmt" => Self::Repeat {
                keyword_seen: false,
            },
            "each_stmt" => Self::Each,
            "alphabet_option" => Self::AlphabetOption { negative: false },
            "disjunction" | "conjunction" | "equality" | "relation" | "additive"
            | "multiplicative" | "power" => Self::Infix { operator: None },
            "unary" => Self::Unary { operator: None },
            "postfix" => Self::Postfix,
            "primary" => Self::Primary(Primary::Complete),
            "call_body" => Self::CallBody { arguments: 0 },
            "comprehension" => Self::Comprehension { variable: None },
            "decoded_pattern" => Self::Pattern,
            "pattern_blank" | "pattern_literal" => Self::PatternPiece,
            "ordinary_name" => Self::CaptureName {
                text: String::new(),
            },
            "name_start" | "name_character" | "digit" => Self::NameCharacter,
            _ => return None,
        })
    }
}

/// What a `primary` has turned out to be so far.
enum Primary {
    /// A literal or a parenthesized expression, already in the stream.
    Complete,
    Name {
        text: String,
    },
    Call {
        callee: String,
        arguments: usize,
    },
    Comprehension,
}

struct Lowering<'r, 't> {
    records: &'r Records<'t>,
    items: Vec<Located>,
    frames: Vec<Frame>,
    /// Comprehension variables in scope, innermost last.
    bound: Vec<String>,
    line: usize,
}

impl<'r, 't> Lowering<'r, 't> {
    fn new(records: &'r Records<'t>) -> Self {
        Self {
            records,
            items: Vec::new(),
            frames: Vec::new(),
            bound: Vec::new(),
            line: 1,
        }
    }

    fn lower(mut self, tree: &[TreeEvent<'static>]) -> Vec<Located> {
        let mut events = tree.iter();
        while let Some(event) = events.next() {
            match event {
                TreeEvent::Open(rule) if LEAF_RULES.contains(rule) => {
                    let Some(TreeEvent::Token(token)) = events.next() else {
                        unreachable!("a leaf rule wraps one token");
                    };
                    self.token(token);
                    events.next();
                }
                TreeEvent::Open(rule) => self.open(rule),
                TreeEvent::Token(token) => self.token(token),
                TreeEvent::Close => self.close(),
            }
        }
        self.items
    }

    fn emit(&mut self, item: Item) {
        self.items.push(Located {
            item,
            line: self.line,
        });
    }

    fn open(&mut self, rule: &str) {
        let frame = Frame::lowering(rule).unwrap_or(Frame::PassThrough);
        if matches!(frame, Frame::Pattern) {
            self.emit(Item::PatternStart);
        }
        self.frames.push(frame);
    }

    fn token(&mut self, token: &Token) {
        self.line = token.line;
        let frame = self.frames.last_mut().expect("tokens are inside a rule");
        match frame {
            Frame::Format { name } => match name {
                None => {
                    *name = Some(String::new());
                    self.emit(Item::Word(token.text.clone()));
                }
                Some(name) => name.push_str(&token.text),
            },
            Frame::FormatJoin => self.join_format_name(&token.text),
            Frame::Repeat { keyword_seen } => {
                if !*keyword_seen {
                    *keyword_seen = true;
                    self.emit(Item::Word(token.text.clone()));
                }
            }
            Frame::Each => {}
            Frame::AlphabetOption { negative } => match (token.kind, token.text.as_str()) {
                (TokenKind::Punct, "-") => *negative = true,
                (TokenKind::Int, _) => {
                    let value = integer(token);
                    let value = if *negative { -value } else { value };
                    self.emit(Item::Integer(value));
                }
                _ => self.emit_token(token),
            },
            Frame::Infix { operator } | Frame::Unary { operator } => {
                *operator = Some(token.text.clone());
            }
            Frame::Postfix => match token.kind {
                TokenKind::Ident => self.emit_member(&token.text),
                _ if token.text == "]" => self.emit(Item::Index),
                _ => {}
            },
            Frame::Primary(primary) => match (&*primary, token.kind) {
                (Primary::Complete, TokenKind::Ident) => {
                    *primary = Primary::Name {
                        text: token.text.clone(),
                    };
                }
                (Primary::Name { text }, TokenKind::Punct) if token.text == "(" => {
                    *primary = Primary::Call {
                        callee: text.clone(),
                        arguments: 0,
                    };
                }
                (_, TokenKind::Punct) => {}
                _ => self.emit_token(token),
            },
            Frame::CallBody { .. } => {}
            Frame::Comprehension { variable } => match (&*variable, token.text.as_str()) {
                (None, name) => *variable = Some(name.into()),
                (Some(_), ":") => {
                    let name = variable.clone().expect("matched just above");
                    self.bound.push(name);
                    self.emit(Item::ComprehensionBody);
                }
                _ => {}
            },
            Frame::Pattern | Frame::PatternPiece => {
                self.emit(Item::PatternText(token.text.clone()));
            }
            Frame::NameCharacter => self.extend_capture_name(&token.text),
            Frame::CaptureName { .. } | Frame::PassThrough => self.emit_token(token),
        }
    }

    fn close(&mut self) {
        match self.frames.pop().expect("the tree is balanced") {
            Frame::Format { name } => {
                self.emit(Item::FormatName(name.expect("format_setting has a name")));
            }
            Frame::Unary {
                operator: Some(operator),
            } => self.emit(Item::Unary(operator)),
            Frame::Primary(Primary::Name { text }) => self.emit_name(&text),
            Frame::Primary(Primary::Call { callee, arguments }) => {
                self.emit(Item::Call { callee, arguments });
            }
            Frame::CallBody { arguments } => {
                if let Some(Frame::Primary(Primary::Call {
                    arguments: count, ..
                })) = self.frames.last_mut()
                {
                    *count = arguments;
                }
            }
            Frame::Comprehension { .. } => self.close_comprehension(),
            Frame::Pattern => self.emit(Item::PatternEnd),
            Frame::CaptureName { text } => self.emit(Item::Word(text)),
            _ => {}
        }
        self.child_closed();
    }

    /// Finishes the operator or argument that the closed rule completed.
    fn child_closed(&mut self) {
        match self.frames.last_mut() {
            Some(Frame::Infix { operator }) => {
                if let Some(operator) = operator.take() {
                    self.emit(Item::Binary(operator));
                }
            }
            Some(Frame::CallBody { arguments }) => *arguments += 1,
            _ => {}
        }
    }

    fn close_comprehension(&mut self) {
        self.bound.pop();
        let depth = self.frames.len();
        let Some(Frame::Primary(primary)) = depth
            .checked_sub(2)
            .and_then(|index| self.frames.get_mut(index))
        else {
            unreachable!("a comprehension is in the call body of a primary");
        };
        let Primary::Call { callee, .. } = std::mem::replace(primary, Primary::Comprehension)
        else {
            unreachable!("a comprehension follows a callee and an opening parenthesis");
        };
        self.emit(Item::Comprehension(callee));
    }

    fn join_format_name(&mut self, text: &str) {
        let format = self.frames.iter_mut().rev().find_map(|frame| match frame {
            Frame::Format { name } => name.as_mut(),
            _ => None,
        });
        format
            .expect("format_join is inside format_setting")
            .push_str(text);
    }

    fn extend_capture_name(&mut self, character: &str) {
        let name = self.frames.iter_mut().rev().find_map(|frame| match frame {
            Frame::CaptureName { text } => Some(text),
            _ => None,
        });
        name.expect("name characters are inside ordinary_name")
            .push_str(character);
    }

    fn emit_token(&mut self, token: &Token) {
        let item = match token.kind {
            TokenKind::Ident => Item::Word(token.text.clone()),
            TokenKind::Punct => Item::Punctuation(token.text.clone()),
            TokenKind::Int => Item::Integer(integer(token)),
            TokenKind::Float => {
                let value: f64 = token.text.parse().expect("the grammar checked this float");
                Item::Float(value.to_bits())
            }
            TokenKind::String => Item::String(token.text.clone()),
        };
        self.emit(item);
    }

    fn emit_name(&mut self, name: &str) {
        let bound = self
            .bound
            .iter()
            .rev()
            .position(|variable| variable == name);
        let item = match (bound, self.records.names.get(name)) {
            (Some(depth), _) => Item::Bound(depth),
            (None, Some(RecordName::Each(repeat))) => Item::Each(*repeat),
            (None, Some(RecordName::List(repeat))) => Item::List(*repeat),
            (None, None) => Item::Value(name.into()),
        };
        self.emit(item);
    }

    fn emit_member(&mut self, name: &str) {
        let item = match self.records.names.get(name) {
            Some(RecordName::List(repeat)) => Item::ListMember(*repeat),
            _ => Item::Member(name.into()),
        };
        self.emit(item);
    }
}

fn integer(token: &Token) -> i64 {
    token
        .text
        .parse()
        .expect("the grammar checked this integer")
}

#[cfg(test)]
mod tests;
