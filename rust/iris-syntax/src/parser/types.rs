use super::{C, Check, K, PResult, Parser, expressions::ExpressionForm};

impl Parser<'_> {
    pub(super) fn read_type(&mut self) -> PResult {
        self.trivia();
        let target = self.raw_position();
        self.start(K::Type);
        let result = self.type_contents();
        self.finish();
        if result.is_ok() {
            self.checkpoint(Check::TypeUse, target);
        }
        result
    }
    fn type_contents(&mut self) -> PResult {
        if self.kind() == Some(K::Ident) {
            if matches!(self.text(), "int" | "float" | "num" | "word" | "rest") {
                self.start(K::BaseType);
                self.bump();
                self.finish();
                return Ok(());
            }
            if self.at("alphabet") {
                return self.alphabet();
            }
            if self.type_names.iter().any(|name| name == self.text()) {
                self.start(K::TypeName);
                self.bump();
                self.finish();
                return Ok(());
            }
        }
        let checkpoint = self.builder.checkpoint();
        let lower = if self.at("..") || self.at("..<") {
            None
        } else {
            Some(self.expression()?)
        };
        if !self.at("..") && !self.at("..<") {
            // Unknown bare names, including parenthesized names, belong to sema.
            if lower == Some(ExpressionForm::Name) {
                self.builder.start_node_at(checkpoint, K::TypeName.into());
                self.finish();
                return Ok(());
            }
            return self.fail(
                C::Syntax,
                "expected '..' or '..<' after the lower bound of a range",
            );
        }
        self.builder.start_node_at(checkpoint, K::Range.into());
        let exclusive = self.at("..<");
        self.bump();
        let result = if self.upper_bound_ahead() {
            self.expression().map(|_| ())
        } else if exclusive || lower.is_none() {
            self.fail(
                C::Syntax,
                if exclusive {
                    "'..<' needs an upper bound"
                } else {
                    "a range needs a lower or an upper bound"
                },
            )
        } else {
            Ok(())
        };
        self.finish();
        result
    }
    fn upper_bound_ahead(&self) -> bool {
        match self.kind() {
            Some(K::Integer | K::Float | K::String) => true,
            Some(K::Punct) => matches!(self.text(), "(" | "-" | "!"),
            Some(K::Ident) => {
                !self.declaration_ahead()
                    && !matches!(
                        self.text(),
                        "comment"
                            | "stop"
                            | "expect"
                            | "tokens"
                            | "separators"
                            | "ignore"
                            | "param"
                            | "list"
                            | "line"
                            | "find"
                            | "skip"
                            | "index"
                            | "repeat"
                            | "each"
                            | "instances"
                            | "sections"
                            | "if"
                            | "let"
                            | "check"
                            | "type"
                            | "layout"
                            | "offset"
                            | "until"
                    )
            }
            _ => false,
        }
    }
    fn alphabet(&mut self) -> PResult {
        self.start(K::Alphabet);
        self.bump();
        let result = (|| {
            self.string("an alphabet")?;
            while ["blank", "strip", "base"]
                .iter()
                .any(|word| self.option(word))
            {
                self.start(K::AlphabetOption);
                let base = self.at("base");
                let description = if self.at("blank") {
                    "blank symbols"
                } else {
                    "symbols to strip"
                };
                self.bump();
                let result = if base {
                    self.eat("-");
                    self.require_kind(K::Integer)
                } else {
                    self.string(description)
                };
                self.finish();
                result?;
            }
            Ok(())
        })();
        self.finish();
        result
    }
}
