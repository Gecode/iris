//! Interpret the three grammar layers. Hooks implement only the named constraints
//! and operational rules explained in grammar.ebnf and Specification §5.12.
use std::collections::BTreeMap;
use std::{cell::RefCell, rc::Rc};

use super::ebnf::{Expr, Grammar};
use super::source::{self, Token, TokenKind};

#[derive(Debug, Clone)]
pub(super) struct Rejection {
    pub layer: &'static str,
    pub rule: String,
    pub line: usize,
    pub offset: usize,
    pub expected: String,
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let position = if self.layer == "pattern" {
            "decoded byte"
        } else {
            "byte"
        };
        write!(
            f,
            "{} / {} at source line {}, {position} {}: expected {}",
            self.layer, self.rule, self.line, self.offset, self.expected
        )
    }
}

#[derive(Debug)]
pub(super) enum Failure {
    Mismatch,
    Rejected(Rejection),
    Configuration(String),
}

pub(super) type Result<T> = std::result::Result<T, Failure>;

#[derive(Clone, Copy, Debug, PartialEq)]
enum TypeKind {
    Alphabet,
    Other,
}

/// Named types visible at the current position. Declarations are logged so
/// that backtracking and block exits undo them without copying the table.
#[derive(Default)]
struct TypeTable {
    kinds: BTreeMap<String, TypeKind>,
    undo: Vec<(String, Option<TypeKind>)>,
}

impl TypeTable {
    fn declare(&mut self, name: String, kind: TypeKind) {
        let previous = self.kinds.insert(name.clone(), kind);
        self.undo.push((name, previous));
    }

    /// Undoes declarations until `declarations` remain.
    fn roll_back(&mut self, declarations: usize) {
        while self.undo.len() > declarations {
            let (name, previous) = self.undo.pop().expect("length checked");
            match previous {
                Some(kind) => self.kinds.insert(name, kind),
                None => self.kinds.remove(&name),
            };
        }
    }
}

#[derive(Clone)]
struct State {
    at: usize,
    limit: usize,
    /// Length of the type table's undo log at this state.
    declarations: usize,
    blocks: usize,
    expressions: usize,
}

#[derive(Clone, Copy)]
enum Input<'a> {
    Source(&'a str),
    Tokens {
        tokens: &'a [Token],
        end_line: usize,
        end_offset: usize,
    },
    Pattern {
        text: &'a str,
        source_line: usize,
    },
}

struct Capture {
    start: usize,
    end: usize,
}

pub(super) struct Recognizer<'a> {
    grammar: &'a Grammar,
    input: Input<'a>,
    state: State,
    /// Shared with pattern sub-recognizers, which see the same visible types.
    types: Rc<RefCell<TypeTable>>,
    emitted: Vec<Token>,
    captures: Vec<Capture>,
    active: Vec<(&'a str, usize)>,
    best: Option<Rejection>,
    /// Byte offsets of every LF in source text, so line lookups stay logarithmic.
    newlines: Vec<usize>,
}

struct Checkpoint {
    state: State,
    emitted: usize,
    captures: usize,
}

enum Frame<'a> {
    Eval(&'a Expr),
    EnterRule(&'a str),
    ExitRule {
        name: &'a str,
        saved: State,
    },
    Sequence {
        children: &'a [Expr],
        next: usize,
    },
    Choice {
        children: &'a [Expr],
        next: usize,
        saved: Checkpoint,
    },
    Optional(Checkpoint),
    Repeat {
        child: &'a Expr,
        saved: Checkpoint,
    },
    Chain {
        body: &'a Expr,
        continuing: bool,
    },
}

impl<'a> Recognizer<'a> {
    pub fn recognize(grammar: &'a Grammar, bytes: &[u8]) -> Result<()> {
        let tokens = source::lex(grammar, bytes, 1)?;
        let text = std::str::from_utf8(bytes).expect("lexer validated UTF-8");
        let input = Input::Tokens {
            tokens: &tokens,
            end_line: 1 + text.bytes().filter(|&b| b == b'\n').count(),
            end_offset: text.len(),
        };
        let mut recognizer = Recognizer::new(grammar, input);
        recognizer.complete("reader_spec")
    }

    fn new(grammar: &'a Grammar, input: Input<'a>) -> Self {
        let limit = match input {
            Input::Source(text) | Input::Pattern { text, .. } => text.len(),
            Input::Tokens { tokens, .. } => tokens.len(),
        };
        let newlines = match input {
            Input::Source(text) => text
                .bytes()
                .enumerate()
                .filter_map(|(offset, byte)| (byte == b'\n').then_some(offset))
                .collect(),
            Input::Tokens { .. } | Input::Pattern { .. } => Vec::new(),
        };
        Self {
            grammar,
            input,
            state: State {
                at: 0,
                limit,
                declarations: 0,
                blocks: 0,
                expressions: 0,
            },
            types: Rc::default(),
            emitted: Vec::new(),
            captures: Vec::new(),
            active: Vec::new(),
            best: None,
            newlines,
        }
    }

    /// The 1-based source line containing `offset`, counting only LF.
    fn source_line(&self, offset: usize) -> usize {
        1 + self.newlines.partition_point(|&newline| newline < offset)
    }

    pub(super) fn lex_source(
        grammar: &'a Grammar,
        text: &'a str,
        line: usize,
    ) -> Result<Vec<Token>> {
        let mut recognizer = Self::new(grammar, Input::Source(text));
        recognizer.complete("source")?;
        for token in &mut recognizer.emitted {
            token.line += line - 1;
        }
        Ok(recognizer.emitted)
    }

    fn complete(&mut self, rule: &'a str) -> Result<()> {
        let result = self.evaluate(Frame::EnterRule(rule));
        match result {
            Ok(()) if self.state.at == self.state.limit => Ok(()),
            Err(Failure::Configuration(message)) => Err(Failure::Configuration(message)),
            Err(Failure::Rejected(rejection)) => Err(Failure::Rejected(rejection)),
            _ => Err(Failure::Rejected(
                self.best
                    .clone()
                    .unwrap_or_else(|| self.rejection(rule, "complete input")),
            )),
        }
    }

    fn rejection(&self, rule: &str, expected: &str) -> Rejection {
        let (layer, line, offset) = match self.input {
            Input::Source(_) => ("source", self.source_line(self.state.at), self.state.at),
            Input::Pattern { source_line, .. } => ("pattern", source_line, self.state.at),
            Input::Tokens {
                tokens,
                end_line,
                end_offset,
            } => tokens
                .get(self.state.at)
                .map_or(("tokens", end_line, end_offset), |t| {
                    ("tokens", t.line, t.span.start)
                }),
        };
        Rejection {
            layer,
            rule: rule.into(),
            line,
            offset,
            expected: expected.into(),
        }
    }

    fn mismatch(&mut self, expected: &str) -> Result<()> {
        let rule = self.active.last().map_or("input", |entry| entry.0);
        let rejection = self.rejection(rule, expected);
        if self
            .best
            .as_ref()
            .is_none_or(|best| rejection.offset >= best.offset)
        {
            self.best = Some(rejection);
        }
        Err(Failure::Mismatch)
    }

    // Grammar calls can exceed the host stack at legal language boundaries.
    // Continuations live here instead; there is no source-size/depth surrogate.
    fn evaluate(&mut self, initial: Frame<'a>) -> Result<()> {
        let mut frames = vec![initial];
        let mut result = Ok(());
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Eval(expr) => match expr {
                    Expr::Literal(text) => result = self.literal(text),
                    Expr::Special(name) => result = self.special(name),
                    Expr::Rule(name) => frames.push(Frame::EnterRule(name)),
                    Expr::Sequence(children) => {
                        frames.push(Frame::Sequence { children, next: 0 });
                    }
                    Expr::Choice(children) => {
                        frames.push(Frame::Choice {
                            children,
                            next: 1,
                            saved: self.checkpoint(),
                        });
                        frames.push(Frame::Eval(&children[0]));
                    }
                    Expr::Optional(child) => {
                        frames.push(Frame::Optional(self.checkpoint()));
                        frames.push(Frame::Eval(child));
                    }
                    Expr::Repeat(child) => {
                        frames.push(Frame::Repeat {
                            child,
                            saved: self.checkpoint(),
                        });
                        frames.push(Frame::Eval(child));
                    }
                },
                Frame::Sequence { children, next } => {
                    if result.is_ok() && next < children.len() {
                        frames.push(Frame::Sequence {
                            children,
                            next: next + 1,
                        });
                        frames.push(Frame::Eval(&children[next]));
                    }
                }
                Frame::Choice {
                    children,
                    next,
                    saved,
                } => {
                    if matches!(result, Err(Failure::Mismatch)) {
                        self.restore(saved);
                        if next < children.len() {
                            result = Ok(());
                            frames.push(Frame::Choice {
                                children,
                                next: next + 1,
                                saved: self.checkpoint(),
                            });
                            frames.push(Frame::Eval(&children[next]));
                        }
                    }
                }
                Frame::Optional(saved) => {
                    if matches!(result, Err(Failure::Mismatch)) {
                        self.restore(saved);
                        result = Ok(());
                    }
                }
                Frame::Repeat { child, saved } => match result {
                    Ok(()) if self.state.at == saved.state.at => {
                        result = Err(Failure::Configuration("zero-consumption repetition".into()));
                    }
                    Ok(()) => {
                        frames.push(Frame::Repeat {
                            child,
                            saved: self.checkpoint(),
                        });
                        frames.push(Frame::Eval(child));
                    }
                    Err(Failure::Mismatch) => {
                        self.restore(saved);
                        result = Ok(());
                    }
                    _ => {}
                },
                Frame::EnterRule(name) => {
                    let expr = self.grammar.rule(name).map_err(Failure::Configuration)?;
                    if self.active.contains(&(name, self.state.at)) {
                        return Err(Failure::Configuration(format!("left recursion in {name}")));
                    }
                    if let Input::Source(text) = self.input
                        && matches!(
                            name,
                            "source_string" | "number_token" | "name_token" | "punctuation"
                        )
                        && self.active.last().is_some_and(|&(parent, at)| {
                            parent == "lexical_token" && at == self.state.at
                        })
                        && name != source::lexical_rule(&text[self.state.at..])
                    {
                        result = self.mismatch("selected lexical token");
                        continue;
                    }
                    if let Input::Source(text) = self.input
                        && matches!(name, "host_name" | "ordinary_name")
                        && self.active.last().is_some_and(|&(parent, at)| {
                            parent == "name_token" && at == self.state.at
                        })
                    {
                        let tail = &text[self.state.at..];
                        let host = tail.starts_with("gecode.")
                            && tail
                                .as_bytes()
                                .get(7)
                                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
                        if host != (name == "host_name") {
                            result = self.mismatch("selected identifier form");
                            continue;
                        }
                    }
                    let saved = self.state.clone();
                    self.active.push((name, self.state.at));
                    frames.push(Frame::ExitRule { name, saved });
                    result = Ok(());
                    match self.input {
                        Input::Source(text) if name == "number_token" => {
                            self.state.limit = source::numeric_end(text, self.state.at);
                        }
                        Input::Tokens { .. } if name == "unary" => {
                            result = self.enter_expression();
                        }
                        _ => {}
                    }
                    if result.is_ok() {
                        if matches!(self.input, Input::Tokens { .. })
                            && matches!(
                                name,
                                "disjunction"
                                    | "conjunction"
                                    | "equality"
                                    | "relation"
                                    | "additive"
                                    | "multiplicative"
                                    | "postfix"
                            )
                        {
                            let Expr::Sequence(parts) = expr else {
                                return self.bad_chain();
                            };
                            let [first, Expr::Repeat(body)] = parts.as_slice() else {
                                return self.bad_chain();
                            };
                            frames.push(Frame::Chain {
                                body,
                                continuing: false,
                            });
                            frames.push(Frame::Eval(first));
                        } else {
                            frames.push(Frame::Eval(expr));
                        }
                    }
                }
                Frame::ExitRule { name, saved } => {
                    if result.is_ok() {
                        result = self.finish_rule(name, saved.at);
                    }
                    self.active.pop();
                    self.state.blocks = saved.blocks;
                    self.state.expressions = saved.expressions;
                    self.state.limit = saved.limit;
                    if matches!(name, "block" | "repeat_block") || result.is_err() {
                        self.types.borrow_mut().roll_back(saved.declarations);
                        self.state.declarations = saved.declarations;
                    }
                }
                Frame::Chain { body, continuing } => {
                    if continuing {
                        result = result.map_err(|error| self.commit(error));
                    }
                    if result.is_ok() {
                        match self.starts(body) {
                            Ok(true) => {
                                result = self.enter_expression();
                                if result.is_ok() {
                                    frames.push(Frame::Chain {
                                        body,
                                        continuing: true,
                                    });
                                    frames.push(Frame::Eval(body));
                                }
                            }
                            Ok(false) => {}
                            Err(error) => result = Err(error),
                        }
                    }
                }
            }
        }
        result
    }

    fn finish_rule(&mut self, name: &str, start: usize) -> Result<()> {
        match self.input {
            Input::Source(text) => self.source_rule_end(name, text, start),
            Input::Tokens { tokens, .. } if name == "type_stmt" => {
                let kind = self.type_kind_at(start + 3);
                let mut types = self.types.borrow_mut();
                types.declare(tokens[start + 1].text.clone(), kind);
                self.state.declarations = types.undo.len();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            state: self.state.clone(),
            emitted: self.emitted.len(),
            captures: self.captures.len(),
        }
    }

    fn restore(&mut self, saved: Checkpoint) {
        self.types.borrow_mut().roll_back(saved.state.declarations);
        self.state = saved.state;
        self.emitted.truncate(saved.emitted);
        self.captures.truncate(saved.captures);
    }

    fn source_rule_end(&mut self, name: &str, text: &str, start: usize) -> Result<()> {
        let boundary = match name {
            "number_token" => Some(self.state.limit),
            "name_token" => Some(source::name_end(text, start)),
            "punctuation" => Some(
                start
                    + self
                        .grammar
                        .punctuation_length(&text[start..])
                        .map_err(Failure::Configuration)?,
            ),
            _ => None,
        };
        if boundary.is_some_and(|end| self.state.at != end || start == end) {
            return self.mismatch("maximal lexical token");
        }
        if name == "number_token" {
            source::convert_number(&text[start..self.state.at])
                .map_err(|message| Failure::Rejected(self.rejection(name, &message)))?;
        }
        if name == "lexical_token" {
            let raw = &text[start..self.state.at];
            let kind = source::token_kind(raw);
            let value = if kind == TokenKind::String {
                source::decode_string(raw)
            } else {
                raw.into()
            };
            let line = self.source_line(start);
            self.emitted.push(Token {
                kind,
                text: value,
                span: start..self.state.at,
                line,
            });
        }
        Ok(())
    }

    fn literal(&mut self, expected: &str) -> Result<()> {
        if let Input::Source(text) = self.input
            && self
                .active
                .last()
                .is_some_and(|&(name, _)| name == "punctuation")
            && expected.len()
                != self
                    .grammar
                    .punctuation_length(&text[self.state.at..])
                    .map_err(Failure::Configuration)?
        {
            return self.mismatch("longest punctuation");
        }
        let length = match self.input {
            Input::Source(text) | Input::Pattern { text, .. } => text
                [self.state.at..self.state.limit]
                .starts_with(expected)
                .then_some(expected.len()),
            Input::Tokens { tokens, .. } => tokens
                .get(self.state.at)
                .filter(|t| {
                    matches!(t.kind, TokenKind::Ident | TokenKind::Punct) && t.text == expected
                })
                .map(|_| 1),
        };
        match length {
            Some(length) => {
                self.state.at += length;
                Ok(())
            }
            None => self.mismatch(&format!("terminal {expected:?}")),
        }
    }

    fn bad_chain(&self) -> Result<()> {
        Err(Failure::Configuration(
            "counted production must have form first, { suffix }".into(),
        ))
    }

    fn starts(&mut self, expr: &'a Expr) -> Result<bool> {
        match expr {
            Expr::Sequence(children) => self.starts(&children[0]),
            Expr::Choice(children) => {
                for child in children {
                    if self.starts(child)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            _ => {
                let saved = self.checkpoint();
                let result = self.evaluate(Frame::Eval(expr));
                self.restore(saved);
                match result {
                    Ok(()) => Ok(true),
                    Err(Failure::Mismatch) => Ok(false),
                    Err(error) => Err(error),
                }
            }
        }
    }

    fn commit(&self, failure: Failure) -> Failure {
        match failure {
            Failure::Mismatch => Failure::Rejected(
                self.best
                    .clone()
                    .unwrap_or_else(|| self.rejection("expression", "continuing expression")),
            ),
            other => other,
        }
    }

    fn enter_expression(&mut self) -> Result<()> {
        self.state.expressions += 1;
        if self.state.expressions > 256 {
            return Err(Failure::Rejected(
                self.rejection("expression", "expression counter <= 256"),
            ));
        }
        Ok(())
    }

    fn special(&mut self, name: &str) -> Result<()> {
        if let Input::Tokens { .. } = self.input {
            return self.token_special(name);
        }
        self.scalar_special(name)
    }

    fn scalar_special(&mut self, name: &str) -> Result<()> {
        let text = match self.input {
            Input::Source(text) | Input::Pattern { text, .. } => text,
            Input::Tokens { .. } => unreachable!(),
        };
        let tail = &text[self.state.at..self.state.limit];
        let character = tail.chars().next();
        let matches = match name {
            "ASCII whitespace byte" => {
                character.is_some_and(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}'))
            }
            "scalar other than LF" => character.is_some_and(|c| c != '\n'),
            "scalar other than quote backslash or LF" => {
                character.is_some_and(|c| !matches!(c, '"' | '\\' | '\n'))
            }
            "ASCII letter or underscore" => character.is_some_and(source::name_start),
            "ASCII digit" => character.is_some_and(|c| c.is_ascii_digit()),
            "NUL character" => character == Some('\0'),
            "tab character" => character == Some('\t'),
            "CR character" => character == Some('\r'),
            "pattern literal character" => {
                character.is_some_and(|c| !matches!(c, '{' | ' ' | '\t' | '\r'))
                    && !tail.starts_with("...")
            }
            "lexical dispatch"
            | "maximal identifier"
            | "maximal numeric candidate"
            | "longest punctuation" => return Ok(()),
            "re-lexed capture type" => {
                let Some(end) = tail.find('}') else {
                    return self.mismatch("closing capture brace");
                };
                self.captures.push(Capture {
                    start: self.state.at,
                    end: self.state.at + end,
                });
                self.state.at += end;
                return Ok(());
            }
            _ => {
                return Err(Failure::Configuration(format!(
                    "{name} is not a scalar special sequence"
                )));
            }
        };
        if matches {
            self.state.at += character.unwrap().len_utf8();
            Ok(())
        } else {
            self.mismatch(name)
        }
    }

    fn token_special(&mut self, name: &str) -> Result<()> {
        let token = self.token(self.state.at);
        let kind = match name {
            "IDENT token" => Some(TokenKind::Ident),
            "INT token" => Some(TokenKind::Int),
            "FLOAT token" => Some(TokenKind::Float),
            "STRING token" => Some(TokenKind::String),
            _ => None,
        };
        if let Some(kind) = kind {
            return if token.is_some_and(|t| t.kind == kind) {
                self.state.at += 1;
                Ok(())
            } else {
                self.mismatch(name)
            };
        }
        let at = self.state.at;
        let matches =
            match name {
                "valid UTF-8 source" | "maximal expression and expression counter" => true,
                "end of tokens" => at == self.state.limit,
                "declaration ahead" => self.declaration(at),
                "no declaration" | "option ahead" => !self.declaration(at),
                "ordinary top-level dispatch" => {
                    !self.keyword(at, "format")
                        && (!self.keyword(at, "instances") || self.declaration(at))
                }
                "base type ahead" => self.base_type(at),
                "alphabet type ahead" => self.keyword(at, "alphabet"),
                "named type ahead" => self.named_type(at),
                "range type ahead" => {
                    !self.base_type(at) && !self.keyword(at, "alphabet") && !self.named_type(at)
                }
                "upper bound ahead" => self.upper_bound(at),
                "no upper bound ahead" => !self.upper_bound(at),
                "not eol" => !self.keyword(at, "eol"),
                "not line" => !self.keyword(at, "line"),
                "not until" => !self.keyword(at, "until"),
                "not eof" => !self.keyword(at, "eof"),
                "else if ahead" => self.keyword(at, "if"),
                "host constant ahead" => token
                    .is_some_and(|t| t.kind == TokenKind::Ident && t.text.starts_with("gecode.")),
                "ordinary expression name ahead" => token
                    .is_some_and(|t| t.kind == TokenKind::Ident && !t.text.starts_with("gecode.")),
                "comprehension ahead" => self.comprehension(at),
                "no comprehension ahead" => !self.comprehension(at),
                "glued format join" => self.format_join(at),
                "block counter entry" | "else-if counter entry" => {
                    self.state.blocks += 1;
                    if self.state.blocks > 256 {
                        return Err(Failure::Rejected(
                            self.rejection("block", "block counter <= 256"),
                        ));
                    }
                    true
                }
                "decoded pattern STRING token" => return self.pattern(),
                _ => {
                    return Err(Failure::Configuration(format!(
                        "{name} is not a token special sequence"
                    )));
                }
            };
        if matches { Ok(()) } else { self.mismatch(name) }
    }

    fn token(&self, at: usize) -> Option<&'a Token> {
        match self.input {
            Input::Tokens { tokens, .. } => tokens.get(at),
            _ => None,
        }
    }
    fn keyword(&self, at: usize, word: &str) -> bool {
        self.token(at)
            .is_some_and(|t| t.kind == TokenKind::Ident && t.text == word)
    }
    fn punctuation(&self, at: usize, word: &str) -> bool {
        self.token(at)
            .is_some_and(|t| t.kind == TokenKind::Punct && t.text == word)
    }
    fn declaration(&self, mut at: usize) -> bool {
        if !self.token(at).is_some_and(|t| t.kind == TokenKind::Ident) {
            return false;
        }
        at += 1;
        while self.punctuation(at, "[") {
            let mut depth = 1;
            at += 1;
            while depth > 0 {
                if self.token(at).is_none() {
                    return false;
                }
                if self.punctuation(at, "[") {
                    depth += 1;
                }
                if self.punctuation(at, "]") {
                    depth -= 1;
                }
                at += 1;
            }
        }
        self.punctuation(at, ":")
    }
    fn base_type(&self, at: usize) -> bool {
        self.token(at).is_some_and(|t| {
            t.kind == TokenKind::Ident
                && matches!(t.text.as_str(), "int" | "float" | "num" | "word" | "rest")
        })
    }
    fn named_type(&self, at: usize) -> bool {
        !self.base_type(at)
            && !self.keyword(at, "alphabet")
            && self.token(at).is_some_and(|t| {
                t.kind == TokenKind::Ident && self.types.borrow().kinds.contains_key(&t.text)
            })
    }
    fn type_kind_at(&self, at: usize) -> TypeKind {
        if self.base_type(at) {
            return TypeKind::Other;
        }
        if self.keyword(at, "alphabet") {
            return TypeKind::Alphabet;
        }
        self.token(at)
            .and_then(|t| self.types.borrow().kinds.get(&t.text).copied())
            .unwrap_or(TypeKind::Other)
    }
    fn upper_bound(&self, at: usize) -> bool {
        let Some(token) = self.token(at) else {
            return false;
        };
        match token.kind {
            TokenKind::Ident => {
                !self.declaration(at)
                    && !matches!(
                        token.text.as_str(),
                        "comment"
                            | "stop"
                            | "expect"
                            | "tokens"
                            | "separators"
                            | "ignore"
                            | "param"
                            | "list"
                            | "line"
                            | "find"
                            | "skip"
                            | "index"
                            | "repeat"
                            | "each"
                            | "instances"
                            | "sections"
                            | "if"
                            | "let"
                            | "check"
                            | "type"
                            | "layout"
                            | "offset"
                            | "until"
                    )
            }
            TokenKind::Punct => matches!(token.text.as_str(), "(" | "-" | "!"),
            _ => true,
        }
    }
    fn comprehension(&self, at: usize) -> bool {
        // call_body begins just after the opener; the enclosing callee is
        // retained at that frame's start even when arguments contain calls.
        at >= 2
            && self.token(at - 2).is_some_and(|t| {
                t.kind == TokenKind::Ident
                    && matches!(t.text.as_str(), "all" | "any" | "sum" | "count")
            })
            && self.token(at).is_some_and(|t| t.kind == TokenKind::Ident)
            && self.keyword(at + 1, "in")
    }
    fn format_join(&self, at: usize) -> bool {
        let (Some(previous), Some(join), Some(next)) = (
            at.checked_sub(1).and_then(|i| self.token(i)),
            self.token(at),
            self.token(at + 1),
        ) else {
            return false;
        };
        join.kind == TokenKind::Punct
            && matches!(join.text.as_str(), "-" | ".")
            && matches!(next.kind, TokenKind::Ident | TokenKind::Int)
            && previous.span.end == join.span.start
            && join.span.end == next.span.start
    }

    fn pattern(&mut self) -> Result<()> {
        let Some(token) = self
            .token(self.state.at)
            .filter(|t| t.kind == TokenKind::String)
        else {
            return self.mismatch("pattern STRING");
        };
        let mut pattern = Recognizer::new(
            self.grammar,
            Input::Pattern {
                text: &token.text,
                source_line: token.line,
            },
        );
        pattern.types = Rc::clone(&self.types);
        pattern.state.declarations = self.state.declarations;
        pattern.state.blocks = self.state.blocks;
        pattern.state.expressions = self.state.expressions;
        // First validate all structure, collecting capture slices. Only then
        // re-lex and recognize their types, in capture order (§7.2).
        pattern.complete("decoded_pattern")?;
        for capture in &pattern.captures {
            let text = &token.text[capture.start..capture.end];
            let tokens = source::lex(self.grammar, text.as_bytes(), token.line)
                .map_err(|error| pattern.capture_error(error, capture.start))?;
            let mut types = Recognizer::new(
                self.grammar,
                Input::Tokens {
                    tokens: &tokens,
                    end_line: token.line + text.bytes().filter(|&b| b == b'\n').count(),
                    end_offset: text.len(),
                },
            );
            types.types = Rc::clone(&self.types);
            types.state.declarations = self.state.declarations;
            types.state.blocks = self.state.blocks;
            types.state.expressions = self.state.expressions;
            if tokens.is_empty() || types.type_kind_at(0) == TypeKind::Alphabet {
                return Err(Failure::Rejected(Rejection {
                    layer: "pattern",
                    rule: "capture_type".into(),
                    line: token.line,
                    offset: capture.start,
                    expected: "nonempty non-alphabet type".into(),
                }));
            }
            types
                .complete("type")
                .map_err(|error| pattern.capture_error(error, capture.start))?;
        }
        self.state.at += 1;
        Ok(())
    }

    fn capture_error(&self, error: Failure, start: usize) -> Failure {
        match error {
            Failure::Rejected(mut rejection) => {
                rejection.layer = "pattern";
                if let Input::Pattern { source_line, .. } = self.input {
                    rejection.line = source_line;
                }
                rejection.offset += start;
                Failure::Rejected(rejection)
            }
            other => other,
        }
    }
}
