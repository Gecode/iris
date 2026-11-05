//! Reading types and patterns.
use super::expr::{Class, Precedence, Value};
use super::{CALL_WIDTH, Renderer, quoted};
use crate::program::{AlphabetOption, BaseType, Declared, Expr, ListEnd, NameId, Piece, Type};

fn base_type(ty: BaseType) -> &'static str {
    match ty {
        BaseType::Int => "int()",
        BaseType::Float => "float()",
        BaseType::Num => "num()",
        BaseType::Word => "word()",
        BaseType::Rest => "rest()",
    }
}

/// The method of an alphabet option and its argument.
fn alphabet_option(option: &AlphabetOption) -> (&'static str, Value) {
    match option {
        AlphabetOption::Blank(text) => ("blank", Value::new(quoted(text), Class::Str)),
        AlphabetOption::Strip(text) => ("strip", Value::new(quoted(text), Class::Str)),
        AlphabetOption::Base(value) => ("base", Value::new(value.to_string(), Class::Int)),
    }
}

/// A literal's integer value.
fn integer(value: &Value) -> Option<i64> {
    if !value.literal {
        return None;
    }
    value.text.trim_end_matches("_i64").parse().ok()
}

/// The bounds of a range, which must have one Rust type. Bare bounds that make
/// an empty Rust range are `clippy::reversed_empty_ranges`.
fn matched_bounds(lower: &Value, upper: &Value, exclusive: bool) -> (String, String) {
    let integers = lower.class == Class::Int && upper.class == Class::Int;
    let ordered = match (integer(lower), integer(upper)) {
        (Some(lower), Some(upper)) if exclusive => lower < upper,
        (Some(lower), Some(upper)) => lower <= upper,
        _ => false,
    };
    if ordered {
        (lower.text.clone(), upper.text.clone())
    } else if integers {
        (lower.receiver(), upper.receiver())
    } else {
        (lower.dynamic().text, upper.dynamic().text)
    }
}

/// A single bound, which may be an integer, an `Expr<Int>` or an `Expr<Dyn>`.
fn single_bound(bound: &Value) -> String {
    if bound.literal && bound.class == Class::Int {
        bound.text.clone()
    } else if bound.class == Class::Int {
        bound.bound(Precedence::Unary)
    } else {
        bound.dynamic().text
    }
}

impl Renderer<'_> {
    /// A reading type, as the argument of a builder call.
    pub(super) fn reading(&mut self, ty: &Type) -> Value {
        match ty {
            Type::Base(ty) => Value::new(base_type(*ty).to_owned(), Class::Other),
            Type::Alphabet { symbols, options } => {
                let symbols = Value::new(quoted(symbols), Class::Str);
                let mut value = self.combine(vec![symbols], |values| {
                    Value::new(format!("alphabet({})", values[0].text), Class::Other)
                });
                for option in options {
                    let (method, argument) = alphabet_option(option);
                    value = self.combine(vec![value, argument], |values| {
                        let text = format!("{}.{method}({})", values[0].text, values[1].text);
                        Value::new(text, Class::Other)
                    });
                }
                value
            }
            Type::Named(name) => Value::new(self.type_name(name), Class::Other),
            Type::Range {
                lower,
                upper,
                exclusive,
            } => self.range(*lower, *upper, *exclusive),
        }
    }

    fn range(&mut self, lower: Option<Expr>, upper: Option<Expr>, exclusive: bool) -> Value {
        let operator = if exclusive { ".." } else { "..=" };
        let lower = lower.map(|lower| self.expr(lower));
        let upper = upper.map(|upper| self.expr(upper));
        let value = match (lower, upper) {
            (Some(lower), Some(upper)) => self.combine(vec![lower, upper], |values| {
                let (lower, upper) = matched_bounds(&values[0], &values[1], exclusive);
                range_value(format!("{lower}{operator}{upper}"))
            }),
            (Some(lower), None) => self.combine(vec![lower], |values| {
                range_value(format!("{}..", single_bound(&values[0])))
            }),
            (None, Some(upper)) => self.combine(vec![upper], |values| {
                range_value(format!("{operator}{}", single_bound(&values[0])))
            }),
            (None, None) => unreachable!("the parser needs a bound in a range"),
        };
        Value {
            precedence: Precedence::Range,
            ..value
        }
    }

    /// The type as a `BaseType`, which has `.dim`, `.layout` and `.until`.
    fn base_reading(&mut self, ty: &Type) -> Value {
        let value = self.reading(ty);
        if matches!(ty, Type::Base(_)) {
            return value;
        }
        self.combine(vec![value], |values| {
            Value::new(format!("ty({})", values[0].text), Class::Other)
        })
    }

    /// The type of a declaration, with its options.
    pub(super) fn declared(&mut self, declared: &Declared) -> Value {
        if declared.dimensions.is_empty() && declared.layout.is_none() && declared.offset.is_none()
        {
            return self.reading(&declared.ty);
        }
        let mut value = self.base_reading(&declared.ty);
        for &size in &declared.dimensions {
            value = self.option(value, "dim", size);
        }
        if let Some(layout) = declared.layout {
            value = self.option(value, "layout", layout);
        }
        if let Some(offset) = declared.offset {
            value = self.option(value, "offset", offset);
        }
        value
    }

    fn option(&mut self, reading: Value, method: &str, argument: Expr) -> Value {
        let argument = self.expr(argument);
        self.combine(vec![reading, argument], |values| {
            let text = format!("{}.{method}({})", values[0].text, values[1].text);
            Value::new(text, Class::Other)
        })
    }

    /// The reading of a `list`.
    pub(super) fn list_reading(&mut self, declared: &Declared, end: &ListEnd) -> Value {
        let reading = self.base_reading(&declared.ty);
        let mut value = match end {
            ListEnd::Eol => self.combine(vec![reading], |values| {
                Value::new(format!("{}.until(eol())", values[0].text), Class::Other)
            }),
            ListEnd::Expr(end) => self.option(reading, "until", *end),
        };
        if let Some(offset) = declared.offset {
            value = self.option(value, "offset", offset);
        }
        value
    }

    fn capture_type(&mut self, ty: &Option<Type>) -> Value {
        match ty {
            Some(ty) => self.reading(ty),
            None => Value::new("untyped()".to_owned(), Class::Other),
        }
    }

    fn part(&mut self, function: &str, arguments: Vec<Value>) -> String {
        let value = self.combine(arguments, |values| {
            let arguments: Vec<&str> = values.iter().map(|value| value.text.as_str()).collect();
            let text = format!("{function}({})", arguments.join(", "));
            Value::new(text, Class::Other)
        });
        format!(".part({})", value.text)
    }

    fn handle(&self, name: NameId) -> Value {
        Value::new(self.value_names[name.0].clone(), Class::Handle)
    }

    fn piece(&mut self, piece: &Piece) -> String {
        match piece {
            Piece::Text(text) => self.chain_call("text", quoted(text)),
            Piece::Bind { name, ty } => {
                let ty = self.capture_type(ty);
                self.part("bind", vec![self.handle(*name), ty])
            }
            Piece::Reference(name) => self.part("reference", vec![self.handle(*name)]),
            Piece::Validate(ty) => {
                let ty = self.reading(ty);
                self.part("validate", vec![ty])
            }
            Piece::Blank => ".part(blank())".to_owned(),
            Piece::Discard => ".part(discard())".to_owned(),
            Piece::DiscardAs(ty) => {
                let ty = self.reading(ty);
                self.part("discard_as", vec![ty])
            }
            Piece::RestOfLine => ".part(rest_of_line())".to_owned(),
        }
    }

    /// A pattern, as the argument of a builder call.
    pub(super) fn pattern(&mut self, pieces: &[Piece]) -> String {
        match pieces {
            [] => return "\"\"".to_owned(),
            [Piece::Text(text)] => return quoted(text),
            _ => {}
        }
        let parts: Vec<String> = pieces.iter().map(|piece| self.piece(piece)).collect();
        let one_line = format!("Pattern::new(){}", parts.concat());
        if one_line.len() <= self.budget() {
            return one_line;
        }
        let name = self.temporary();
        let head = format!("let {name}");
        let statement = format!("{head} = {one_line};");
        // A chain with one call has no width of its own.
        let narrow = parts.len() == 1 || one_line.len() <= CALL_WIDTH;
        if narrow && (self.fits(&statement) || self.fits_next_line(&format!("{one_line};"))) {
            self.let_statement(&head, &one_line);
            return name;
        }
        self.line(format!("{head} = Pattern::new()"));
        self.indent += 1;
        let last = parts.len() - 1;
        for (index, part) in parts.into_iter().enumerate() {
            let end = if index == last { ";" } else { "" };
            self.line(format!("{part}{end}"));
        }
        self.indent -= 1;
        name
    }
}

fn range_value(text: String) -> Value {
    Value::new(text, Class::Other)
}
