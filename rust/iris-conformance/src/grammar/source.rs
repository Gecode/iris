//! Source boundaries and conversions; lexical structure comes from the EBNF.
use std::ops::Range;

use super::ebnf::Grammar;
use super::recognize::{Failure, Recognizer, Rejection, Result};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TokenKind {
    Ident,
    Int,
    Float,
    String,
    Punct,
}

#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub text: String,
    pub span: Range<usize>,
    pub line: usize,
}

pub(super) fn lex(grammar: &Grammar, bytes: &[u8], line: usize) -> Result<Vec<Token>> {
    let text = std::str::from_utf8(bytes).map_err(|error| {
        let offset = error.valid_up_to();
        Failure::Rejected(Rejection {
            layer: "source",
            rule: "reader_spec".into(),
            line: line + bytes[..offset].iter().filter(|&&b| b == b'\n').count(),
            offset,
            expected: "valid UTF-8 source".into(),
        })
    })?;
    Recognizer::lex_source(grammar, text, line)
}

pub(super) fn name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

pub(super) fn lexical_rule(text: &str) -> &'static str {
    match text.chars().next() {
        Some('"') => "source_string",
        Some(c) if c.is_ascii_digit() => "number_token",
        Some(c) if name_start(c) => "name_token",
        _ => "punctuation",
    }
}

pub(super) fn token_kind(text: &str) -> TokenKind {
    match text.chars().next() {
        Some('"') => TokenKind::String,
        Some(c) if c.is_ascii_digit() => {
            if text.bytes().all(|b| b.is_ascii_digit()) {
                TokenKind::Int
            } else {
                TokenKind::Float
            }
        }
        Some(c) if name_start(c) => TokenKind::Ident,
        _ => TokenKind::Punct,
    }
}

pub(super) fn name_end(text: &str, at: usize) -> usize {
    let tail = &text[at..];
    let is_name_character = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let prefix = if tail.starts_with("gecode.")
        && tail
            .as_bytes()
            .get(7)
            .is_some_and(|&b| is_name_character(b))
    {
        7
    } else {
        0
    };
    at + prefix
        + tail.as_bytes()[prefix..]
            .iter()
            .take_while(|&&b| is_name_character(b))
            .count()
}

pub(super) fn numeric_end(text: &str, at: usize) -> usize {
    let bytes = text.as_bytes();
    let mut end = at;
    while let Some(&byte) = bytes.get(end) {
        if byte.is_ascii_digit()
            || matches!(byte, b'.' | b'e' | b'E')
            || (matches!(byte, b'+' | b'-') && end > at && matches!(bytes[end - 1], b'e' | b'E'))
        {
            end += 1;
        } else {
            break;
        }
    }
    at + text[at..end].find("..").unwrap_or(end - at)
}

pub(super) fn convert_number(text: &str) -> std::result::Result<(), String> {
    if text.bytes().all(|b| b.is_ascii_digit()) {
        return text
            .parse::<i64>()
            .map(|_| ())
            .map_err(|_| "integer in 0..=i64::MAX".into());
    }
    let value = text
        .parse::<f64>()
        .map_err(|_| "decimal binary64 number".to_string())?;
    let significand = text.split(['e', 'E']).next().unwrap_or(text);
    let nonzero = significand.bytes().any(|b| matches!(b, b'1'..=b'9'));
    if !value.is_finite() || (nonzero && value.abs() < f64::MIN_POSITIVE) {
        return Err("finite normal binary64 number or exact zero".into());
    }
    Ok(())
}

pub(super) fn decode_string(raw: &str) -> String {
    let mut decoded = String::new();
    let mut chars = raw[1..raw.len() - 1].chars();
    while let Some(c) = chars.next() {
        decoded.push(if c == '\\' {
            match chars
                .next()
                .expect("escape production requires a character")
            {
                'n' => '\n',
                't' => '\t',
                c => c,
            }
        } else {
            c
        });
    }
    decoded
}
