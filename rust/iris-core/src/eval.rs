use crate::{
    ErrorCode, Failure, Value, numeric,
    reader::{Interpreter, expression, tokens, truth, unsupported},
    value::LeafStorage,
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
                    K::String => Value::string(
                        self.spec
                            .index
                            .pattern_literals
                            .get(&token.text_range())
                            .cloned()
                            .unwrap_or_else(|| DecodedString::new(token.text()).text),
                    ),
                    _ => unreachable!("literal kind"),
                })
            }
            K::Name => {
                let own_tokens = tokens(node);
                self.require_value_name(&own_tokens[0])?;
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
            K::IndexExpression => self.eval_index(node),
            K::Comprehension => self.eval_comprehension(node),
            K::Field => Err(unsupported(
                node.text_range(),
                "record field evaluation",
                "rust-007",
            )),
            _ => unreachable!("expression kind"),
        }
    }
    fn require_value_name(&self, token: &iris_syntax::SyntaxToken) -> Result<(), Failure> {
        if let Some(reference) = self.spec.index.references.get(&token.text_range())
            && matches!(
                self.spec.analysis.model.reference(*reference).resolution,
                iris_sema::Resolution::Structural { .. }
            )
        {
            return Err(unsupported(
                token.text_range(),
                "record expression evaluation",
                "rust-007",
            ));
        }
        Ok(())
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
        if left.depth() > 0 || right.depth() > 0 {
            return Err(self
                .error(
                    ErrorCode::ComparisonType,
                    format!("'{operator}' compares single values, not arrays"),
                )
                .into());
        }
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
            if argument.kind() == K::Field {
                return Err(unsupported(
                    argument.text_range(),
                    "record field evaluation",
                    "rust-007",
                ));
            }
            let name = &tokens(&argument)[0];
            self.require_value_name(name)?;
            return Ok(Value::int(i64::from(self.lookup(name.text()).is_some())));
        }
        if matches!(name, "size" | "count") {
            let value = self.eval(&expression(node))?;
            let children = value.root().children().map_err(|_| {
                self.error(ErrorCode::FunctionType, format!("'{name}' needs an array"))
            })?;
            return Ok(Value::int(children.len() as i64));
        }
        let mut minimum: Option<i64> = None;
        let mut maximum: Option<i64> = None;
        let mut sum = 0i64;
        let mut seen = HashSet::new();
        let mut distinct = true;
        for argument in node.children() {
            let value = self.eval(&argument)?;
            if value.leaf_count() > 0 && value.value_type() != "int" {
                return Err(self
                    .error(
                        ErrorCode::FunctionType,
                        format!("'{name}' has an invalid argument type"),
                    )
                    .into());
            }
            self.charge_expression(value.leaf_count() as u128)?;
            let LeafStorage::Int(integers) = value.leaves else {
                continue;
            };
            for integer in integers {
                match name {
                    "min" => minimum = Some(minimum.map_or(integer, |v| v.min(integer))),
                    "max" => maximum = Some(maximum.map_or(integer, |v| v.max(integer))),
                    "sum" => {
                        sum = sum.checked_add(integer).ok_or_else(|| {
                            self.error(ErrorCode::IntegerOverflow, "integer overflow in 'sum'")
                        })?
                    }
                    "distinct" => distinct &= seen.insert(integer),
                    _ => unreachable!("validated aggregate function"),
                }
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
    fn eval_index(&mut self, node: &SyntaxNode) -> Result<Value, Failure> {
        let mut operands = node.children();
        let base = self.eval(&operands.next().expect("index base"))?;
        let length = base
            .root()
            .children()
            .map_err(|_| self.error(ErrorCode::IndexScalar, "cannot index a scalar"))?
            .len();
        let index = self.eval(&operands.next().expect("index operand"))?;
        let index = index
            .root()
            .as_int()
            .map_err(|_| self.error(ErrorCode::IndexType, "array index must be an integer"))?;
        if index < 0 || index as u64 >= length as u64 {
            return Err(self
                .error(ErrorCode::IndexRange, "array index out of range")
                .into());
        }
        let selected = base.select(&[index as usize]).expect("validated index");
        self.charge_expression(1 + selected.leaf_range().len() as u128)?;
        Ok(selected.to_owned())
    }

    fn eval_comprehension(&mut self, node: &SyntaxNode) -> Result<Value, Failure> {
        let parts = tokens(node);
        let function = parts[0].text();
        let binding = parts[2].text();
        let mut expressions = node.children();
        let domain = self.eval(&expressions.next().expect("comprehension domain"))?;
        let length = domain
            .root()
            .children()
            .map_err(|_| self.error(ErrorCode::FunctionType, "comprehension needs an array"))?
            .len();
        let body = expressions.next().expect("comprehension body");
        let mut result = i64::from(function == "all");
        for index in 0..length {
            let selected = domain.select(&[index]).expect("domain element");
            self.charge_expression(1 + selected.leaf_range().len() as u128)?;
            self.bindings
                .push((binding.to_owned(), selected.to_owned()));
            let value = self.eval(&body);
            self.bindings.pop();
            let value = value?;
            match function {
                "all" if !truth(&value) => return Ok(Value::int(0)),
                "any" if truth(&value) => return Ok(Value::int(1)),
                "all" | "any" => {}
                "count" => result += i64::from(truth(&value)),
                "sum" => {
                    let integer = value
                        .root()
                        .as_int()
                        .map_err(|_| self.error(ErrorCode::IntegerType, "'sum' needs integers"))?;
                    result = result.checked_add(integer).ok_or_else(|| {
                        self.error(ErrorCode::IntegerOverflow, "integer overflow in 'sum'")
                    })?;
                }
                _ => unreachable!("comprehension function"),
            }
        }
        Ok(Value::int(result))
    }
}
