use super::*;
use crate::input::{Line, is_line_blank, is_word_byte};
use iris_sema::ReadingKind;

/// Decoded scanner metadata over the existing pattern CST. Bounds retain their
/// original nodes; only a selected match may evaluate them.
#[derive(Debug)]
pub(super) struct Pattern {
    pub source: String,
    items: Vec<Item>,
}
#[derive(Debug)]
enum Item {
    Literal(Vec<u8>),
    Blank {
        required: bool,
    },
    Ellipsis,
    Reference(Name),
    Capture {
        name: Option<Name>,
        kind: ReadingKind,
        domain: Option<SyntaxNode>,
    },
}
impl Pattern {
    pub fn new(
        node: &SyntaxNode,
        kinds: &HashMap<TextRange, ReadingKind>,
        names: &HashMap<TextRange, Name>,
    ) -> Self {
        let raw = node.text().to_string();
        let decoded = DecodedString::new(&raw);
        let base = usize::from(node.text_range().start());
        let mut boundary = 0;
        let mut items = vec![];
        for item in node.children() {
            let start = usize::from(item.text_range().start()) - base;
            let end = usize::from(item.text_range().end()) - base;
            while decoded.raw_boundaries[boundary] < start {
                boundary += 1;
            }
            let from = boundary;
            while decoded.raw_boundaries[boundary] < end {
                boundary += 1;
            }
            items.push(match item.kind() {
                K::PatternText => Item::Literal(decoded.text.as_bytes()[from..boundary].to_vec()),
                K::PatternBlank => Item::Blank { required: false },
                K::Ellipsis => Item::Ellipsis,
                K::CurrentValue => Item::Reference(
                    names[&tokens(&item)
                        .into_iter()
                        .find(|t| t.kind() == K::Ident)
                        .expect("reference name")
                        .text_range()]
                        .clone(),
                ),
                K::Capture => Item::capture(&item, kinds, names),
                _ => unreachable!("pattern item"),
            });
        }
        for index in 0..items.len() {
            if matches!(items[index], Item::Blank { .. }) {
                let required = index > 0
                    && index + 1 < items.len()
                    && items[index - 1].word_boundary(false)
                    && items[index + 1].word_boundary(true);
                items[index] = Item::Blank { required };
            }
        }
        Self {
            source: decoded.text,
            items,
        }
    }
}
impl Item {
    fn capture(
        item: &SyntaxNode,
        kinds: &HashMap<TextRange, ReadingKind>,
        names: &HashMap<TextRange, Name>,
    ) -> Self {
        let name = tokens(item)
            .into_iter()
            .find(|t| t.kind() == K::Ident)
            .map(|t| names[&t.text_range()].clone());
        let domain = item.children().find(|n| n.kind() == K::Type);
        let kind = domain.as_ref().map_or(ReadingKind::Word, |domain| {
            let contents = domain.first_child().expect("type contents");
            match contents.kind() {
                K::Range => ReadingKind::Range,
                K::TypeName => {
                    let name = &tokens(&contents)[0];
                    kinds[&name.text_range()]
                }
                K::BaseType => match tokens(&contents)[0].text() {
                    "int" => ReadingKind::Int,
                    "float" => ReadingKind::Float,
                    "num" => ReadingKind::Num,
                    "word" => ReadingKind::Word,
                    "rest" => ReadingKind::Rest,
                    _ => unreachable!("capture base type"),
                },
                _ => unreachable!("admitted capture type"),
            }
        });
        Item::Capture { name, kind, domain }
    }
    fn word_boundary(&self, first: bool) -> bool {
        match self {
            Self::Capture { .. } | Self::Reference(_) => true,
            Self::Literal(bytes) => {
                if first { bytes.first() } else { bytes.last() }.is_some_and(|&b| is_word_byte(b))
            }
            _ => false,
        }
    }
}
pub(super) struct Capture<'i> {
    item: usize,
    token: Token<'i>,
    value: CapturedValue,
}
enum CapturedValue {
    Int(i64),
    Float(f64),
    String,
}
impl Capture<'_> {
    fn value(
        &self,
        control: &crate::cancellation::Control,
    ) -> Result<Value, crate::cancellation::Cancelled> {
        Ok(match self.value {
            CapturedValue::Int(n) => Value::int(n),
            CapturedValue::Float(n) => Value::float(n),
            CapturedValue::String => Value::string(
                control
                    .copy_utf8(self.token.bytes)?
                    .expect("selected string validated"),
            ),
        })
    }
}
impl<'i> Interpreter<'_, 'i> {
    pub(super) fn execute_pattern(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let pattern = &self.spec.index.patterns[&child(node, K::Pattern).text_range()];
        let find = node.kind() == K::Find;
        loop {
            if let Some(line) = self.input.remaining_line()? {
                if let Some(captures) = self.match_pattern(pattern, &line)? {
                    return self.take_pattern(pattern, &line, captures);
                }
                if find {
                    self.input.consume_line(&line)?;
                    continue;
                }
            }
            let message = if find {
                self.input.control.text_message(&[
                    "no line matching \"",
                    &pattern.source,
                    "\" found",
                ])?
            } else {
                self.input.control.text_message(&[
                    "expected a line matching \"",
                    &pattern.source,
                    "\"",
                    if self.input.peek_byte().is_none() {
                        ", found end of input"
                    } else {
                        ""
                    },
                ])?
            };
            return Err(self.error(
                if find {
                    ErrorCode::FindMatch
                } else {
                    ErrorCode::LineMatch
                },
                message,
            ));
        }
    }
    pub(super) fn match_pattern(
        &self,
        pattern: &Pattern,
        line: &Line<'i>,
    ) -> Result<Option<Vec<Capture<'i>>>, Failure> {
        let mut offset = 0;
        let mut captures = vec![];
        for (index, item) in pattern.items.iter().enumerate() {
            self.input.control.check()?;
            match item {
                Item::Literal(literal) => {
                    if !self
                        .input
                        .control
                        .starts_with(&line.bytes[offset..], literal)?
                    {
                        return Ok(None);
                    }
                    offset += literal.len();
                }
                Item::Blank { required } => {
                    let start = offset;
                    while line.bytes.get(offset).is_some_and(|&b| is_line_blank(b)) {
                        if offset.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
                            self.input.control.check()?;
                        }
                        offset += 1;
                    }
                    if *required && offset == start && offset != line.bytes.len() {
                        return Ok(None);
                    }
                }
                Item::Ellipsis => offset = line.bytes.len(),
                Item::Reference(name) => {
                    let value = self.lookup(name.id).ok_or_else(|| {
                        match self
                            .input
                            .control
                            .text_message(&["'", name, "' is not defined yet"])
                        {
                            Ok(message) => {
                                self.error_at(ErrorCode::NameUndefined, message, line.start)
                            }
                            Err(cancelled) => cancelled.into(),
                        }
                    })?;
                    let integer;
                    let bytes = if let Ok(n) = value.root().as_int() {
                        integer = n.to_string();
                        integer.as_bytes()
                    } else if let Ok(s) = value.root().as_str() {
                        s.as_bytes()
                    } else {
                        return Err(self.error_at(
                            ErrorCode::PatternReference,
                            self.input.control.text_message(&[
                                "'",
                                name,
                                "' must be a string or an integer",
                            ])?,
                            line.start,
                        ));
                    };
                    if bytes.is_empty()
                        || !self
                            .input
                            .control
                            .starts_with(&line.bytes[offset..], bytes)?
                    {
                        return Ok(None);
                    }
                    offset += bytes.len();
                }
                Item::Capture { kind, .. } => {
                    let start = offset;
                    let stop = match pattern.items.get(index + 1) {
                        Some(Item::Literal(bytes)) => bytes[0],
                        _ => 0,
                    };
                    offset = capture_end(*kind, line.bytes, offset, stop, &self.input.control)?;
                    let bytes = &line.bytes[start..offset];
                    if bytes.is_empty() {
                        return Ok(None);
                    }
                    let value = match kind {
                        ReadingKind::Word | ReadingKind::Rest => Some(CapturedValue::String),
                        ReadingKind::Int | ReadingKind::Range => {
                            numeric::parse_integer_controlled(bytes, &self.input.control)?
                                .map(CapturedValue::Int)
                        }
                        ReadingKind::Float => {
                            numeric::parse_float_controlled(bytes, &self.input.control)?
                                .map(CapturedValue::Float)
                        }
                        ReadingKind::Num => {
                            match numeric::parse_integer_controlled(bytes, &self.input.control)? {
                                Some(n) => Some(CapturedValue::Int(n)),
                                None => {
                                    numeric::parse_float_controlled(bytes, &self.input.control)?
                                        .map(CapturedValue::Float)
                                }
                            }
                        }
                        ReadingKind::Alphabet => {
                            unreachable!("alphabet capture rejected at admission")
                        }
                    };
                    let Some(value) = value else {
                        return Ok(None);
                    };
                    captures.push(Capture {
                        item: index,
                        token: Token {
                            bytes,
                            start: line.position(start),
                            end: line.position(offset),
                        },
                        value,
                    });
                }
            }
        }
        Ok((offset == line.bytes.len()).then_some(captures))
    }
    pub(super) fn take_pattern(
        &mut self,
        pattern: &Pattern,
        line: &Line<'i>,
        captures: Vec<Capture<'i>>,
    ) -> Result<(), Failure> {
        // Validate every selected string before publishing any name.
        for capture in &captures {
            if matches!(capture.value, CapturedValue::String) {
                self.input
                    .control
                    .utf8(capture.token.bytes)?
                    .map_err(|offset| {
                        self.error_at(
                            ErrorCode::Utf8,
                            "string value is not valid UTF-8",
                            line.position(capture.token.start.offset - line.start.offset + offset),
                        )
                    })?;
            }
        }
        for capture in &captures {
            if let Item::Capture {
                name: Some(name), ..
            } = &pattern.items[capture.item]
                && &**name != "_"
            {
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .set_controlled(
                        name.clone(),
                        capture.value(&self.input.control)?,
                        &self.input.control,
                    )?;
            }
        }
        for capture in &captures {
            if let Item::Capture {
                kind: ReadingKind::Range,
                domain: Some(domain),
                ..
            } = &pattern.items[capture.item]
            {
                let domain = self.evaluate_domain(domain)?;
                self.validate_range(
                    &domain,
                    &capture.value(&self.input.control)?,
                    &capture.token,
                )?;
            }
        }
        self.input.consume_line(line)?;
        Ok(())
    }
    pub(super) fn execute_skip(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let count = if node.children().any(|n| is_expression(n.kind())) {
            self.admit_count(&expression(node), "number of lines")?
        } else {
            1
        };
        for _ in 0..count {
            if !self.input.skip_line()? {
                return Err(self.error(ErrorCode::SkipEof, "cannot skip a line at end of input"));
            }
        }
        Ok(())
    }
    pub(super) fn execute_expect(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        for literal in tokens(node).iter().filter(|t| t.kind() == K::String) {
            let expected = &self.spec.index.string_literals[&literal.text_range()];
            let Some(token) = self.input.next_token()? else {
                return Err(self.error(
                    ErrorCode::ExpectedToken,
                    self.input.control.text_message(&[
                        "expected '",
                        expected,
                        "', found end of input",
                    ])?,
                ));
            };
            if !self
                .input
                .control
                .equal_bytes(token.bytes, expected.as_bytes())?
            {
                let mut message =
                    self.input
                        .control
                        .text_message(&["expected '", expected, "', found '"])?;
                self.input.control.push_text(&mut message, token.bytes)?;
                message.push('\'');
                let mut error =
                    self.language_error_at(ErrorCode::ExpectedToken, message, token.start)?;
                error.set_input_end(token.end.offset);
                return Err(error.into());
            }
        }
        Ok(())
    }
}
fn capture_end(
    kind: ReadingKind,
    bytes: &[u8],
    mut offset: usize,
    stop: u8,
    control: &crate::cancellation::Control,
) -> Result<usize, crate::cancellation::Cancelled> {
    Ok(match kind {
        ReadingKind::Rest => bytes.len(),
        ReadingKind::Word => {
            while bytes
                .get(offset)
                .is_some_and(|&b| !is_line_blank(b) && b != stop)
            {
                if control.is_active() && offset.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
                    control.check()?;
                }
                offset += 1;
            }
            offset
        }
        _ => {
            if matches!(bytes.get(offset), Some(b'+' | b'-')) {
                if control.is_active() && offset.is_multiple_of(crate::cancellation::SCAN_CHUNK) {
                    control.check()?;
                }
                offset += 1;
            }
            let float = matches!(kind, ReadingKind::Float | ReadingKind::Num);
            while let Some(&byte) = bytes.get(offset) {
                if byte.is_ascii_digit()
                    || (float
                        && (matches!(byte, b'.' | b'e' | b'E')
                            || (matches!(byte, b'+' | b'-')
                                && matches!(bytes.get(offset - 1), Some(b'e' | b'E')))))
                {
                    if control.is_active() && offset.is_multiple_of(crate::cancellation::SCAN_CHUNK)
                    {
                        control.check()?;
                    }
                    offset += 1;
                } else {
                    break;
                }
            }
            offset
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_validation_waits_for_selection_and_precedes_publication() {
        // The first separator byte splits a valid character after whole-input
        // admission. A failed candidate must not validate or publish its slice.
        let spec = Spec::parse(
            "separators \"Ã\" sections {\"{x}?\" optional \"...\" last}".as_bytes(),
            "spec",
        )
        .unwrap();
        let data = spec
            .read("é!".as_bytes(), "input", &crate::Params::new())
            .unwrap();
        assert!(data.entries().is_empty());
        for capture in ["x", "_", ":word"] {
            let source = format!("separators \"Ã\" line \"{{{capture}}}!\"");
            let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
            let mut reader = Interpreter::new(&spec, Input::new("é!".as_bytes(), "input")).unwrap();
            let Failure::Language(error) = reader.execute(spec.parsed.syntax()).unwrap_err() else {
                panic!("language error")
            };
            assert_eq!(error.code, ErrorCode::Utf8);
            assert_eq!(error.input_span().unwrap().bytes, 1..1);
            assert_eq!(error.input_location().unwrap().column, Some(2));
            assert!(reader.frames[0].is_empty());
            assert_eq!(reader.input.position().offset, 1);
        }
        let spec = Spec::parse(b"line \"{$missing}\"", "spec").unwrap();
        let Failure::Language(error) = spec
            .read(b"\xff", "input", &crate::Params::new())
            .unwrap_err()
        else {
            panic!("language error")
        };
        assert_eq!(error.code, ErrorCode::Utf8);
        assert!(error.spec_location().is_none());
    }
    #[test]
    fn selected_ranges_and_post_body_hooks_keep_execution_order() {
        let spec = Spec::parse(b"a[2]:int\nfind \"A {x:0..sum(a)} Z\"\nrepeat 1 {\nsections {\"END\" last {\nrepeat 1 {\nif 0 {} else {}\n}\n}}\n}", "spec").unwrap();
        let mut reader =
            Interpreter::new(&spec, Input::new(b"1 2\nA 99 NO\nA 2 Z\nEND", "input")).unwrap();
        reader.execute(spec.parsed.syntax()).unwrap();
        assert_eq!(
            reader.charges,
            vec![
                Charge {
                    expression: true,
                    units: 2,
                    statement_line: Some(2),
                    context: None
                },
                Charge {
                    expression: false,
                    units: 1,
                    statement_line: Some(6),
                    context: Some("repeat [1 of 1] > section \"END\" > repeat [1 of 1]".into())
                },
            ]
        );
        let spec = Spec::parse(
            b"a[2]:int type t=0..sum(a) repeat 2 {line \"{x:t}\"}",
            "spec",
        )
        .unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"1 2\n1\n2", "input")).unwrap();
        reader.execute(spec.parsed.syntax()).unwrap();
        assert_eq!(reader.charges.len(), 1);
        assert!(reader.charges[0].expression);
        assert_eq!(reader.charges[0].units, 2);
    }
}
