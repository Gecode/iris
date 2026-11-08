//! Reading types and patterns.
use super::expr::{Class, Value};
use super::{Arg, Renderer};
use crate::program::{AlphabetOption, BaseType, Declared, Expr, ListEnd, Piece, Type};

fn base_type(ty: BaseType) -> &'static str {
    match ty {
        BaseType::Int => "int_",
        BaseType::Float => "float_",
        BaseType::Num => "num",
        BaseType::Word => "word",
        BaseType::Rest => "rest",
    }
}

/// What a reading's C++ type allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A `BaseType` or a `Range`: `dim`, `layout`, `offset` and `until`.
    Base,
    /// An `Alphabet`: `dim` and `until`.
    Alphabet,
    /// A `NamedType`, which has no methods.
    Named,
    /// A `TypeOf`, which has them all.
    TypeOf,
}

impl Kind {
    fn has(self, method: &str) -> bool {
        match self {
            Self::Base | Self::TypeOf => true,
            Self::Alphabet => matches!(method, "dim" | "until"),
            Self::Named => false,
        }
    }
}

/// A reading type with what its C++ type allows.
struct Reading {
    value: Value,
    kind: Kind,
}

/// `receiver.method(argument)`.
fn method_call(receiver: &Value, method: &str, argument: Value) -> Value {
    let callee = format!("{}.{method}", receiver.receiver());
    Value::call(&callee, vec![Arg::new(argument)], Class::Other)
}

impl Renderer<'_> {
    /// A reading type, as the argument of a builder call.
    pub(super) fn reading(&mut self, ty: &Type) -> Value {
        self.typed_reading(ty).value
    }

    fn typed_reading(&mut self, ty: &Type) -> Reading {
        match ty {
            Type::Base(ty) => Reading {
                value: Value::atom(base_type(*ty).to_owned(), Class::Other),
                kind: Kind::Base,
            },
            Type::Alphabet { symbols, options } => Reading {
                value: self.alphabet(symbols, options),
                kind: Kind::Alphabet,
            },
            Type::Named(name) => Reading {
                value: Value::name(self.type_reference(name), Class::Other),
                kind: Kind::Named,
            },
            Type::Range {
                lower,
                upper,
                exclusive,
            } => Reading {
                value: self.range(*lower, *upper, *exclusive),
                kind: Kind::Base,
            },
        }
    }

    /// `alphabet(symbols)` with its options chained in order.
    fn alphabet(&mut self, symbols: &str, options: &[AlphabetOption]) -> Value {
        let symbols = self.string_value(symbols);
        let mut value = self.combine(vec![symbols], |values| {
            Value::call_of("alphabet", values, Class::Other)
        });
        for option in options {
            let (method, argument) = match option {
                AlphabetOption::Blank(text) => ("blank", self.string_value(text)),
                AlphabetOption::Strip(text) => ("strip", self.string_value(text)),
                AlphabetOption::Base(base) => ("base", Value::atom(base.to_string(), Class::Int)),
            };
            value = self.combine(vec![value, argument], |values| {
                method_call(&values[0], method, values[1].clone())
            });
        }
        value
    }

    /// `Range{.from = lower, .to = upper}`, or `.below` for `..<`.
    fn range(&mut self, lower: Option<Expr>, upper: Option<Expr>, exclusive: bool) -> Value {
        let mut keywords = Vec::new();
        let mut operands = Vec::new();
        if let Some(lower) = lower {
            keywords.push(".from");
            operands.push(self.expr(lower));
        }
        if let Some(upper) = upper {
            keywords.push(if exclusive { ".below" } else { ".to" });
            operands.push(self.expr(upper));
        }
        self.combine(operands, |values| {
            let arguments = keywords
                .iter()
                .zip(values)
                .map(|(keyword, value)| Arg::keyword(keyword, value.integral()))
                .collect();
            Value::braced("Range", arguments, None, Class::Other)
        })
    }

    /// The type of a declaration, with its options.
    pub(super) fn declared(&mut self, declared: &Declared) -> Value {
        let mut reading = self.typed_reading(&declared.ty);
        for &size in &declared.dimensions {
            reading = self.option(reading, "dim", size, true);
        }
        if let Some(layout) = declared.layout {
            reading = self.option(reading, "layout", layout, false);
        }
        if let Some(offset) = declared.offset {
            reading = self.option(reading, "offset", offset, true);
        }
        reading.value
    }

    /// `reading.method(argument)`, through `ty` if the reading lacks the
    /// method, with an integer argument if `integral`.
    fn option(
        &mut self,
        reading: Reading,
        method: &str,
        argument: Expr,
        integral: bool,
    ) -> Reading {
        let argument = self.expr(argument);
        let argument = if integral {
            argument.integral()
        } else {
            argument
        };
        self.method(reading, method, argument)
    }

    fn method(&mut self, reading: Reading, method: &str, argument: Value) -> Reading {
        let receiver = if reading.kind.has(method) {
            reading.value
        } else {
            self.combine(vec![reading.value], |values| {
                Value::call_of("ty", values, Class::Other)
            })
        };
        let value = self.combine(vec![receiver, argument], |values| {
            method_call(&values[0], method, values[1].clone())
        });
        Reading {
            value,
            kind: Kind::TypeOf,
        }
    }

    /// The reading of a `list`.
    pub(super) fn list_reading(&mut self, declared: &Declared, end: &ListEnd) -> Value {
        let reading = self.typed_reading(&declared.ty);
        let end = match end {
            ListEnd::Eol => Value::atom("eol".to_owned(), Class::Other),
            ListEnd::Expr(end) => self.expr(*end),
        };
        let mut value = self.method(reading, "until", end).value;
        if let Some(offset) = declared.offset {
            let offset = self.expr(offset).integral();
            value = self.combine(vec![value, offset], |values| {
                method_call(&values[0], "offset", values[1].clone())
            });
        }
        value
    }

    /// `function(arguments)`, through temporaries if it is too wide.
    fn part(&mut self, function: &str, arguments: Vec<Value>) -> Value {
        self.combine(arguments, |values| {
            Value::call_of(function, values, Class::Other)
        })
    }

    fn piece(&mut self, piece: &Piece) -> Option<Value> {
        let atom = |text: &str| Value::atom(text.to_owned(), Class::Other);
        Some(match piece {
            Piece::Text(_) => return None,
            Piece::Bind { name, ty } => {
                let handle = self.handle(*name);
                let ty = match ty {
                    Some(ty) => self.reading(ty),
                    None => atom("untyped"),
                };
                // `bind` alone may also find the socket function `::bind`.
                self.part("iris::build::bind", vec![handle, ty])
            }
            Piece::Reference(name) => {
                let handle = self.handle(*name);
                self.part("ref", vec![handle])
            }
            Piece::Validate(ty) => {
                let ty = self.reading(ty);
                self.part("validate", vec![ty])
            }
            Piece::Blank => atom("blank"),
            Piece::Discard => atom("discard"),
            Piece::DiscardAs(ty) => {
                let ty = self.reading(ty);
                self.part("discard_as", vec![ty])
            }
            Piece::RestOfLine => atom("rest_of_line"),
        })
    }

    /// A pattern, as the argument of a builder call: a string literal, or
    /// `pattern` with a `{}` in its text for each part.
    pub(super) fn pattern(&mut self, pieces: &[Piece]) -> Value {
        let mut text = String::new();
        let mut parts = Vec::new();
        for piece in pieces {
            match piece {
                Piece::Text(piece) => text.push_str(piece),
                _ => {
                    text.push_str("{}");
                    parts.extend(self.piece(piece));
                }
            }
        }
        let text = self.string_value(&text);
        if parts.is_empty() {
            return text;
        }
        let mut arguments = vec![text];
        arguments.extend(parts);
        Value::call_of("pattern", &arguments, Class::Other)
    }
}
