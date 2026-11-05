//! Line patterns. The `pattern!` macro expands to the calls here.
use crate::expr::{Name, ValueTag, ValueType};
use crate::field::{BuilderId, Field};
use crate::types::{Declared, IntoDeclared};
use iris_core::handle::{D0, Str};

/// A capture's type: `{x:type}`, or [`untyped`] for `{x}`.
#[derive(Debug, Clone)]
pub enum CaptureSpec {
    Untyped,
    Typed(Declared),
}

/// What a capture accepts: any type without dimensions, or [`untyped`].
pub trait CaptureType {
    type Value: ValueType;
    #[doc(hidden)]
    fn into_capture(self) -> CaptureSpec;
}

impl<R: IntoDeclared<Dims = D0>> CaptureType for R {
    type Value = R::Value;
    fn into_capture(self) -> CaptureSpec {
        CaptureSpec::Typed(self.into_declared())
    }
}

/// The capture-only form `{x}`, which reads a word without naming a type.
#[derive(Debug, Clone, Copy)]
pub struct Untyped;

/// `{x}`. It reads what `word()` reads, but `word()` prints `{x:word}`.
pub fn untyped() -> Untyped {
    Untyped
}

impl CaptureType for Untyped {
    type Value = Str;
    fn into_capture(self) -> CaptureSpec {
        CaptureSpec::Untyped
    }
}

/// One part of a pattern.
#[derive(Debug, Clone)]
pub enum Piece {
    Text(String),
    /// A handle from `capture`, printed from its declaration.
    Capture(Name),
    /// `{x:type}` through an existing handle.
    Bind {
        name: Name,
        value: ValueTag,
        capture: CaptureSpec,
    },
    /// `{$x}`.
    Reference(Name),
    /// `{:type}`.
    Validate(CaptureSpec),
    /// `{}`.
    Blank,
    /// `{_}`.
    Discard,
    /// `{_:type}`.
    DiscardAs(CaptureSpec),
    /// The trailing `...`.
    RestOfLine,
}

/// A line pattern: literal text with captures in it.
///
/// `Pattern::new().text("n = ").part(n)` is what `pattern!("n = {n}")` expands
/// to. [`Pattern::text`] takes literals computed at run time; one containing
/// `{` or `...` has no spelling, and `build()` rejects it.
#[derive(Debug, Clone, Default)]
pub struct Pattern {
    pub(crate) pieces: Vec<Piece>,
}

impl Pattern {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append literal text.
    pub fn text(mut self, text: impl AsRef<str>) -> Self {
        let text = text.as_ref();
        match self.pieces.last_mut() {
            Some(Piece::Text(last)) => last.push_str(text),
            _ => self.pieces.push(Piece::Text(text.into())),
        }
        self
    }

    /// Append a capture or marker.
    pub fn part(mut self, part: impl PatternPart) -> Self {
        self.pieces.push(part.into_piece());
        self
    }
}

impl From<&str> for Pattern {
    fn from(text: &str) -> Self {
        Self::new().text(text)
    }
}

/// What `pattern!` can interpolate: a capture handle, or one of [`bind`],
/// [`reference()`], [`validate`], [`blank`], [`discard`](crate::discard),
/// [`discard_as`] and [`rest_of_line`].
pub trait PatternPart {
    #[doc(hidden)]
    fn into_piece(self) -> Piece;
}

fn name_of<V, D, O>(field: Field<V, D, O>) -> Name {
    Name {
        text: field.name,
        owner: field.owner,
    }
}

impl<V, D, O> PatternPart for Field<V, D, O> {
    fn into_piece(self) -> Piece {
        Piece::Capture(name_of(self))
    }
}

/// Wraps a piece built by one of the helper functions.
#[derive(Debug, Clone)]
pub struct Part(Piece);

impl PatternPart for Part {
    fn into_piece(self) -> Piece {
        self.0
    }
}

/// `{x:type}`, binding a forward or dynamic handle.
pub fn bind<V, D, O, T: CaptureType>(handle: Field<V, D, O>, capture: T) -> Part {
    Part(Piece::Bind {
        name: name_of(handle),
        value: T::Value::TAG,
        capture: capture.into_capture(),
    })
}

/// `{$x}`, matching the current value of `x`.
pub fn reference<V, D, O>(handle: Field<V, D, O>) -> Part {
    Part(Piece::Reference(name_of(handle)))
}

/// `{:type}`, which checks the text without storing it.
pub fn validate<T: CaptureType>(capture: T) -> Part {
    Part(Piece::Validate(capture.into_capture()))
}

/// `{}`, which matches a word without storing it.
pub fn blank() -> Part {
    Part(Piece::Blank)
}

/// `{_:type}`, a discarded capture with a type.
pub fn discard_as<T: CaptureType>(capture: T) -> Part {
    Part(Piece::DiscardAs(capture.into_capture()))
}

/// The trailing `...` of a pattern.
pub fn rest_of_line() -> Part {
    Part(Piece::RestOfLine)
}

/// The name a piece mentions, with the builder that made its handle.
pub(crate) fn piece_name(piece: &Piece) -> Option<(&'static str, BuilderId)> {
    match piece {
        Piece::Capture(name) | Piece::Bind { name, .. } | Piece::Reference(name) => {
            Some((name.text, name.owner))
        }
        _ => None,
    }
}
