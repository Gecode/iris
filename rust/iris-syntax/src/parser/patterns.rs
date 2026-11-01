use super::{C, Check, K, PResult, Parser, Stream};
use crate::lexer::{decode_string, is_name, lex};
use std::ops::Range;

struct PatternItem {
    range: Range<usize>,
    kind: PatternItemKind,
}
enum PatternItemKind {
    Text,
    Blank,
    Ellipsis,
    Capture {
        name: Range<usize>,
        capture_type: Option<Range<usize>>,
    },
    CurrentValue {
        name: Range<usize>,
    },
}

impl Parser<'_> {
    pub(super) fn pattern(&mut self) -> PResult {
        if self.kind() != Some(K::String) {
            return self.fail(C::Syntax, "expected a line pattern (a string)");
        }
        self.trivia();
        let token = self.token().expect("pattern string").clone();
        let (decoded, boundaries) = decode_string(&self.stream.text[token.range.clone()]);
        let offsets: Vec<usize> = boundaries
            .into_iter()
            .map(|offset| self.stream.raw_offset(token.range.start + offset))
            .collect();
        let raw = self.stream.raw_range(token.range.clone());
        self.start(K::Pattern);
        self.raw_token(K::Quote, raw.start..raw.start + 1);
        // Complete structural validation precedes every capture's sub-lexer.
        let result = match scan_pattern(&decoded) {
            Err(message) => {
                self.diagnostic_at(C::Pattern, message, &token);
                self.start(K::Error);
                self.raw_token(
                    K::PatternToken,
                    offsets[0]..*offsets.last().expect("nonempty boundary map"),
                );
                self.finish();
                Err(())
            }
            Ok(items) => {
                let mut valid = true;
                for item in items {
                    if self
                        .pattern_item(item, &decoded, &offsets, token.line)
                        .is_err()
                    {
                        valid = false;
                    }
                }
                if valid { Ok(()) } else { Err(()) }
            }
        };
        self.raw_token(K::Quote, raw.end - 1..raw.end);
        self.finish();
        self.stream.pos += 1;
        result
    }
    fn pattern_item(
        &mut self,
        item: PatternItem,
        decoded: &str,
        offsets: &[usize],
        line: usize,
    ) -> PResult {
        let mut valid = true;
        match item.kind {
            PatternItemKind::Text | PatternItemKind::Blank | PatternItemKind::Ellipsis => {
                let kind = match item.kind {
                    PatternItemKind::Text => K::PatternText,
                    PatternItemKind::Blank => K::PatternBlank,
                    _ => K::Ellipsis,
                };
                self.start(kind);
                self.pattern_leaf(K::PatternToken, item.range, offsets);
                self.finish();
            }
            PatternItemKind::CurrentValue { name } => {
                self.start(K::CurrentValue);
                self.pattern_leaf(K::Punct, item.range.start..item.range.start + 1, offsets);
                self.pattern_leaf(K::Punct, item.range.start + 1..name.start, offsets);
                self.pattern_leaf(K::Ident, name.clone(), offsets);
                self.pattern_leaf(K::Punct, name.end..item.range.end, offsets);
                self.finish();
            }
            PatternItemKind::Capture { name, capture_type } => {
                self.start(K::Capture);
                self.pattern_leaf(K::Punct, item.range.start..name.start, offsets);
                self.pattern_leaf(K::Ident, name.clone(), offsets);
                if let Some(capture_type) = capture_type {
                    self.pattern_leaf(K::Punct, name.end..capture_type.start, offsets);
                    if self
                        .capture_type(
                            &decoded[capture_type.clone()],
                            &offsets[capture_type.start..=capture_type.end],
                            line,
                        )
                        .is_err()
                    {
                        valid = false;
                    }
                    self.pattern_leaf(K::Punct, capture_type.end..item.range.end, offsets);
                } else {
                    self.pattern_leaf(K::Punct, name.end..item.range.end, offsets);
                }
                self.finish();
            }
        }
        if valid { Ok(()) } else { Err(()) }
    }
    fn pattern_leaf(&mut self, kind: K, range: Range<usize>, offsets: &[usize]) {
        if !range.is_empty() {
            self.raw_token(kind, offsets[range.start]..offsets[range.end]);
        }
    }
    fn capture_type(&mut self, text: &str, offsets: &[usize], line: usize) -> PResult {
        let mut lexed = lex(text, self.origin.clone());
        for token in &mut lexed.tokens {
            token.line += line - 1;
        }
        for mut diagnostic in lexed.diagnostics {
            let start = usize::from(diagnostic.range.start());
            let end = usize::from(diagnostic.range.end());
            diagnostic.range = crate::diagnostic::text_range(offsets[start], offsets[end]);
            diagnostic.location.line += line - 1;
            diagnostic.phase = crate::DiagnosticPhase::Parsing;
            self.checks.push(crate::ParseCheck {
                kind: Check::Diagnostic(self.diagnostics.len()),
                target: diagnostic.range.start(),
                range: diagnostic.range,
                location: diagnostic.location.clone(),
            });
            self.diagnostics.push(diagnostic);
        }
        let mut stream = Stream::new(text.into(), lexed.tokens);
        stream.raw_offsets = Some(offsets.to_vec());
        stream.eof_line += line - 1;
        let outer = std::mem::replace(&mut self.stream, stream);
        let result = if self.eof() {
            self.report(
                C::Pattern,
                "a capture needs a type after ':'".into(),
                offsets[text.len()]..offsets[text.len()],
                line,
            );
            Err(())
        } else {
            let target = self.raw_position();
            self.read_type().and_then(|_| {
                if self.eof() {
                    self.checkpoint(Check::CaptureForm, target);
                    Ok(())
                } else {
                    self.fail(
                        C::Pattern,
                        format!("unexpected {} in the capture type '{text}'", self.found()),
                    )
                }
            })
        };
        if !self.eof() {
            self.start(K::Error);
            while !self.eof() {
                self.bump();
            }
            self.finish();
        }
        self.trivia();
        self.stream = outer;
        result
    }
}

fn scan_pattern(text: &str) -> Result<Vec<PatternItem>, String> {
    let mut items = Vec::new();
    let mut offset = 0;
    while offset < text.len() {
        let start = offset;
        let kind = if text[offset..].starts_with("...") {
            if !text[offset + 3..]
                .bytes()
                .all(|byte| matches!(byte, b' ' | b'\t'))
            {
                return Err(format!("'...' must end the pattern \"{text}\""));
            }
            offset = text.len();
            PatternItemKind::Ellipsis
        } else if matches!(text.as_bytes()[offset], b' ' | b'\t' | b'\r') {
            offset += 1;
            while offset < text.len() && matches!(text.as_bytes()[offset], b' ' | b'\t' | b'\r') {
                offset += 1;
            }
            PatternItemKind::Blank
        } else if text.as_bytes()[offset] == b'{' {
            let end = offset
                + text[offset..]
                    .find('}')
                    .ok_or_else(|| format!("unterminated capture in pattern \"{text}\""))?;
            let inside = &text[offset + 1..end];
            let kind = if let Some(name) = inside.strip_prefix('$') {
                if !is_name(name) {
                    return Err(format!("missing or bad name in pattern \"{text}\""));
                }
                PatternItemKind::CurrentValue {
                    name: offset + 2..end,
                }
            } else {
                let name_end = inside.find(':').map_or(end, |colon| offset + 1 + colon);
                let name = offset + 1..name_end;
                if !name.is_empty() && !is_name(&text[name.clone()]) {
                    return Err(format!(
                        "capture name '{}' in pattern \"{text}\" is not a name",
                        &text[name.clone()]
                    ));
                }
                let capture_type = (name_end < end).then_some(name_end + 1..end);
                PatternItemKind::Capture { name, capture_type }
            };
            offset = end + 1;
            kind
        } else {
            offset += text[offset..].chars().next().expect("not EOF").len_utf8();
            while offset < text.len()
                && !matches!(text.as_bytes()[offset], b'{' | b' ' | b'\t' | b'\r')
                && !text[offset..].starts_with("...")
            {
                offset += text[offset..].chars().next().expect("not EOF").len_utf8();
            }
            PatternItemKind::Text
        };
        items.push(PatternItem {
            range: start..offset,
            kind,
        });
    }
    Ok(items)
}
