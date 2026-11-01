use crate::{
    ErrorCode, Failure, Value, numeric,
    reader::{Interpreter, expression, tokens, truth, unsupported},
};
use iris_syntax::{DecodedString, SyntaxKind as K, SyntaxNode};
use std::{cmp::Ordering, collections::HashSet};

impl Interpreter<'_, '_> {
    pub fn eval(&mut self, node: &SyntaxNode) -> Result<Value, Failure> {
        match node.kind() {
            K::Literal => {
                let token = &tokens(node)[0];
                Ok(match token.kind() {
                    K::Integer => Value::int(
                        numeric::parse_integer(token.text().as_bytes())
                            .expect("validated integer literal"),
                    ),
                    K::Float => Value::float(
                        numeric::parse_float(token.text().as_bytes())
                            .expect("validated float literal"),
                    ),
                    K::String => Value::string(DecodedString::new(token.text()).text),
                    _ => unreachable!("literal kind"),
                })
            }
            K::Name => {
                let own_tokens = tokens(node);
                let name = own_tokens[0].text();
                self.lookup(name).cloned().ok_or_else(|| {
                    self.error(
                        ErrorCode::NameUndefined,
                        format!("'{name}' is not defined yet"),
                    )
                    .into()
                })
            }
            K::HostConstant => {
                let token = &tokens(node)[0];
                let reference = self.spec.index.references[&token.text_range()];
                let iris_sema::Resolution::HostConstant(value) =
                    self.spec.analysis.model.reference(reference).resolution
                else {
                    unreachable!("host constant has a resolved value");
                };
                Ok(Value::int(value))
            }
            K::Parenthesized => self.eval(&expression(node)),
            K::Unary => self.eval_unary(node),
            K::Binary | K::Power => self.eval_binary(node),
            K::Call => self.eval_call(node),
            K::IndexExpression | K::Comprehension => Err(unsupported(
                node.text_range(),
                "indexing and comprehensions",
                "rust-004",
            )),
            K::Field => Err(unsupported(
                node.text_range(),
                "record field evaluation",
                "rust-007",
            )),
            _ => unreachable!("expression kind"),
        }
    }
    fn eval_unary(&mut self, node: &SyntaxNode) -> Result<Value, Failure> {
        let value = self.eval(&expression(node))?;
        if tokens(node)[0].text() == "!" {
            return Ok(Value::int(i64::from(!truth(&value))));
        }
        if let Ok(integer) = value.root().as_int() {
            return integer.checked_neg().map(Value::int).ok_or_else(|| {
                self.error(ErrorCode::IntegerOverflow, "integer overflow in '-'")
                    .into()
            });
        }
        if let Ok(float) = value.root().as_float() {
            return Ok(Value::float(-float));
        }
        Err(self
            .error(ErrorCode::IntegerType, "'-' needs integers")
            .into())
    }
    fn eval_binary(&mut self, node: &SyntaxNode) -> Result<Value, Failure> {
        let mut operands = node.children();
        let left = self.eval(&operands.next().expect("binary left operand"))?;
        let own_tokens = tokens(node);
        let operator = own_tokens[0].text();
        if (operator == "&&" && !truth(&left)) || (operator == "||" && truth(&left)) {
            return Ok(Value::int(i64::from(truth(&left))));
        }
        let right = self.eval(&operands.next().expect("binary right operand"))?;
        if matches!(operator, "&&" | "||") {
            return Ok(Value::int(i64::from(truth(&right))));
        }
        if matches!(operator, "==" | "!=" | "<" | ">" | "<=" | ">=") {
            return self.compare(operator, &left, &right);
        }
        self.integer_arithmetic(operator, &left, &right)
    }
    fn integer_arithmetic(
        &self,
        operator: &str,
        left: &Value,
        right: &Value,
    ) -> Result<Value, Failure> {
        let a = left.root().as_int().map_err(|_| {
            self.error(
                ErrorCode::IntegerType,
                format!("'{operator}' needs integers"),
            )
        })?;
        let b = right.root().as_int().map_err(|_| {
            self.error(
                ErrorCode::IntegerType,
                format!("'{operator}' needs integers"),
            )
        })?;
        if matches!(operator, "/" | "%") && b == 0 {
            return Err(self
                .error(ErrorCode::DivisionZero, "division by zero")
                .into());
        }
        if operator == "^" && b < 0 {
            return Err(self
                .error(ErrorCode::Exponent, "negative exponent in '^'")
                .into());
        }
        let result = match operator {
            "+" => a.checked_add(b),
            "-" => a.checked_sub(b),
            "*" => a.checked_mul(b),
            "/" => a.checked_div(b),
            "%" => a.checked_rem(b),
            "^" => numeric::integer_power(a, b),
            _ => unreachable!("arithmetic operator"),
        };
        result.map(Value::int).ok_or_else(|| {
            self.error(
                ErrorCode::IntegerOverflow,
                format!("integer overflow in '{operator}'"),
            )
            .into()
        })
    }
    fn compare(&self, operator: &str, left: &Value, right: &Value) -> Result<Value, Failure> {
        let a = left.root();
        let b = right.root();
        let order = match (a.as_int(), b.as_int()) {
            (Ok(a), Ok(b)) => Some(a.cmp(&b)),
            _ if a.value_type() != "string" && b.value_type() != "string" => {
                let a = a
                    .as_float()
                    .unwrap_or_else(|_| a.as_int().expect("numeric scalar") as f64);
                let b = b
                    .as_float()
                    .unwrap_or_else(|_| b.as_int().expect("numeric scalar") as f64);
                a.partial_cmp(&b)
            }
            _ if matches!(operator, "==" | "!=") => {
                if a.as_str()
                    .ok()
                    .zip(b.as_str().ok())
                    .is_some_and(|(a, b)| a == b)
                {
                    Some(Ordering::Equal)
                } else {
                    None
                }
            }
            _ => {
                return Err(self
                    .error(
                        ErrorCode::ComparisonType,
                        format!("'{operator}' compares numbers"),
                    )
                    .into());
            }
        };
        let result = match operator {
            "==" => order == Some(Ordering::Equal),
            "!=" => order != Some(Ordering::Equal),
            "<" => order == Some(Ordering::Less),
            ">" => order == Some(Ordering::Greater),
            "<=" => matches!(order, Some(Ordering::Less | Ordering::Equal)),
            ">=" => matches!(order, Some(Ordering::Greater | Ordering::Equal)),
            _ => unreachable!("comparison operator"),
        };
        Ok(Value::int(i64::from(result)))
    }
    fn eval_call(&mut self, node: &SyntaxNode) -> Result<Value, Failure> {
        let own_tokens = tokens(node);
        let name = own_tokens[0].text();
        if name == "defined" {
            let mut argument = expression(node);
            while argument.kind() == K::Parenthesized {
                argument = expression(&argument);
            }
            return Ok(Value::int(i64::from(
                self.lookup(tokens(&argument)[0].text()).is_some(),
            )));
        }
        let mut minimum: Option<i64> = None;
        let mut maximum: Option<i64> = None;
        let mut sum = 0i64;
        let mut seen = HashSet::new();
        let mut distinct = true;
        for argument in node.children() {
            let value = self.eval(&argument)?;
            let integer = if matches!(name, "size" | "count") {
                None
            } else {
                value.root().as_int().ok()
            }
            .ok_or_else(|| {
                self.error(
                    ErrorCode::FunctionType,
                    format!("'{name}' has an invalid argument type"),
                )
            })?;
            // Scalar aggregate arguments cost one leaf, after type validation (§10.3).
            self.charge_expression(1)?;
            match name {
                "min" => minimum = Some(minimum.map_or(integer, |v| v.min(integer))),
                "max" => maximum = Some(maximum.map_or(integer, |v| v.max(integer))),
                "sum" => {
                    sum = sum.checked_add(integer).ok_or_else(|| {
                        self.error(ErrorCode::IntegerOverflow, "integer overflow in 'sum'")
                    })?
                }
                "distinct" => distinct &= seen.insert(integer),
                _ => unreachable!("validated ordinary function"),
            }
        }
        let result = match name {
            "min" => minimum,
            "max" => maximum,
            "sum" => Some(sum),
            "distinct" => Some(i64::from(distinct)),
            _ => unreachable!("size/count failed type check"),
        };
        result.map(Value::int).ok_or_else(|| {
            self.error(
                ErrorCode::FunctionEmpty,
                format!("'{name}' needs at least one value"),
            )
            .into()
        })
    }
}
