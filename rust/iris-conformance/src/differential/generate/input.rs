//! The input that the planned statements read, with perturbed tokens.

use super::{
    Arm, Condition, Count, Generator, PatternPart, Plan, ReadType, RepeatBody, Value, WORDS,
};

impl Generator {
    pub(super) fn write_top(&mut self, plan: &Plan) {
        if let Some(prefix) = self.settings.comment
            && self.rng.chance(20)
        {
            self.input
                .extend_from_slice(format!("{prefix} a comment line\n").as_bytes());
        }
        self.write(plan);
    }

    fn write(&mut self, plan: &Plan) {
        match plan {
            Plan::Silent => {}
            Plan::Let { name, value } => {
                self.values.insert(name.clone(), value.clone());
            }
            Plan::Value { name, read, offset } => {
                let (token, value) = self.value_token(read);
                let perturbed = if matches!(read, ReadType::Rest) {
                    self.raw_line(&token)
                } else {
                    self.token_line(&[token])
                };
                let value = match value {
                    _ if perturbed => Value::Other,
                    Value::Int(number) => Value::Int(number + offset),
                    value => value,
                };
                self.values.insert(name.clone(), value);
            }
            Plan::Array {
                name,
                read,
                shape,
                layout,
            } => {
                self.values.insert(name.clone(), Value::Other);
                let rows = self.count_value(&shape[0]);
                let cells = match (shape.get(1), layout) {
                    (None, _) => rows,
                    (Some(_), Some(layout)) => layout.cells(rows),
                    (Some(columns), None) => rows * self.count_value(columns),
                };
                let tokens: Vec<String> = (0..cells).map(|_| self.value_token(read).0).collect();
                for chunk in tokens.chunks(4) {
                    self.token_line(chunk);
                }
            }
            Plan::Alphabet {
                name,
                rows,
                columns,
                symbols,
                strip,
            } => {
                self.values.insert(name.clone(), Value::Other);
                let rows = self.count_value(rows);
                let columns = self.count_value(columns);
                for _ in 0..rows {
                    let mut line = Vec::new();
                    for _ in 0..columns {
                        if let Some(strip) = strip
                            && self.rng.chance(20)
                        {
                            line.push(*strip);
                        }
                        line.push(*self.rng.pick(symbols));
                    }
                    self.perturb_bytes(&mut line);
                    line.push(b'\n');
                    self.input.extend_from_slice(&line);
                }
            }
            Plan::ListEol { name, read } => {
                self.values.insert(name.clone(), Value::Other);
                let count = self.rng.between(1, 4);
                let tokens: Vec<String> = (0..count).map(|_| self.value_token(read).0).collect();
                self.token_line(&tokens);
            }
            Plan::ListSentinel {
                name,
                read,
                sentinel,
            } => {
                self.values.insert(name.clone(), Value::Other);
                let mut tokens: Vec<String> = (0..self.rng.below(4))
                    .map(|_| self.value_token(read).0)
                    .filter(|token| token != sentinel)
                    .collect();
                tokens.push(sentinel.clone());
                self.token_line(&tokens);
            }
            Plan::Expect(tokens) => {
                self.token_line(tokens);
            }
            Plan::Line { find, parts } => {
                if *find {
                    for _ in 0..self.rng.below(3) {
                        self.raw_line("zz noise line");
                    }
                }
                let line = self.pattern_line(parts);
                self.raw_line(&line);
            }
            Plan::Skip(lines) => {
                for _ in 0..*lines {
                    self.raw_line("skipped words 1 2");
                }
            }
            Plan::Index(base) => {
                let iteration = self.iterations.last().copied().unwrap_or(0);
                self.token_line(&[(iteration + base).to_string()]);
            }
            Plan::Repeat { count, body } => {
                let count = self.count_value(count);
                self.write_iterations(body, count);
            }
            Plan::RepeatUntil {
                terminator,
                store,
                body,
            } => {
                let count = self.rng.below(4);
                self.write_iterations(body, count);
                self.token_line(std::slice::from_ref(terminator));
                if let Some(store) = store {
                    self.values
                        .insert(store.clone(), Value::Word(terminator.clone()));
                }
            }
            Plan::RepeatEof { body } => {
                let count = self.rng.below(4);
                self.write_iterations(body, count);
            }
            Plan::Sections {
                arms,
                terminator,
                skip_other,
            } => self.write_sections(arms, terminator.as_deref(), *skip_other),
            Plan::If {
                branches,
                otherwise,
            } => {
                let taken = branches
                    .iter()
                    .find(|(condition, _)| self.holds(condition))
                    .map_or(otherwise, |(_, body)| body)
                    .clone();
                for plan in &taken {
                    self.write(plan);
                }
            }
            Plan::Instances { body, count } => {
                let saved = self.values.clone();
                for _ in 0..*count {
                    for plan in body {
                        self.write(plan);
                    }
                    self.values.clone_from(&saved);
                }
            }
        }
    }

    fn write_iterations(&mut self, body: &RepeatBody, count: usize) {
        let saved = self.values.clone();
        for iteration in 0..count {
            self.iterations.push(iteration as i64);
            for plan in &body.plans {
                self.write(plan);
            }
            self.iterations.pop();
        }
        // Body values become columns, whose values the generator forgets.
        self.values = saved;
        for column in &body.columns {
            self.values.insert(column.clone(), Value::Other);
        }
    }

    fn write_sections(&mut self, arms: &[Arm], terminator: Option<&str>, skip_other: bool) {
        let mut order: Vec<&Arm> = arms
            .iter()
            .filter(|arm| !arm.last && (!arm.optional || self.rng.chance(60)))
            .collect();
        for index in (1..order.len()).rev() {
            let other = self.rng.below(index + 1);
            order.swap(index, other);
        }
        order.extend(arms.iter().filter(|arm| arm.last));
        for arm in order {
            if skip_other && self.rng.chance(30) {
                self.raw_line("zz other line");
            }
            let line = self.pattern_line(&arm.parts);
            debug_assert!(line.starts_with(&arm.head));
            self.raw_line(&line);
            for plan in &arm.body {
                self.write(plan);
            }
            if arm.last {
                return;
            }
        }
        if let Some(terminator) = terminator {
            self.token_line(&[terminator.to_string()]);
        }
    }

    fn holds(&self, condition: &Condition) -> bool {
        match condition {
            Condition::Constant(value) => *value,
            Condition::Defined(name) => self.values.contains_key(name),
            Condition::Compare { name, op, value } => match self.values.get(name) {
                Some(Value::Int(known)) => op.holds(*known, *value),
                _ => false,
            },
        }
    }

    /// The text of a line that `parts` matches.
    fn pattern_line(&mut self, parts: &[PatternPart]) -> String {
        let mut line = String::new();
        for part in parts {
            match part {
                PatternPart::Literal(text) => {
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(text);
                }
                PatternPart::Capture { name, read, glued } => {
                    if !glued {
                        line.push(' ');
                    }
                    let (token, value) = self.value_token(read);
                    line.push_str(&token);
                    if let Some(name) = name {
                        self.values.insert(name.clone(), value);
                    }
                }
                PatternPart::Reference(name) => {
                    line.push(' ');
                    match self.values.get(name) {
                        Some(Value::Int(value)) => line.push_str(&value.to_string()),
                        Some(Value::Word(word)) => line.push_str(word),
                        _ => line.push_str("unknown"),
                    }
                }
                PatternPart::Ellipsis => line.push_str(" anything 1 2"),
            }
        }
        line
    }

    fn count_value(&self, count: &Count) -> usize {
        let value = match count {
            Count::Literal(value) => *value,
            Count::Name(name) => match self.values.get(name) {
                Some(Value::Int(value)) => *value,
                _ => 1,
            },
        };
        value.clamp(0, 6) as usize
    }

    /// A token that `read` accepts, and its value.
    fn value_token(&mut self, read: &ReadType) -> (String, Value) {
        match read {
            ReadType::Int | ReadType::Num if self.rng.chance(75) => {
                let value = self.rng.between(-5, 40);
                (value.to_string(), Value::Int(value))
            }
            ReadType::Int => {
                let value = *self.rng.pick(&[0, 1, -1, 100, 65_536, i64::from(i32::MAX)]);
                (value.to_string(), Value::Int(value))
            }
            ReadType::Float | ReadType::Num => {
                let text = *self
                    .rng
                    .pick(&["1.5", "-2", "3e2", "0.25", "-0.0", "7", "1E-3"]);
                (text.into(), Value::Other)
            }
            ReadType::Word => {
                let word = (*self.rng.pick(&WORDS)).to_string();
                (word.clone(), Value::Word(word))
            }
            ReadType::Rest => {
                let text = *self.rng.pick(&["rest of the line", "x", "a  b\tc", "é 1"]);
                (text.into(), Value::Word(text.into()))
            }
            ReadType::Range {
                low,
                high,
                exclusive,
            } => {
                let low = low.as_ref().map(|low| self.bound_value(low));
                let high = high
                    .as_ref()
                    .map(|high| self.bound_value(high) - i64::from(*exclusive));
                let value = match (low, high) {
                    (Some(low), Some(high)) if low <= high => self.rng.between(low, high),
                    (Some(low), None) => low + self.rng.between(0, 9),
                    (None, Some(high)) => high - self.rng.between(0, 9),
                    _ => self.rng.between(0, 9),
                };
                (value.to_string(), Value::Int(value))
            }
        }
    }

    fn bound_value(&self, bound: &Count) -> i64 {
        match bound {
            Count::Literal(value) => *value,
            Count::Name(name) => match self.values.get(name) {
                Some(Value::Int(value)) => *value,
                _ => 0,
            },
        }
    }

    /// Write `tokens` as one input line, joined by the separator, each
    /// perturbed with a small probability. Returns whether any was.
    fn token_line(&mut self, tokens: &[String]) -> bool {
        let separator = match self.settings.separator {
            Some(separator) if self.rng.chance(50) => separator,
            _ => " ",
        };
        let mut perturbed = false;
        let mut line = Vec::new();
        for (position, token) in tokens.iter().enumerate() {
            if position > 0 {
                line.extend_from_slice(separator.as_bytes());
            }
            let mut bytes = token.as_bytes().to_vec();
            if self.rng.chance(2) {
                bytes = self.perturbed_token(&bytes);
                perturbed = true;
            }
            line.extend_from_slice(&bytes);
        }
        line.push(b'\n');
        self.input.extend_from_slice(&line);
        perturbed
    }

    /// Write `text` as one input line, perturbed as a whole with a small
    /// probability. Returns whether it was.
    fn raw_line(&mut self, text: &str) -> bool {
        let mut bytes = text.as_bytes().to_vec();
        let perturbed = self.perturb_bytes(&mut bytes);
        bytes.push(b'\n');
        self.input.extend_from_slice(&bytes);
        perturbed
    }

    fn perturb_bytes(&mut self, bytes: &mut Vec<u8>) -> bool {
        let perturbed = self.rng.chance(2);
        if perturbed {
            *bytes = self.perturbed_token(bytes);
        }
        perturbed
    }

    /// A variant of `token` near a boundary of the input rules: a boundary
    /// number, an odd spelling, another kind of token, a control byte, a
    /// comment, truncation, or invalid UTF-8.
    fn perturbed_token(&mut self, token: &[u8]) -> Vec<u8> {
        const BOUNDARIES: [&str; 16] = [
            "9223372036854775807",
            "9223372036854775808",
            "-9223372036854775808",
            "-9223372036854775809",
            "2147483647",
            "-0",
            "+007",
            "1e309",
            "5e-324",
            "0e-9999",
            "2.2250738585072014e-308",
            "1.e2",
            "+.5",
            "nan",
            "0x10",
            "1_000",
        ];
        let mut bytes = token.to_vec();
        match self.rng.below(9) {
            0 | 1 => return self.rng.pick(&BOUNDARIES).as_bytes().to_vec(),
            2 => bytes.insert(0, b'+'),
            3 => return b"word".to_vec(),
            4 => bytes.push(*self.rng.pick(&[b'\r', 0x0b, 0x0c, 0])),
            5 => bytes.insert(0, b'#'),
            6 => {
                bytes.pop();
            }
            7 => {
                const INVALID: [&[u8]; 3] = [b"\xff", b"\xc3", b"\xed\xa0\x80"];
                let invalid: &[u8] = self.rng.pick::<&[u8]>(&INVALID);
                bytes.extend_from_slice(invalid);
            }
            _ => bytes.extend_from_slice(b"\r\n"),
        }
        bytes
    }
}
