use super::*;

// Only integer lists need a scalar sentinel fast path. Other domains retain
// their ordinary Value conversions and numeric/string equality rules.
enum IntegerSentinel<'a> {
    Integer(i64),
    Generic(&'a Value),
}

impl Interpreter<'_, '_> {
    pub(super) fn evaluate_offset(&mut self, node: &SyntaxNode) -> Result<i64, Failure> {
        let Some(offset) = node.children().find(|n| n.kind() == K::Offset) else {
            return Ok(0);
        };
        self.eval(&expression(&offset))?
            .root()
            .as_int()
            .map_err(|_| self.error(ErrorCode::OffsetType, "offset must be an integer"))
    }

    pub(super) fn evaluate_dimensions(&mut self, node: &SyntaxNode) -> Result<Vec<usize>, Failure> {
        let mut dimensions = vec![];
        for dimension in node.children().filter(|n| n.kind() == K::Dimension) {
            let value = self.eval(&expression(&dimension))?;
            let size = value
                .root()
                .as_int()
                .map_err(|_| self.error(ErrorCode::Count, "array size must be an integer"))?;
            if size < 0 {
                return Err(
                    self.error(ErrorCode::Count, format!("array size is negative ({size})"))
                );
            }
            if size > i32::MAX as i64 {
                return Err(self.error(
                    ErrorCode::Count,
                    format!("array size is too large ({size})"),
                ));
            }
            dimensions.push(size as usize);
        }
        Ok(dimensions)
    }

    fn array_size(&self, dimensions: &[usize]) -> Result<usize, Failure> {
        let mut size = 1u64;
        for &dimension in dimensions {
            size *= dimension as u64;
            if size > i32::MAX as u64 {
                return Err(self.error(ErrorCode::SizeLimit, "array is too large"));
            }
        }
        Ok(size as usize)
    }

    fn charge_empty_rows(&mut self, dimensions: &[usize]) -> Result<(), Failure> {
        let mut rows = 1u128;
        for &dimension in dimensions.iter().take_while(|&&n| n != 0) {
            rows *= dimension as u128;
            self.charge_idle(rows)?;
        }
        Ok(())
    }

    fn append_scalar(&self, value: &mut Value, scalar: Value) -> Result<(), Failure> {
        value
            .append_controlled(scalar, &self.input.control)
            .map_err(|e| match e {
                crate::value::BuildError::Host(e) => e.into(),
                crate::value::BuildError::Type(code) => {
                    self.error(code, "incompatible value types")
                }
            })
    }

    fn apply_offset(&self, value: &mut Value, offset: i64) -> Result<(), Failure> {
        let chunk = if self.input.control.is_active() {
            crate::cancellation::VALUE_CHUNK
        } else {
            usize::MAX
        };
        match &mut value.leaves {
            LeafStorage::Int(_) if offset == 0 => {}
            LeafStorage::Int(values) => {
                for values in values.chunks_mut(chunk) {
                    self.input.control.check()?;
                    for integer in values {
                        *integer = integer.checked_add(offset).ok_or_else(|| {
                            self.error(
                                ErrorCode::IntegerOverflow,
                                "integer overflow when applying offset",
                            )
                        })?;
                    }
                }
            }
            LeafStorage::Float(values) => {
                for values in values.chunks_mut(chunk) {
                    self.input.control.check()?;
                    for float in values {
                        *float += offset as f64;
                    }
                }
            }
            LeafStorage::String(_) => {}
        }
        Ok(())
    }

    pub(super) fn read_array(
        &mut self,
        domain: &EvaluatedType,
        dimensions: &[usize],
        offset: i64,
    ) -> Result<Value, Failure> {
        let size = self.array_size(dimensions)?;
        let mut value = domain.empty_value();
        if size == 0 {
            self.charge_empty_rows(dimensions)?;
        }
        let chunk = if self.input.control.is_active() {
            crate::cancellation::VALUE_CHUNK
        } else {
            usize::MAX
        };
        for start in (0..size).step_by(chunk) {
            self.input.control.check()?;
            for _ in start..start.saturating_add(chunk).min(size) {
                let scalar = self.read_scalar(domain)?;
                self.append_scalar(&mut value, scalar)?;
            }
        }
        self.apply_offset(&mut value, offset)?;
        value.set_dimensions_controlled(dimensions, &self.input.control)?;
        Ok(value)
    }

    pub(super) fn read_layout(
        &mut self,
        domain: &EvaluatedType,
        dimensions: &[usize],
        node: &SyntaxNode,
        offset: i64,
    ) -> Result<Value, Failure> {
        let layout = self.eval(&expression(node))?;
        let name = layout
            .root()
            .as_str()
            .map_err(|_| self.error(ErrorCode::Layout, "matrix layout must be a string"))?;
        if name == "FULL_MATRIX" {
            return self.read_array(domain, dimensions, offset);
        }
        let n = dimensions[0];
        if n != dimensions[1] {
            return Err(self.error_text(
                ErrorCode::Layout,
                &["layout ", name, " needs a square matrix"],
            ));
        }
        // Symmetric column traversals have the same output order as these row traversals.
        let (upper, diagonal) = match name {
            "UPPER_ROW" | "LOWER_COL" => (true, false),
            "LOWER_ROW" | "UPPER_COL" => (false, false),
            "UPPER_DIAG_ROW" | "LOWER_DIAG_COL" => (true, true),
            "LOWER_DIAG_ROW" | "UPPER_DIAG_COL" => (false, true),
            _ => {
                return Err(
                    self.error_text(ErrorCode::Layout, &["unknown matrix layout '", name, "'"])
                );
            }
        };
        let size = self.array_size(dimensions)?;
        let cells = || {
            (0..n).flat_map(move |i| {
                let columns = if upper {
                    i + usize::from(!diagonal)..n
                } else {
                    0..i + usize::from(diagonal)
                };
                columns.map(move |j| (i, j))
            })
        };
        let mut triangle = domain.empty_value();
        for _ in cells() {
            self.input.control.check()?;
            let scalar = self.read_scalar(domain)?;
            self.append_scalar(&mut triangle, scalar)?;
        }
        self.apply_offset(&mut triangle, offset)?;
        let leaves = match triangle.leaves {
            LeafStorage::Int(values) => {
                let mut full = vec![0; size];
                for (cell, ((i, j), value)) in cells().zip(values).enumerate() {
                    if cell.is_multiple_of(crate::cancellation::VALUE_CHUNK) {
                        self.input.control.check()?;
                    }
                    full[i * n + j] = value;
                    full[j * n + i] = value;
                }
                LeafStorage::Int(full)
            }
            LeafStorage::Float(values) => {
                let mut full = vec![0.0; size];
                for (cell, ((i, j), value)) in cells().zip(values).enumerate() {
                    if cell.is_multiple_of(crate::cancellation::VALUE_CHUNK) {
                        self.input.control.check()?;
                    }
                    full[i * n + j] = value;
                    full[j * n + i] = value;
                }
                LeafStorage::Float(full)
            }
            LeafStorage::String(_) => unreachable!("layout requires a numeric reading type"),
        };
        let mut value = Value {
            leaves,
            levels: vec![],
        };
        value.set_dimensions_controlled(dimensions, &self.input.control)?;
        Ok(value)
    }

    pub(super) fn read_alphabet(
        &mut self,
        alphabet: &Alphabet,
        dimensions: &[usize],
    ) -> Result<Value, Failure> {
        let size = self.array_size(dimensions)?;
        if dimensions[1] == 0 {
            self.charge_idle(dimensions[0] as u128)?;
        }
        let mut leaves = vec![];
        for cell in 0..size {
            let byte = loop {
                self.input.skip_to_data()?;
                match self.input.peek_byte() {
                    Some(byte)
                        if self
                            .input
                            .control
                            .find_byte(&alphabet.strip, byte)?
                            .is_some() =>
                    {
                        self.input.consume_byte()?
                    }
                    byte => break byte,
                }
            };
            let symbol_error = || {
                self.error(
                    ErrorCode::Symbol,
                    format!(
                        "expected a symbol at row {}, column {}",
                        cell / dimensions[1] + 1,
                        cell % dimensions[1] + 1
                    ),
                )
            };
            let byte = byte.ok_or_else(symbol_error)?;
            let integer = if self
                .input
                .control
                .find_byte(&alphabet.blank, byte)?
                .is_some()
            {
                0
            } else if let Some(index) = self.input.control.find_byte(&alphabet.symbols, byte)? {
                alphabet.base.checked_add(index as i64).ok_or_else(|| {
                    self.error(
                        ErrorCode::IntegerOverflow,
                        "integer overflow in alphabet value",
                    )
                })?
            } else {
                return Err(symbol_error());
            };
            self.input.consume_byte()?;
            leaves.push(integer);
        }
        let mut value = Value {
            leaves: LeafStorage::Int(leaves),
            levels: vec![],
        };
        value.set_dimensions_controlled(dimensions, &self.input.control)?;
        Ok(value)
    }

    pub(super) fn execute_list(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let list = &self.spec.index.lists[self.spec.index.list_ids[&node.text_range()].0];
        self.execute_list_descriptor(list, None)
    }

    pub(super) fn execute_list_descriptor(
        &mut self,
        list: &prepared::List,
        mut bindings: Option<&mut prepared::ListBindings>,
    ) -> Result<(), Failure> {
        let name = list.name.clone();
        self.context = Some(name.clone());
        let domain = self.evaluate_prepared_domain(&list.domain, bindings.as_deref_mut())?;
        let mut value = if let Some(sentinel) = &list.sentinel {
            match self.eval_integer(sentinel, bindings.as_deref_mut().map(|b| &mut b.sentinel))? {
                Some(integer)
                    if matches!(domain, EvaluatedType::Int | EvaluatedType::Range { .. }) =>
                {
                    self.read_integer_list(
                        &domain,
                        IntegerSentinel::Integer(integer),
                        name.id,
                        bindings.as_deref().and_then(|b| b.output),
                    )?
                }
                integer => {
                    let sentinel = match integer {
                        Some(integer) => Value::int(integer),
                        None => self.eval(&sentinel.node)?,
                    };
                    let sentinel = self.list_sentinel(&domain, sentinel)?;
                    self.read_sentinel_list(
                        &domain,
                        &sentinel,
                        name.id,
                        bindings.as_deref().and_then(|b| b.output),
                    )?
                }
            }
        } else {
            self.read_eol_list(&domain)?
        };
        let offset =
            if let Some(offset) = &list.offset {
                match self.eval_integer(offset, bindings.as_deref_mut().map(|b| &mut b.offset))? {
                    Some(integer) => integer,
                    None => self.eval(&offset.node)?.root().as_int().map_err(|_| {
                        self.error(ErrorCode::OffsetType, "offset must be an integer")
                    })?,
                }
            } else {
                0
            };
        self.apply_offset(&mut value, offset)?;
        value.set_list_dimension(&self.input.control)?;
        self.publish(name, value, bindings.map(|bindings| &mut bindings.output))?;
        Ok(())
    }

    fn list_sentinel(&self, domain: &EvaluatedType, sentinel: Value) -> Result<Value, Failure> {
        if sentinel.depth() > 0 {
            return Err(self.error(
                ErrorCode::ListTerminator,
                "list terminator must be a scalar",
            ));
        }
        if matches!(domain, EvaluatedType::Word) {
            if let Ok(integer) = sentinel.root().as_int() {
                return Ok(Value::string(integer.to_string()));
            }
            if sentinel.value_type() != "string" {
                return Err(self.error(
                    ErrorCode::ListTerminator,
                    "word list terminator must be an integer or string",
                ));
            }
        }
        Ok(sentinel)
    }

    fn read_sentinel_list(
        &mut self,
        domain: &EvaluatedType,
        sentinel: &Value,
        name: NameId,
        output: Option<crate::value::EntryId>,
    ) -> Result<Value, Failure> {
        if matches!(domain, EvaluatedType::Int | EvaluatedType::Range { .. }) {
            return self.read_integer_list(
                domain,
                IntegerSentinel::Generic(sentinel),
                name,
                output,
            );
        }
        let mut value = domain.empty_value();
        loop {
            let string = sentinel.root().as_str().ok();
            let token = self.input.next_token()?.ok_or_else(|| {
                self.error(
                    if string.is_some() {
                        ErrorCode::ExpectedToken
                    } else {
                        ErrorCode::ExpectedValue
                    },
                    "expected list terminator, found end of input",
                )
            })?;
            if let Some(string) = string
                && self
                    .input
                    .control
                    .equal_bytes(string.as_bytes(), token.bytes)?
            {
                break;
            }
            let scalar = self.convert_scalar(domain, &token)?;
            if string.is_none() && numeric_equal(&scalar, sentinel) {
                break;
            }
            self.validate_range(domain, &scalar, &token)?;
            self.append_scalar(&mut value, scalar)?;
        }
        Ok(value)
    }

    fn read_integer_list(
        &mut self,
        domain: &EvaluatedType,
        sentinel: IntegerSentinel<'_>,
        name: NameId,
        output: Option<crate::value::EntryId>,
    ) -> Result<Value, Failure> {
        let (string, integer, float) = match sentinel {
            IntegerSentinel::Integer(integer) => (None, Some(integer), None),
            IntegerSentinel::Generic(sentinel) => (
                sentinel.root().as_str().ok(),
                sentinel.root().as_int().ok(),
                sentinel.root().as_float().ok(),
            ),
        };
        self.work(WorkKind::Probe, 1);
        let frame = self.frames.last_mut().expect("active frame");
        let mut value = match output {
            Some(entry) => frame.take_buffer_at(entry),
            None => frame.take_buffer(name),
        }
        .filter(|value| value.value_type() == "int")
        .unwrap_or_else(|| Value::empty("int"));
        let LeafStorage::Int(leaves) = &mut value.leaves else {
            unreachable!("integer buffer")
        };
        leaves.clear();
        loop {
            let token = self.input.next_token()?.ok_or_else(|| {
                self.error(
                    if string.is_some() {
                        ErrorCode::ExpectedToken
                    } else {
                        ErrorCode::ExpectedValue
                    },
                    "expected list terminator, found end of input",
                )
            })?;
            self.work(WorkKind::Leaf, token.bytes.len() + 1);
            if let Some(string) = string
                && self
                    .input
                    .control
                    .equal_bytes(string.as_bytes(), token.bytes)?
            {
                break;
            }
            let Some(value) = numeric::parse_integer_controlled(token.bytes, &self.input.control)?
            else {
                return Err(self.expected_value(domain, &token)?.into());
            };
            if integer == Some(value) || float.is_some_and(|sentinel| value as f64 == sentinel) {
                break;
            }
            if let EvaluatedType::Range { lower, upper } = domain
                && (lower.is_some_and(|lo| value < lo) || upper.is_some_and(|hi| value > hi))
            {
                return Err(self.expected_value(domain, &token)?.into());
            }
            leaves.push(value);
        }
        Ok(value)
    }

    fn read_eol_list(&mut self, domain: &EvaluatedType) -> Result<Value, Failure> {
        if !self.input.skip_to_line_data()? {
            self.input.skip_to_data()?;
        }
        if self.input.peek_byte().is_none() {
            return Err(self.error(
                ErrorCode::ExpectedValue,
                "expected a value, found end of input",
            ));
        }
        let mut value = domain.empty_value();
        loop {
            let scalar = self.read_scalar(domain)?;
            self.append_scalar(&mut value, scalar)?;
            if !self.input.skip_to_line_data()? {
                break;
            }
        }
        Ok(value)
    }
}

fn numeric_equal(left: &Value, right: &Value) -> bool {
    if let (Ok(a), Ok(b)) = (left.root().as_int(), right.root().as_int()) {
        return a == b;
    }
    let number = |v: &Value| {
        v.root()
            .as_float()
            .ok()
            .or_else(|| v.root().as_int().ok().map(|n| n as f64))
    };
    number(left).zip(number(right)).is_some_and(|(a, b)| a == b)
}
