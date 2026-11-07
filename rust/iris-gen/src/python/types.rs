//! Reading types and patterns.
use super::expr::{Class, Value};
use super::{Arg, Renderer, quoted, string};
use crate::program::{AlphabetOption, BaseType, Declared, Expr, ListEnd, Piece, Type};

fn base_type(ty: BaseType) -> &'static str {
    match ty {
        BaseType::Int => "ib.int_",
        BaseType::Float => "ib.float_",
        BaseType::Num => "ib.num",
        BaseType::Word => "ib.word",
        BaseType::Rest => "ib.rest",
    }
}

/// The keyword or method of an alphabet option, its argument, and its place
/// among the keywords of `ib.alphabet`.
fn alphabet_option(option: &AlphabetOption) -> (&'static str, Value, usize) {
    match option {
        AlphabetOption::Blank(text) => ("blank", string(text), 0),
        AlphabetOption::Strip(text) => ("strip", string(text), 1),
        AlphabetOption::Base(value) => ("base", Value::atom(value.to_string(), Class::Int), 2),
    }
}

/// `receiver.method(argument)`.
fn method_call(receiver: &Value, method: &str, argument: Value) -> Value {
    let callee = format!("{}.{method}", receiver.receiver());
    Value::call(&callee, vec![Arg::new(argument)], Class::Other)
}

impl Renderer<'_> {
    /// A reading type, as the argument of a builder call.
    pub(super) fn reading(&mut self, ty: &Type) -> Value {
        match ty {
            Type::Base(ty) => Value::atom(base_type(*ty).to_owned(), Class::Other),
            Type::Alphabet { symbols, options } => self.alphabet(symbols, options),
            Type::Named(name) => Value::name(self.type_reference(name), Class::Other),
            Type::Range {
                lower,
                upper,
                exclusive,
            } => self.range(*lower, *upper, *exclusive),
        }
    }

    /// `ib.alphabet` takes its options as keywords in one order; any other
    /// order chains them.
    fn alphabet(&mut self, symbols: &str, options: &[AlphabetOption]) -> Value {
        let options: Vec<_> = options.iter().map(alphabet_option).collect();
        let in_order = options.windows(2).all(|pair| pair[0].2 < pair[1].2);
        let symbols = Arg::new(Value::atom(quoted(symbols), Class::Str));
        if in_order {
            let mut arguments = vec![symbols];
            for (keyword, value, _) in options {
                arguments.push(Arg::keyword(keyword, value));
            }
            return Value::call("ib.alphabet", arguments, Class::Other);
        }
        let mut value = Value::call("ib.alphabet", vec![symbols], Class::Other);
        for (method, argument, _) in options {
            value = self.combine(vec![value, argument], |values| {
                method_call(&values[0], method, values[1].clone())
            });
        }
        value
    }

    fn range(&mut self, lower: Option<Expr>, upper: Option<Expr>, exclusive: bool) -> Value {
        let lower = lower.map(|lower| self.expr(lower));
        let upper = upper.map(|upper| self.expr(upper));
        let upper_keyword = if exclusive { "below" } else { "hi" };
        match (lower, upper) {
            (Some(lower), Some(upper)) => self.combine(vec![lower, upper], |values| {
                let upper = values[1].integral();
                let upper = if exclusive {
                    Arg::keyword("below", upper)
                } else {
                    Arg::new(upper)
                };
                let arguments = vec![Arg::new(values[0].integral()), upper];
                Value::call("ib.int_range", arguments, Class::Other)
            }),
            (Some(lower), None) => self.combine(vec![lower], |values| {
                Value::call_of("ib.at_least", &[values[0].integral()], Class::Other)
            }),
            (None, Some(upper)) => self.combine(vec![upper], |values| {
                let arguments = vec![Arg::keyword(upper_keyword, values[0].integral())];
                Value::call("ib.int_range", arguments, Class::Other)
            }),
            (None, None) => unreachable!("the parser needs a bound in a range"),
        }
    }

    /// The type of a declaration, with its options.
    pub(super) fn declared(&mut self, declared: &Declared) -> Value {
        let mut value = self.reading(&declared.ty);
        for &size in &declared.dimensions {
            value = self.option(value, "dim", size, true);
        }
        if let Some(layout) = declared.layout {
            value = self.option(value, "layout", layout, false);
        }
        if let Some(offset) = declared.offset {
            value = self.option(value, "offset", offset, true);
        }
        value
    }

    /// `reading.method(argument)`, with an integer argument if `integral`.
    fn option(&mut self, reading: Value, method: &str, argument: Expr, integral: bool) -> Value {
        let argument = self.expr(argument);
        self.combine(vec![reading, argument], |values| {
            let argument = if integral {
                values[1].integral()
            } else {
                values[1].clone()
            };
            method_call(&values[0], method, argument)
        })
    }

    /// The reading of a `list`.
    pub(super) fn list_reading(&mut self, declared: &Declared, end: &ListEnd) -> Value {
        let reading = self.reading(&declared.ty);
        let mut value = match end {
            ListEnd::Eol => {
                let eol = Value::atom("ib.eol".to_owned(), Class::Other);
                self.combine(vec![reading, eol], |values| {
                    method_call(&values[0], "until", values[1].clone())
                })
            }
            ListEnd::Expr(end) => self.option(reading, "until", *end, false),
        };
        if let Some(offset) = declared.offset {
            value = self.option(value, "offset", offset, true);
        }
        value
    }

    /// `function(arguments)`, through temporaries if it is too wide.
    fn part(&mut self, function: &str, arguments: Vec<Value>) -> Value {
        self.combine(arguments, |values| {
            Value::call_of(function, values, Class::Other)
        })
    }

    fn piece(&mut self, piece: &Piece) -> Value {
        let atom = |text: &str| Value::atom(text.to_owned(), Class::Other);
        match piece {
            Piece::Text(text) => string(text),
            Piece::Bind { name, ty } => {
                let mut arguments = vec![self.handle(*name)];
                if let Some(ty) = ty {
                    arguments.push(self.reading(ty));
                }
                self.part("ib.bind", arguments)
            }
            Piece::Reference(name) => {
                let handle = self.handle(*name);
                self.part("ib.ref", vec![handle])
            }
            Piece::Validate(ty) => {
                let ty = self.reading(ty);
                self.part("ib.validate", vec![ty])
            }
            Piece::Blank => atom("ib.blank()"),
            Piece::Discard => atom("ib.discard()"),
            Piece::DiscardAs(ty) => {
                let ty = self.reading(ty);
                self.part("ib.discard_as", vec![ty])
            }
            Piece::RestOfLine => atom("ib.rest_of_line"),
        }
    }

    /// A pattern, as the argument of a builder call: a string, or a tuple.
    pub(super) fn pattern(&mut self, pieces: &[Piece]) -> Value {
        match pieces {
            [] => string(""),
            [Piece::Text(text)] => string(text),
            _ => {
                let items = pieces.iter().map(|piece| self.piece(piece)).collect();
                Value::tuple(items)
            }
        }
    }
}
