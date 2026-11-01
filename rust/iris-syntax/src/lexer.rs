use std::{ops::Range, sync::Arc};

use crate::{
    Diagnostic, DiagnosticCode, DiagnosticPhase, SourceLocation, SyntaxKind as K,
    diagnostic::text_range,
};

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub kind: K,
    pub range: Range<usize>,
    pub line: usize,
}

impl Token {
    pub fn is_trivia(&self) -> bool {
        matches!(self.kind, K::Whitespace | K::Comment)
    }
}

pub(crate) struct Lexed {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Scan the complete source before the parser sees any token.
pub(crate) fn lex(source: &str, origin: Arc<str>) -> Lexed {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    let mut offset = 0;
    let mut line = 1;
    while offset < bytes.len() {
        let start = offset;
        let mut error = None;
        let kind = match bytes[offset] {
            b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c => {
                offset += 1;
                while offset < bytes.len() && bytes[offset].is_ascii_whitespace() {
                    offset += 1;
                }
                K::Whitespace
            }
            b'#' => {
                offset += 1;
                while offset < bytes.len() && bytes[offset] != b'\n' {
                    offset += 1;
                }
                K::Comment
            }
            b'"' => {
                let (end, valid) = scan_string(source, offset);
                offset = end;
                if valid {
                    K::String
                } else {
                    error = Some(DiagnosticCode::String);
                    K::Invalid
                }
            }
            b'0'..=b'9' => {
                offset = scan_number(bytes, offset);
                match number_kind(&source[start..offset]) {
                    Some(kind) => kind,
                    None => {
                        error = Some(DiagnosticCode::Number);
                        K::Invalid
                    }
                }
            }
            byte if is_name_start(byte) => {
                offset += 1;
                while offset < bytes.len() && is_name_character(bytes[offset]) {
                    offset += 1;
                }
                if &source[start..offset] == "gecode"
                    && bytes.get(offset) == Some(&b'.')
                    && bytes
                        .get(offset + 1)
                        .is_some_and(|&byte| is_name_character(byte))
                {
                    offset += 2;
                    while offset < bytes.len() && is_name_character(bytes[offset]) {
                        offset += 1;
                    }
                }
                K::Ident
            }
            _ => {
                let rest = &source[offset..];
                if let Some(punct) = ["..<", "..", "==", "!=", "<=", ">=", "&&", "||"]
                    .into_iter()
                    .find(|punct| rest.starts_with(punct))
                {
                    offset += punct.len();
                    K::Punct
                } else {
                    let ch = rest.chars().next().expect("offset precedes EOF");
                    offset += ch.len_utf8();
                    if "{}[]()=<>+-*/%^!,.:\0".contains(ch) {
                        K::Punct
                    } else {
                        error = Some(DiagnosticCode::Character);
                        K::Invalid
                    }
                }
            }
        };
        if let Some(code) = error {
            diagnostics.push(Diagnostic {
                code,
                message: match code {
                    DiagnosticCode::String => "invalid or unterminated reader-spec string".into(),
                    DiagnosticCode::Number => format!("bad number {}", &source[start..offset]),
                    _ => format!("unexpected character {:?}", &source[start..offset]),
                },
                range: text_range(start, offset),
                location: SourceLocation {
                    origin: origin.clone(),
                    line,
                    column: None,
                },
                phase: DiagnosticPhase::Lexing,
            });
        }
        tokens.push(Token {
            kind,
            range: start..offset,
            line,
        });
        line += bytes[start..offset]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count();
    }
    Lexed {
        tokens,
        diagnostics,
    }
}

pub(crate) fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}
pub(crate) fn is_name_character(byte: u8) -> bool {
    is_name_start(byte) || byte.is_ascii_digit()
}
pub(crate) fn is_name(text: &str) -> bool {
    text.as_bytes()
        .first()
        .is_some_and(|&byte| is_name_start(byte))
        && text.bytes().all(is_name_character)
}

fn scan_string(source: &str, start: usize) -> (usize, bool) {
    let bytes = source.as_bytes();
    let mut offset = start + 1;
    let mut valid = true;
    while offset < bytes.len() {
        match bytes[offset] {
            b'"' => return (offset + 1, valid),
            b'\n' => return (offset, false),
            b'\\' => {
                offset += 1;
                match bytes.get(offset) {
                    None | Some(b'\n') => return (offset, false),
                    Some(b'n' | b't' | b'\\' | b'"') => offset += 1,
                    Some(_) => {
                        valid = false;
                        offset += source[offset..].chars().next().expect("not EOF").len_utf8();
                    }
                }
            }
            _ => offset += source[offset..].chars().next().expect("not EOF").len_utf8(),
        }
    }
    (offset, false)
}

fn scan_number(bytes: &[u8], start: usize) -> usize {
    let mut end = start;
    while end < bytes.len() {
        if bytes[end..].starts_with(b"..") {
            break;
        }
        let byte = bytes[end];
        if byte.is_ascii_digit()
            || matches!(byte, b'.' | b'e' | b'E')
            || (matches!(byte, b'+' | b'-') && end > start && matches!(bytes[end - 1], b'e' | b'E'))
        {
            end += 1;
        } else {
            break;
        }
    }
    end
}

fn number_kind(text: &str) -> Option<K> {
    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        return text.parse::<i64>().ok().map(|_| K::Integer);
    }
    let bytes = text.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() && bytes[pos].is_ascii_digit() {
        pos += 1;
    }
    if bytes.get(pos) == Some(&b'.') {
        pos += 1;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
    }
    let significand_end = pos;
    if matches!(bytes.get(pos), Some(b'e' | b'E')) {
        pos += 1;
        if matches!(bytes.get(pos), Some(b'+' | b'-')) {
            pos += 1;
        }
        let exponent_start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == exponent_start {
            return None;
        }
    }
    if pos != bytes.len() {
        return None;
    }
    let value = text.parse::<f64>().ok()?;
    let exact_zero = bytes[..significand_end]
        .iter()
        .all(|&byte| matches!(byte, b'0' | b'.'));
    if !value.is_finite() || (!exact_zero && (value == 0.0 || value.is_subnormal())) {
        None
    } else {
        Some(K::Float)
    }
}

/// Decoded byte boundaries map back to raw source boundaries. Escape spellings
/// remain intact when a pattern or its re-lexed types emit raw leaf text.
pub(crate) fn decode_string(text: &str) -> (String, Vec<usize>) {
    let mut decoded = String::new();
    let mut boundaries = vec![1];
    let mut offset = 1;
    while offset < text.len() - 1 {
        if text.as_bytes()[offset] == b'\\' {
            let ch = match text.as_bytes()[offset + 1] {
                b'n' => '\n',
                b't' => '\t',
                b'\\' => '\\',
                b'"' => '"',
                _ => unreachable!("only lexically valid strings are decoded"),
            };
            decoded.push(ch);
            offset += 2;
            boundaries.push(offset);
        } else {
            let ch = text[offset..].chars().next().expect("inside string");
            decoded.push(ch);
            for byte in 1..=ch.len_utf8() {
                boundaries.push(offset + byte);
            }
            offset += ch.len_utf8();
        }
    }
    (decoded, boundaries)
}
