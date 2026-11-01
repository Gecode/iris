use super::{C, K, PResult, Parser};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpressionForm {
    Name,
    Other,
}

impl Parser<'_> {
    pub(super) fn expression(&mut self) -> PResult<ExpressionForm> {
        self.binary(1)
    }

    fn binary(&mut self, minimum: u8) -> PResult<ExpressionForm> {
        self.trivia();
        let checkpoint = self.builder.checkpoint();
        let depth = self.expression_depth;
        let result = (|| {
            let mut form = self.unary()?;
            while let Some(level) = self.binary_precedence() {
                if level < minimum {
                    break;
                }
                self.enter_expression()?;
                self.builder.start_node_at(checkpoint, K::Binary.into());
                self.bump();
                let rhs = self.binary(level + 1);
                self.finish();
                rhs?;
                form = ExpressionForm::Other;
            }
            Ok(form)
        })();
        // Operator entries accumulate through this invocation, including chains.
        self.expression_depth = depth;
        result
    }
    fn binary_precedence(&self) -> Option<u8> {
        if self.kind() != Some(K::Punct) {
            return None;
        }
        match self.text() {
            "||" => Some(1),
            "&&" => Some(2),
            "==" | "!=" => Some(3),
            "<" | ">" | "<=" | ">=" => Some(4),
            "+" | "-" => Some(5),
            "*" | "/" | "%" => Some(6),
            _ => None,
        }
    }
    fn enter_expression(&mut self) -> PResult {
        if self.expression_depth == 256 {
            self.fail(C::Nesting, "expressions are nested too deeply")
        } else {
            self.expression_depth += 1;
            Ok(())
        }
    }
    fn unary(&mut self) -> PResult<ExpressionForm> {
        self.enter_expression()?;
        let result = if self.at("-") || self.at("!") {
            self.trivia();
            self.start(K::Unary);
            self.bump();
            let result = self.unary().map(|_| ExpressionForm::Other);
            self.finish();
            result
        } else {
            self.power()
        };
        self.expression_depth -= 1;
        result
    }
    fn power(&mut self) -> PResult<ExpressionForm> {
        self.trivia();
        let checkpoint = self.builder.checkpoint();
        let form = self.postfix()?;
        if !self.at("^") {
            return Ok(form);
        }
        self.builder.start_node_at(checkpoint, K::Power.into());
        self.bump();
        let result = self.unary();
        self.finish();
        result.map(|_| ExpressionForm::Other)
    }
    fn postfix(&mut self) -> PResult<ExpressionForm> {
        self.trivia();
        let checkpoint = self.builder.checkpoint();
        let depth = self.expression_depth;
        let result = (|| {
            let mut form = self.primary()?;
            while self.at("[") || self.at(".") {
                let field = self.at(".");
                self.builder.start_node_at(
                    checkpoint,
                    if field { K::Field } else { K::IndexExpression }.into(),
                );
                self.bump();
                let result = (|| {
                    self.enter_expression()?;
                    if field {
                        self.require_kind(K::Ident)
                    } else {
                        self.expression()?;
                        self.require("]")
                    }
                })();
                self.finish();
                result?;
                form = ExpressionForm::Other;
            }
            Ok(form)
        })();
        self.expression_depth = depth;
        result
    }
    fn primary(&mut self) -> PResult<ExpressionForm> {
        self.trivia();
        match self.kind() {
            Some(K::Integer | K::Float | K::String) => {
                self.start(K::Literal);
                self.bump();
                self.finish();
                Ok(ExpressionForm::Other)
            }
            Some(K::Punct) if self.at("(") => {
                self.start(K::Parenthesized);
                self.bump();
                let result = self.expression().and_then(|form| {
                    self.require(")")?;
                    Ok(form)
                });
                self.finish();
                result
            }
            Some(K::Ident) => self.name_expression(),
            _ => self.fail(C::Syntax, "expected an expression"),
        }
    }
    fn name_expression(&mut self) -> PResult<ExpressionForm> {
        let token = self.token().expect("identifier token").clone();
        let name = self.text().to_owned();
        if name.starts_with("gecode.") {
            self.start(K::HostConstant);
            self.bump();
            let result = if matches!(name.as_str(), "gecode.int_min" | "gecode.int_max") {
                Ok(ExpressionForm::Other)
            } else {
                self.diagnostic_at(
                    C::Constant,
                    format!("unknown Gecode constant '{name}'"),
                    &token,
                );
                Err(())
            };
            self.finish();
            return result;
        }
        let checkpoint = self.builder.checkpoint();
        self.bump();
        if !self.at("(") {
            self.builder.start_node_at(checkpoint, K::Name.into());
            self.finish();
            return Ok(ExpressionForm::Name);
        }
        self.bump();
        let comprehension = matches!(name.as_str(), "all" | "any" | "sum" | "count")
            && self.kind() == Some(K::Ident)
            && self.token_at(self.stream.significant(self.position() + 1), "in");
        self.builder.start_node_at(
            checkpoint,
            if comprehension {
                K::Comprehension
            } else {
                K::Call
            }
            .into(),
        );
        let result = if comprehension {
            self.comprehension()
        } else {
            self.arguments(&name, &token)
        };
        self.finish();
        result.map(|_| ExpressionForm::Other)
    }
    fn comprehension(&mut self) -> PResult {
        self.name(false)?;
        self.require("in")?;
        self.expression()?;
        self.require(":")?;
        self.expression()?;
        self.require(")")
    }
    fn arguments(&mut self, name: &str, callee: &crate::lexer::Token) -> PResult {
        let mut arguments = 0;
        let mut first_is_name = false;
        if !self.at(")") {
            loop {
                let form = self.expression()?;
                if arguments == 0 {
                    first_is_name = form == ExpressionForm::Name;
                }
                arguments += 1;
                if !self.eat(",") {
                    break;
                }
            }
        }
        self.require(")")?;
        let valid = match name {
            "defined" => arguments == 1 && first_is_name,
            "count" | "size" => arguments == 1,
            "min" | "max" | "sum" | "distinct" => arguments >= 1,
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            self.diagnostic_at(
                C::Function,
                format!("unknown function or wrong arguments for '{name}'"),
                callee,
            );
            Err(())
        }
    }
}
