//! The notation in Specification §2.2, without an implicit terminal escape syntax.
use std::collections::BTreeMap;

#[derive(Debug)]
pub(super) enum Expr {
    Literal(String),
    Rule(String),
    Sequence(Vec<Expr>),
    Choice(Vec<Expr>),
    Optional(Box<Expr>),
    Repeat(Box<Expr>),
    Special(String),
}

#[derive(Debug)]
pub(super) struct Grammar {
    rules: BTreeMap<String, Expr>,
}

impl Grammar {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut parser = Parser { text, at: 0 };
        let mut rules = BTreeMap::new();
        while parser.peek()?.is_some() {
            let name = parser.name()?;
            parser.require('=')?;
            let expr = parser.expression(';')?;
            parser.require(';')?;
            if rules.insert(name.clone(), expr).is_some() {
                return Err(format!("duplicate production {name}"));
            }
        }
        let grammar = Self { rules };
        for expr in grammar.rules.values() {
            grammar.validate(expr)?;
        }
        Ok(grammar)
    }

    #[cfg(test)]
    pub fn rule_names(&self) -> impl Iterator<Item = &str> {
        self.rules.keys().map(String::as_str)
    }

    pub fn rule(&self, name: &str) -> Result<&Expr, String> {
        self.rules
            .get(name)
            .ok_or_else(|| format!("undefined production {name}"))
    }

    fn validate(&self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Rule(name) => {
                self.rule(name)?;
            }
            Expr::Special(name) if !SPECIALS.contains(&name.as_str()) => {
                return Err(format!("unsupported special sequence ? {name} ?"));
            }
            Expr::Sequence(children) | Expr::Choice(children) => {
                for child in children {
                    self.validate(child)?;
                }
            }
            Expr::Optional(child) | Expr::Repeat(child) => self.validate(child)?,
            _ => {}
        }
        Ok(())
    }

    pub fn punctuation_length(&self, text: &str) -> Result<usize, String> {
        let mut literals = Vec::new();
        self.literals(self.rule("punctuation")?, &mut literals, &mut Vec::new())?;
        Ok(literals
            .iter()
            .filter(|literal| text.starts_with(**literal))
            .map(|literal| literal.len())
            .max()
            .unwrap_or(usize::from(text.starts_with('\0'))))
    }

    fn literals<'a>(
        &'a self,
        expr: &'a Expr,
        literals: &mut Vec<&'a str>,
        active: &mut Vec<&'a str>,
    ) -> Result<(), String> {
        match expr {
            Expr::Literal(text) => literals.push(text),
            Expr::Rule(name) => {
                if active.contains(&name.as_str()) {
                    return Err("recursive punctuation inventory".into());
                }
                active.push(name);
                self.literals(self.rule(name)?, literals, active)?;
                active.pop();
            }
            Expr::Sequence(children) | Expr::Choice(children) => {
                for child in children {
                    self.literals(child, literals, active)?;
                }
            }
            Expr::Optional(child) | Expr::Repeat(child) => {
                self.literals(child, literals, active)?
            }
            Expr::Special(_) => {}
        }
        Ok(())
    }
}

const SPECIALS: &[&str] = &[
    "valid UTF-8 source",
    "glued format join",
    "ordinary top-level dispatch",
    "no declaration",
    "block counter entry",
    "declaration ahead",
    "option ahead",
    "not eol",
    "base type ahead",
    "alphabet type ahead",
    "named type ahead",
    "range type ahead",
    "no upper bound ahead",
    "upper bound ahead",
    "not line",
    "not until",
    "not eof",
    "else if ahead",
    "else-if counter entry",
    "maximal expression and expression counter",
    "host constant ahead",
    "ordinary expression name ahead",
    "comprehension ahead",
    "no comprehension ahead",
    "IDENT token",
    "INT token",
    "FLOAT token",
    "STRING token",
    "end of tokens",
    "ASCII whitespace byte",
    "scalar other than LF",
    "lexical dispatch",
    "scalar other than quote backslash or LF",
    "maximal identifier",
    "ASCII letter or underscore",
    "ASCII digit",
    "maximal numeric candidate",
    "longest punctuation",
    "NUL character",
    "decoded pattern STRING token",
    "tab character",
    "CR character",
    "pattern literal character",
    "re-lexed capture type",
];

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn peek(&mut self) -> Result<Option<char>, String> {
        loop {
            while self.text[self.at..].starts_with(char::is_whitespace) {
                self.at += self.text[self.at..].chars().next().unwrap().len_utf8();
            }
            if !self.text[self.at..].starts_with("(*") {
                break;
            }
            let end = self.text[self.at + 2..]
                .find("*)")
                .ok_or_else(|| self.error("unterminated comment"))?;
            self.at += end + 4;
        }
        Ok(self.text[self.at..].chars().next())
    }

    fn error(&self, message: &str) -> String {
        format!("grammar byte {}: {message}", self.at)
    }

    fn require(&mut self, expected: char) -> Result<(), String> {
        if self.peek()? != Some(expected) {
            return Err(self.error(&format!("expected {expected:?}")));
        }
        self.at += 1;
        Ok(())
    }

    fn name(&mut self) -> Result<String, String> {
        if !self
            .peek()?
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            return Err(self.error("expected production name"));
        }
        let start = self.at;
        while self.text[self.at..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
            self.at += 1;
        }
        Ok(self.text[start..self.at].into())
    }

    fn delimited(&mut self, delimiter: char) -> Result<String, String> {
        self.at += 1;
        let end = self.text[self.at..]
            .find(delimiter)
            .ok_or_else(|| self.error("unterminated terminal or special sequence"))?;
        let text = self.text[self.at..self.at + end].to_string();
        self.at += end + 1;
        Ok(text)
    }

    fn expression(&mut self, close: char) -> Result<Expr, String> {
        let mut choices = Vec::new();
        let mut sequence = Vec::new();
        let mut needs_term = true;
        loop {
            let token = self
                .peek()?
                .ok_or_else(|| self.error("unterminated expression"))?;
            if token == close || token == '|' {
                if needs_term {
                    return Err(self.error("expected expression term"));
                }
                choices.push(if sequence.len() == 1 {
                    sequence.pop().unwrap()
                } else {
                    Expr::Sequence(std::mem::take(&mut sequence))
                });
                if token == close {
                    break;
                }
                self.at += 1;
                needs_term = true;
                continue;
            }
            if token == ',' {
                if needs_term {
                    return Err(self.error("unexpected comma"));
                }
                self.at += 1;
                needs_term = true;
                continue;
            }
            let term = match token {
                '\'' | '"' => Expr::Literal(self.delimited(token)?),
                '?' => Expr::Special(self.delimited('?')?.trim().into()),
                '(' | '[' | '{' => {
                    self.at += 1;
                    let end = match token {
                        '(' => ')',
                        '[' => ']',
                        _ => '}',
                    };
                    let child = self.expression(end)?;
                    self.require(end)?;
                    match token {
                        '(' => child,
                        '[' => Expr::Optional(Box::new(child)),
                        _ => Expr::Repeat(Box::new(child)),
                    }
                }
                c if c.is_ascii_alphabetic() || c == '_' => Expr::Rule(self.name()?),
                _ => return Err(self.error(&format!("unexpected notation character {token:?}"))),
            };
            sequence.push(term);
            needs_term = false;
        }
        Ok(if choices.len() == 1 {
            choices.pop().unwrap()
        } else {
            Expr::Choice(choices)
        })
    }
}
