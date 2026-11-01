use super::*;

impl Interpreter<'_, '_> {
    pub(super) fn execute_param(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let name = tokens(node)[1].text().to_owned();
        // Defaults execute even when overridden, and may themselves fail.
        let default = node
            .children()
            .find(|n| is_expression(n.kind()))
            .map(|n| self.eval(&n))
            .transpose()?;
        self.work(WorkKind::Probe, 1);
        let value = if let Some(text) = self.params.get(&name) {
            self.work(WorkKind::Leaf, text.len());
            let kind = default.as_ref().map(Value::value_type);
            let converted = match kind {
                Some("string") => Some(Value::string(text.into())),
                Some("int") => numeric::parse_integer(text.as_bytes()).map(Value::int),
                Some("float") => numeric::parse_float(text.as_bytes()).map(Value::float),
                None => Some(
                    numeric::parse_integer(text.as_bytes())
                        .map(Value::int)
                        .or_else(|| numeric::parse_float(text.as_bytes()).map(Value::float))
                        .unwrap_or_else(|| Value::string(text.into())),
                ),
                _ => unreachable!("three value types"),
            };
            converted.ok_or_else(|| {
                self.error(
                    ErrorCode::ParamType,
                    format!(
                        "parameter '{name}' must be {}, not '{text}'",
                        if kind == Some("int") {
                            "an integer"
                        } else {
                            "a number"
                        }
                    ),
                )
            })?
        } else {
            default.ok_or_else(|| {
                self.error(
                    ErrorCode::ParamMissing,
                    format!("parameter '{name}' must be given"),
                )
            })?
        };
        self.frames
            .last_mut()
            .expect("active frame")
            .set(name, value);
        Ok(())
    }
}
