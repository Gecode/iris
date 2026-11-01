mod expressions;
mod patterns;
mod types;

use crate::{
    Diagnostic, DiagnosticCode as C, DiagnosticPhase, SourceLocation, SyntaxKind as K,
    diagnostic::text_range,
    lexer::{Lexed, Token},
};
use rowan::{GreenNode, GreenNodeBuilder};
use std::{ops::Range, sync::Arc};

type PResult<T = ()> = Result<T, ()>;

struct Stream {
    text: String,
    tokens: Vec<Token>,
    pos: usize,
    /// None for outer source; otherwise indexed by decoded byte boundary.
    raw_offsets: Option<Vec<usize>>,
    eof_line: usize,
}

impl Stream {
    fn new(text: String, tokens: Vec<Token>) -> Self {
        let eof_line = text.bytes().filter(|&byte| byte == b'\n').count() + 1;
        Self {
            text,
            tokens,
            pos: 0,
            raw_offsets: None,
            eof_line,
        }
    }
    fn significant(&self, from: usize) -> usize {
        let mut pos = from;
        while self.tokens.get(pos).is_some_and(Token::is_trivia) {
            pos += 1;
        }
        pos
    }
    fn raw_offset(&self, offset: usize) -> usize {
        self.raw_offsets.as_ref().map_or(offset, |map| map[offset])
    }
    fn raw_range(&self, range: Range<usize>) -> Range<usize> {
        self.raw_offset(range.start)..self.raw_offset(range.end)
    }
}

struct Parser<'a> {
    source: &'a str,
    origin: Arc<str>,
    stream: Stream,
    builder: GreenNodeBuilder<'static>,
    diagnostics: Vec<Diagnostic>,
    block_depth: usize,
    expression_depth: usize,
    type_names: Vec<String>,
    instances_seen: bool,
}

pub(crate) fn parse(source: &str, origin: Arc<str>, lexed: Lexed) -> (GreenNode, Vec<Diagnostic>) {
    let mut parser = Parser {
        source,
        origin,
        stream: Stream::new(source.into(), lexed.tokens),
        builder: GreenNodeBuilder::new(),
        diagnostics: lexed.diagnostics,
        block_depth: 0,
        expression_depth: 0,
        type_names: Vec::new(),
        instances_seen: false,
    };
    parser.start(K::Root);
    while !parser.eof() {
        parser.statement(true, false);
    }
    parser.trivia();
    parser.finish();
    (parser.builder.finish(), parser.diagnostics)
}

impl Parser<'_> {
    fn start(&mut self, kind: K) {
        self.builder.start_node(kind.into());
    }
    fn finish(&mut self) {
        self.builder.finish_node();
    }
    fn position(&self) -> usize {
        self.stream.significant(self.stream.pos)
    }
    fn token(&self) -> Option<&Token> {
        self.stream.tokens.get(self.position())
    }
    fn kind(&self) -> Option<K> {
        self.token().map(|token| token.kind)
    }
    fn text(&self) -> &str {
        self.token()
            .map_or("", |token| &self.stream.text[token.range.clone()])
    }
    fn at(&self, text: &str) -> bool {
        matches!(self.kind(), Some(K::Ident | K::Punct)) && self.text() == text
    }
    fn eof(&self) -> bool {
        self.token().is_none()
    }
    fn line(&self) -> usize {
        self.token()
            .map_or(self.stream.eof_line, |token| token.line)
    }
    fn raw_token(&mut self, kind: K, range: Range<usize>) {
        self.builder.token(kind.into(), &self.source[range]);
    }
    fn emit_current(&mut self) {
        let token = &self.stream.tokens[self.stream.pos];
        let kind = token.kind;
        let range = self.stream.raw_range(token.range.clone());
        self.raw_token(kind, range);
        self.stream.pos += 1;
    }
    fn trivia(&mut self) {
        while self
            .stream
            .tokens
            .get(self.stream.pos)
            .is_some_and(Token::is_trivia)
        {
            self.emit_current();
        }
    }
    fn bump(&mut self) {
        self.trivia();
        if !self.eof() {
            self.emit_current();
        }
    }
    fn eat(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn require(&mut self, text: &str) -> PResult {
        if self.eat(text) {
            Ok(())
        } else {
            self.fail(C::Syntax, format!("expected '{text}'"))
        }
    }
    fn require_kind(&mut self, kind: K) -> PResult {
        if self.kind() == Some(kind) {
            self.bump();
            Ok(())
        } else {
            self.fail(C::Syntax, format!("expected {kind:?}"))
        }
    }
    fn diagnostic(&mut self, code: C, message: impl Into<String>) {
        let range = self
            .token()
            .map_or(self.stream.text.len()..self.stream.text.len(), |token| {
                token.range.clone()
            });
        let range = self.stream.raw_range(range);
        self.report(code, message.into(), range, self.line());
    }
    fn diagnostic_at(&mut self, code: C, message: impl Into<String>, token: &Token) {
        self.report(
            code,
            message.into(),
            self.stream.raw_range(token.range.clone()),
            token.line,
        );
    }
    fn report(&mut self, code: C, message: String, range: Range<usize>, line: usize) {
        self.diagnostics.push(Diagnostic {
            code,
            message,
            range: text_range(range.start, range.end),
            location: SourceLocation {
                origin: self.origin.clone(),
                line,
                column: None,
            },
            phase: DiagnosticPhase::Parsing,
        });
        self.start(K::Error);
        self.finish();
    }
    fn fail<T>(&mut self, code: C, message: impl Into<String>) -> PResult<T> {
        self.diagnostic(code, message);
        Err(())
    }
    fn name(&mut self, allow_discard: bool) -> PResult<String> {
        if self.kind() != Some(K::Ident) {
            return self.fail(C::Syntax, "expected a name");
        }
        let name = self.text().to_owned();
        self.bump();
        if name.starts_with("gecode.") || (!allow_discard && name == "_") {
            return self.fail(C::Name, format!("'{name}' cannot be defined here"));
        }
        Ok(name)
    }
    fn declaration_ahead(&self) -> bool {
        let mut pos = self.position();
        if self
            .stream
            .tokens
            .get(pos)
            .is_none_or(|token| token.kind != K::Ident)
        {
            return false;
        }
        pos = self.stream.significant(pos + 1);
        while self.token_at(pos, "[") {
            let mut depth = 1;
            pos += 1;
            while pos < self.stream.tokens.len() && depth > 0 {
                if self.token_at(pos, "[") {
                    depth += 1;
                }
                if self.token_at(pos, "]") {
                    depth -= 1;
                }
                pos += 1;
            }
            if depth != 0 {
                return false;
            }
            pos = self.stream.significant(pos);
        }
        self.token_at(pos, ":")
    }
    fn token_at(&self, pos: usize, text: &str) -> bool {
        self.stream.tokens.get(pos).is_some_and(|token| {
            matches!(token.kind, K::Ident | K::Punct)
                && &self.stream.text[token.range.clone()] == text
        })
    }
    fn option(&self, text: &str) -> bool {
        self.at(text) && !self.declaration_ahead()
    }

    /// Statement owns its node and recovery; production methods only recognize
    /// their contents. A failed statement cannot leave the builder unbalanced.
    fn statement(&mut self, top: bool, first_in_repeat: bool) {
        let before = self.position();
        let declaration = !(top && self.at("format")) && self.declaration_ahead();
        let kind = if declaration {
            K::Declaration
        } else {
            match self.text() {
                "format" if top => K::Format,
                "comment" | "stop" | "tokens" | "separators" | "ignore" => K::Setting,
                "expect" => K::Expect,
                "param" => K::Param,
                "list" => K::List,
                "type" => K::TypeDefinition,
                "line" => K::Line,
                "find" => K::Find,
                "skip" => K::Skip,
                "index" => K::Index,
                "repeat" => K::Repeat,
                "each" => K::Each,
                "instances" => K::Instances,
                "sections" => K::Sections,
                "if" => K::If,
                "let" => K::Let,
                "check" => K::Check,
                _ => K::Error,
            }
        };
        self.trivia();
        self.start(kind);
        let result = if top && self.instances_seen && kind != K::Format {
            self.fail(
                C::Placement,
                "'instances' must be the last executable statement",
            )
        } else {
            self.statement_contents(kind, top, first_in_repeat)
        };
        if result.is_err() {
            self.recover_statement(before, top);
        }
        self.finish();
    }
    fn statement_contents(&mut self, kind: K, top: bool, first_in_repeat: bool) -> PResult {
        if kind == K::Declaration {
            return self.declaration();
        }
        if kind == K::Error {
            return self.fail(C::Syntax, "unknown statement");
        }
        if kind == K::Format {
            return self.format();
        }
        if kind == K::Setting && !top {
            return self.fail(C::Placement, "settings must be at the top level");
        }
        if kind == K::Instances && !top {
            return self.fail(C::Placement, "'instances' must be at the top level");
        }
        if kind == K::Each && !first_in_repeat {
            return self.fail(C::Placement, "'each' must be first in a repeat body");
        }
        let keyword = self.text().to_owned();
        self.bump();
        match kind {
            K::Setting => match keyword.as_str() {
                "comment" | "stop" => self.strings(),
                "ignore" => self.require("trailing"),
                _ => self.require_kind(K::String),
            },
            K::Expect => self.strings(),
            K::Each => self.name(true).map(|_| ()),
            K::Param => {
                self.name(false)?;
                if self.eat("=") {
                    self.expression()?;
                }
                Ok(())
            }
            K::Let => {
                self.name(false)?;
                self.require("=")?;
                self.expression().map(|_| ())
            }
            K::Check => {
                self.expression()?;
                if self.kind() == Some(K::String) {
                    self.bump();
                }
                Ok(())
            }
            K::TypeDefinition => {
                let name = self.name(false)?;
                self.require("=")?;
                self.read_type()?;
                self.type_names.push(name);
                Ok(())
            }
            K::List => {
                self.name(true)?;
                self.require(":")?;
                self.read_type()?;
                self.require("until")?;
                if !self.eat("eol") {
                    self.expression()?;
                }
                self.expression_option("offset", K::Offset)
            }
            K::Line | K::Find => self.pattern(),
            K::Skip => {
                if self.eat("line") {
                    Ok(())
                } else {
                    self.expression()?;
                    self.require("lines")
                }
            }
            K::Index => self.expression().map(|_| ()),
            K::Repeat => {
                if self.eat("until") {
                    if !self.eat("eof") {
                        self.terminators()?;
                    }
                } else {
                    self.expression()?;
                }
                if self.eat("as") {
                    self.name(true)?;
                }
                self.block(true)
            }
            K::Instances => {
                self.instances_seen = true;
                self.block(false)
            }
            K::Sections => self.sections(),
            K::If => self.if_contents(),
            _ => unreachable!("statement kind is dispatched above"),
        }
    }
    fn declaration(&mut self) -> PResult {
        self.name(true)?;
        while self.at("[") {
            self.start(K::Dimension);
            self.bump();
            let result = self.expression().and_then(|_| self.require("]"));
            self.finish();
            result?;
        }
        self.require(":")?;
        self.read_type()?;
        self.expression_option("layout", K::Layout)?;
        self.expression_option("offset", K::Offset)
    }
    fn expression_option(&mut self, keyword: &str, kind: K) -> PResult {
        if self.option(keyword) {
            self.start(kind);
            self.bump();
            let result = self.expression();
            self.finish();
            result?;
        }
        Ok(())
    }
    fn strings(&mut self) -> PResult {
        self.require_kind(K::String)?;
        while self.kind() == Some(K::String) {
            self.bump();
        }
        Ok(())
    }
    fn terminators(&mut self) -> PResult {
        self.start(K::Terminators);
        let result = (|| {
            self.strings()?;
            if self.eat("store") {
                self.name(true)?;
            }
            Ok(())
        })();
        self.finish();
        result
    }
    fn format(&mut self) -> PResult {
        self.bump();
        if !matches!(self.kind(), Some(K::Ident | K::String)) {
            return self.fail(C::Syntax, "expected a format name");
        }
        self.bump();
        loop {
            let pos = self.position();
            if !(self.at("-") || self.at(".")) {
                break;
            }
            let join = &self.stream.tokens[pos];
            let previous = &self.stream.tokens[self.stream.pos - 1];
            let next = self.stream.tokens.get(pos + 1);
            if previous.range.end != join.range.start
                || next.is_none_or(|next| {
                    next.range.start != join.range.end
                        || !matches!(next.kind, K::Ident | K::Integer)
                })
            {
                break;
            }
            self.bump();
            self.bump();
        }
        Ok(())
    }
    fn block(&mut self, repeat: bool) -> PResult {
        self.start(K::Block);
        let result = (|| {
            self.require("{")?;
            if self.block_depth == 256 {
                self.diagnostic(C::Nesting, "blocks are nested too deeply");
                self.skip_block_tail();
                return Err(());
            }
            self.block_depth += 1;
            let scope = self.type_names.len();
            let mut first = repeat;
            while !self.eof() && !self.at("}") {
                self.statement(false, first);
                first = false;
            }
            let result = self.require("}");
            self.type_names.truncate(scope);
            self.block_depth -= 1;
            result
        })();
        self.finish();
        result
    }
    fn if_contents(&mut self) -> PResult {
        self.expression()?;
        self.block(false)?;
        if self.eat("else") {
            if self.at("if") {
                if self.block_depth == 256 {
                    return self.fail(C::Nesting, "blocks are nested too deeply");
                }
                self.block_depth += 1;
                self.statement(false, false);
                self.block_depth -= 1;
            } else {
                self.block(false)?;
            }
        }
        Ok(())
    }
    fn sections(&mut self) -> PResult {
        while self.at("until") || self.at("skip") {
            if self.eat("until") {
                self.terminators()?;
            } else {
                self.bump();
                self.require("other")?;
            }
        }
        self.require("{")?;
        while !self.eof() && !self.at("}") {
            self.trivia();
            self.start(K::Arm);
            let result: PResult = (|| {
                self.pattern()?;
                self.eat("optional");
                self.eat("last");
                if self.at("{") {
                    self.block(false)?;
                }
                Ok(())
            })();
            if result.is_err() {
                self.start(K::Error);
                while !self.eof() && !self.at("}") && self.kind() != Some(K::String) {
                    self.bump();
                }
                self.finish();
            }
            self.finish();
        }
        self.require("}")
    }
    fn statement_ahead(&self) -> bool {
        self.declaration_ahead()
            || (self.kind() == Some(K::Ident)
                && matches!(
                    self.text(),
                    "format"
                        | "comment"
                        | "stop"
                        | "tokens"
                        | "separators"
                        | "ignore"
                        | "expect"
                        | "param"
                        | "list"
                        | "type"
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
                ))
    }
    fn recover_statement(&mut self, before: usize, top: bool) {
        self.start(K::Error);
        let mut depth = 0usize;
        while !self.eof() {
            if depth == 0
                && ((self.at("}") && !top) || (self.position() > before && self.statement_ahead()))
            {
                break;
            }
            if self.at("{") || self.at("[") || self.at("(") {
                depth += 1;
            }
            if self.at("}") || self.at("]") || self.at(")") {
                depth = depth.saturating_sub(1);
            }
            self.bump();
        }
        self.finish();
    }
    fn skip_block_tail(&mut self) {
        self.start(K::Error);
        let mut depth = 1;
        while !self.eof() {
            if self.at("{") {
                depth += 1;
            }
            if self.at("}") {
                depth -= 1;
            }
            self.bump();
            if depth == 0 {
                break;
            }
        }
        self.finish();
    }
}
