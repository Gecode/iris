use super::*;

impl Interpreter<'_, '_> {
    pub(super) fn execute_param(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let name = self.spec.index.names[&tokens(node)[1].text_range()].clone();
        // Defaults execute even when overridden, and may themselves fail.
        let default = node
            .children()
            .find(|n| is_expression(n.kind()))
            .map(|n| self.eval(&n))
            .transpose()?;
        self.work(WorkKind::Probe, 1);
        let value = if let Some(text) = self.params.get(&name.id) {
            self.work(WorkKind::Leaf, text.len());
            let kind = default.as_ref().map(Value::value_type);
            let converted = match kind {
                Some("string") => Some(Value::string(self.input.control.copy_text(text)?)),
                Some("int") => {
                    numeric::parse_integer_controlled(text.as_bytes(), &self.input.control)?
                        .map(Value::int)
                }
                Some("float") => {
                    numeric::parse_float_controlled(text.as_bytes(), &self.input.control)?
                        .map(Value::float)
                }
                None => Some(
                    match numeric::parse_integer_controlled(text.as_bytes(), &self.input.control)? {
                        Some(n) => Value::int(n),
                        None => match numeric::parse_float_controlled(
                            text.as_bytes(),
                            &self.input.control,
                        )? {
                            Some(n) => Value::float(n),
                            None => Value::string(self.input.control.copy_text(text)?),
                        },
                    },
                ),
                _ => unreachable!("three value types"),
            };
            converted.ok_or_else(|| {
                self.error_text(
                    ErrorCode::ParamType,
                    &[
                        "parameter '",
                        &name,
                        "' must be ",
                        if kind == Some("int") {
                            "an integer"
                        } else {
                            "a number"
                        },
                        ", not '",
                        text,
                        "'",
                    ],
                )
            })?
        } else {
            default.ok_or_else(|| {
                self.error_text(
                    ErrorCode::ParamMissing,
                    &["parameter '", &name, "' must be given"],
                )
            })?
        };
        self.frames
            .last_mut()
            .expect("active frame")
            .set_controlled(name, value, &self.input.control)?;
        Ok(())
    }
}
