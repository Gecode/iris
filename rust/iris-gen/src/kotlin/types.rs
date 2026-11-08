//! Reading types and patterns.
use super::Renderer;
use super::expr::{Class, Value, int_literal};
use crate::program::{AlphabetOption, BaseType, Declared, Expr, ListEnd, Piece, Type};

fn base_type(ty: BaseType) -> &'static str {
    match ty {
        BaseType::Int => "int()",
        BaseType::Float => "float()",
        BaseType::Num => "num()",
        BaseType::Word => "word()",
        BaseType::Rest => "rest()",
    }
}

/// The Kotlin class of a reading type: what `defineType` gives back for it.
fn base_class(ty: BaseType) -> &'static str {
    match ty {
        BaseType::Int => "IntType",
        BaseType::Float => "FloatType",
        BaseType::Num => "NumType",
        BaseType::Word | BaseType::Rest => "StringType",
    }
}

impl Renderer<'_> {
    /// The Kotlin class of `ty`.
    pub(super) fn type_class(&self, ty: &Type) -> &'static str {
        match ty {
            Type::Base(ty) => base_class(*ty),
            Type::Alphabet { .. } | Type::Range { .. } => "IntType",
            Type::Named(name) => {
                self.visible_types
                    .iter()
                    .rev()
                    .find(|visible| visible.name == *name)
                    .expect("a type that a type uses is visible")
                    .kotlin_type
            }
        }
    }

    /// A reading type, as the argument of a builder call.
    pub(super) fn reading(&mut self, ty: &Type) -> Value {
        match ty {
            Type::Base(ty) => Value::new(base_type(*ty).to_owned(), Class::Other),
            Type::Alphabet { symbols, options } => self.alphabet(symbols, options),
            Type::Named(name) => Value::name(self.type_reference(name), Class::Other),
            Type::Range {
                lower,
                upper,
                exclusive,
            } => self.range(*lower, *upper, *exclusive),
        }
    }

    /// `alphabet(symbols)` with its options chained in order.
    fn alphabet(&mut self, symbols: &str, options: &[AlphabetOption]) -> Value {
        let symbols = Value::string(symbols);
        let mut value = self.combine(vec![symbols], |values| {
            Value::call("alphabet", values.to_vec(), Class::Other)
        });
        for option in options {
            let (method, argument) = match option {
                AlphabetOption::Blank(text) => ("blank", Value::string(text)),
                AlphabetOption::Strip(text) => ("strip", Value::string(text)),
                AlphabetOption::Base(base) => ("base", Value::new(int_literal(*base), Class::Int)),
            };
            value = self.combine(vec![value, argument], |values| {
                values[0].method(method, &[values[1].clone()], Class::Other)
            });
        }
        value
    }

    /// `atLeast(lower)`, `atMost(upper)`, `below(upper)`, or `lower..upper`
    /// and `range(lower).below(upper)` with both bounds.
    fn range(&mut self, lower: Option<Expr>, upper: Option<Expr>, exclusive: bool) -> Value {
        match (lower, upper) {
            (Some(lower), Some(upper)) => {
                let operands = vec![self.expr(lower), self.expr(upper)];
                self.combine(operands, |values| {
                    Value::range(&values[0].bound(), &values[1].bound(), exclusive)
                })
            }
            (Some(lower), None) => {
                let lower = self.expr(lower);
                self.part("atLeast", vec![lower.bound()])
            }
            (None, Some(upper)) => {
                let upper = self.expr(upper);
                let function = if exclusive { "below" } else { "atMost" };
                self.part(function, vec![upper.bound()])
            }
            // The parser gives every range of an accepted spec a bound.
            (None, None) => Value::new("int()".to_owned(), Class::Other),
        }
    }

    /// The type of a declaration, with its options.
    pub(super) fn declared(&mut self, declared: &Declared) -> Value {
        let mut reading = self.reading(&declared.ty);
        for &size in &declared.dimensions {
            reading = self.option(reading, "dim", size);
        }
        if let Some(layout) = declared.layout {
            reading = self.option(reading, "layout", layout);
        }
        if let Some(offset) = declared.offset {
            reading = self.option(reading, "offset", offset);
        }
        reading
    }

    /// `reading.method(argument)`, with an `Expr` or `Long` argument.
    fn option(&mut self, reading: Value, method: &str, argument: Expr) -> Value {
        let argument = self.expr(argument).long_or_expr();
        self.combine(vec![reading, argument], |values| {
            values[0].method(method, &[values[1].clone()], Class::Other)
        })
    }

    /// The reading of a `list`.
    pub(super) fn list_reading(&mut self, declared: &Declared, end: &ListEnd) -> Value {
        let reading = self.reading(&declared.ty);
        let mut value = match end {
            ListEnd::Eol => self.combine(vec![reading], |values| {
                values[0].method("untilEol", &[], Class::Other)
            }),
            ListEnd::Expr(end) => {
                let end = self.expr(*end);
                // A string literal is the `String` overload's argument.
                let end = if end.class == Class::Str {
                    end
                } else {
                    end.long_or_expr()
                };
                self.combine(vec![reading, end], |values| {
                    values[0].method("until", &[values[1].clone()], Class::Other)
                })
            }
        };
        if let Some(offset) = declared.offset {
            value = self.option(value, "offset", offset);
        }
        value
    }

    /// `function(arguments)`, through temporaries if it is too wide.
    fn part(&mut self, function: &str, arguments: Vec<Value>) -> Value {
        self.combine(arguments, |values| {
            Value::call(function, values.to_vec(), Class::Other)
        })
    }

    fn piece(&mut self, piece: &Piece) -> Value {
        let call = |text: &str| Value::new(format!("{text}()"), Class::Other);
        match piece {
            Piece::Text(text) => Value::string(text),
            Piece::Bind { name, ty: None } => self.handle(*name),
            Piece::Bind { name, ty: Some(ty) } => {
                let handle = self.handle(*name);
                let ty = self.reading(ty);
                self.part("capture", vec![handle, ty])
            }
            Piece::Reference(name) => {
                let handle = self.handle(*name);
                self.part("ref", vec![handle])
            }
            Piece::Validate(ty) => {
                let ty = self.reading(ty);
                self.part("validate", vec![ty])
            }
            Piece::Blank => call("blank"),
            Piece::Discard => call("discard"),
            Piece::DiscardAs(ty) => {
                let ty = self.reading(ty);
                self.part("discard", vec![ty])
            }
            Piece::RestOfLine => call("restOfLine"),
        }
    }

    /// The parts of a pattern, as the arguments of `line`, `find` and `arm`.
    pub(super) fn pattern_parts(&mut self, pieces: &[Piece]) -> Vec<Value> {
        pieces.iter().map(|piece| self.piece(piece)).collect()
    }
}
