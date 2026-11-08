//! Reading types and patterns.
use super::Renderer;
use super::expr::{Class, Form, Value, int_literal};
use crate::program::{AlphabetOption, BaseType, Declared, Expr, ListEnd, Piece, Type};

fn base_type(ty: BaseType) -> &'static str {
    match ty {
        BaseType::Int => "int",
        BaseType::Float => "float",
        BaseType::Num => "num",
        BaseType::Word => "word",
        BaseType::Rest => "rest",
    }
}

/// The TypeScript type of a reading type: what `defineType` gives back for
/// it.
fn base_class(ty: BaseType) -> &'static str {
    match ty {
        BaseType::Int => "Type<\"int\", 0>",
        BaseType::Float => "Type<\"float\", 0>",
        BaseType::Num => "Type<\"num\", 0>",
        BaseType::Word | BaseType::Rest => "Type<\"string\", 0>",
    }
}

/// `text` as the text of a template literal, in ASCII: a backtick, a
/// backslash and a `$` before `{` have escapes of their own, and everything
/// outside printable ASCII is a `\u` escape of its UTF-16 units.
fn template_text(text: &str) -> String {
    let mut literal = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\\' => literal.push_str("\\\\"),
            '`' => literal.push_str("\\`"),
            '$' if characters.peek() == Some(&'{') => literal.push_str("\\$"),
            '\n' => literal.push_str("\\n"),
            '\r' => literal.push_str("\\r"),
            '\t' => literal.push_str("\\t"),
            ' '..='~' => literal.push(character),
            _ => {
                let mut units = [0; 2];
                for unit in character.encode_utf16(&mut units) {
                    literal.push_str(&format!("\\u{unit:04x}"));
                }
            }
        }
    }
    literal
}

impl Renderer<'_> {
    /// The TypeScript type of `ty`.
    pub(super) fn type_class(&self, ty: &Type) -> &'static str {
        match ty {
            Type::Base(ty) => base_class(*ty),
            Type::Alphabet { .. } | Type::Range { .. } => "Type<\"int\", 0>",
            Type::Named(name) => {
                self.visible_types
                    .iter()
                    .rev()
                    .find(|visible| visible.name == *name)
                    .expect("a type that a type uses is visible")
                    .ts_type
            }
        }
    }

    /// A reading type, as the argument of a builder call.
    pub(super) fn reading(&mut self, ty: &Type) -> Value {
        match ty {
            Type::Base(ty) => Value::name(base_type(*ty).to_owned(), Class::Other),
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
        let mut value = self.part("alphabet", vec![symbols]);
        for option in options {
            let (method, argument) = match option {
                AlphabetOption::Blank(text) => ("blank", Value::string(text)),
                AlphabetOption::Strip(text) => ("strip", Value::string(text)),
                AlphabetOption::Base(base) => ("base", Value::number(int_literal(*base))),
            };
            value = self.combine(vec![value, argument], |values| {
                values[0].method(method, &[values[1].clone()], Class::Other)
            });
        }
        value
    }

    /// `range(lower, upper)`, `range(lower).below(upper)`, `atLeast(lower)`,
    /// `atMost(upper)` or `below(upper)`.
    fn range(&mut self, lower: Option<Expr>, upper: Option<Expr>, exclusive: bool) -> Value {
        match (lower, upper) {
            (Some(lower), Some(upper)) => {
                let operands = vec![self.expr(lower), self.expr(upper)];
                self.combine(operands, |values| {
                    let (lower, upper) = (values[0].bound(), values[1].bound());
                    if exclusive {
                        let range = Value::call("range", vec![lower], Class::Other);
                        range.method("below", &[upper], Class::Other)
                    } else {
                        Value::call("range", vec![lower, upper], Class::Other)
                    }
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
            (None, None) => Value::name("int".to_owned(), Class::Other),
        }
    }

    /// The type of a declaration, with its options.
    pub(super) fn declared(&mut self, declared: &Declared) -> Value {
        let mut reading = self.reading(&declared.ty);
        for &size in &declared.dimensions {
            let size = self.expr(size).integral();
            reading = self.option(reading, "dim", size);
        }
        if let Some(layout) = declared.layout {
            let layout = self.expr(layout);
            reading = self.option(reading, "layout", layout);
        }
        if let Some(offset) = declared.offset {
            let offset = self.expr(offset);
            reading = self.option(reading, "offset", offset);
        }
        reading
    }

    /// `reading.method(argument)`.
    fn option(&mut self, reading: Value, method: &str, argument: Value) -> Value {
        self.combine(vec![reading, argument], |values| {
            values[0].method(method, &[values[1].clone()], Class::Other)
        })
    }

    /// The reading of a `list`.
    pub(super) fn list_reading(&mut self, declared: &Declared, end: &ListEnd) -> Value {
        let reading = self.reading(&declared.ty);
        let end = match end {
            ListEnd::Eol => Value::name("eol".to_owned(), Class::Other),
            ListEnd::Expr(end) => self.expr(*end),
        };
        let mut value = self.option(reading, "until", end);
        if let Some(offset) = declared.offset {
            let offset = self.expr(offset);
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

    /// The substitution of a piece that is not text.
    fn substitution(&mut self, piece: &Piece) -> Value {
        let call = |text: &str| Value::call(text, Vec::new(), Class::Other);
        match piece {
            Piece::Text(text) => Value::string(text),
            // A forward handle is no capture, so `{x}` is `bind(x)`.
            Piece::Bind { name, ty: None } => {
                let handle = self.handle(*name);
                self.part("bind", vec![handle])
            }
            Piece::Bind { name, ty: Some(ty) } => {
                let handle = self.handle(*name);
                let ty = self.reading(ty);
                self.part("bind", vec![handle, ty])
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
                self.part("discardAs", vec![ty])
            }
            Piece::RestOfLine => Value::name("restOfLine".to_owned(), Class::Other),
        }
    }

    /// A line pattern: a string if it is only text, and a template literal
    /// with the tag `p` otherwise, which no layout splits.
    pub(super) fn pattern(&mut self, pieces: &[Piece]) -> Value {
        if pieces.iter().all(|piece| matches!(piece, Piece::Text(_))) {
            let text: String = pieces
                .iter()
                .map(|piece| match piece {
                    Piece::Text(text) => text.as_str(),
                    _ => "",
                })
                .collect();
            return Value::string(&text);
        }
        let mut text = String::from("p`");
        for piece in pieces {
            match piece {
                Piece::Text(piece) => text.push_str(&template_text(piece)),
                piece => {
                    let value = self.substitution(piece);
                    text.push_str(&format!("${{{}}}", value.text));
                }
            }
        }
        text.push('`');
        Value::opaque(text, Class::Other, Form::Other)
    }
}

#[cfg(test)]
mod tests {
    use super::template_text;

    #[test]
    fn template_text_escapes_what_a_template_reads() {
        assert_eq!(template_text("a b"), "a b");
        assert_eq!(template_text("`x`"), "\\`x\\`");
        assert_eq!(template_text("$5 ${y}"), "$5 \\${y}");
        assert_eq!(template_text("back\\slash"), "back\\\\slash");
        assert_eq!(template_text("é\t"), "\\u00e9\\t");
        assert_eq!(template_text("😀"), "\\ud83d\\ude00");
    }
}
