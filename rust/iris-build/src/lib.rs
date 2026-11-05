//! Build Iris reader specs in Rust, with typed handles.
//!
//! A builder writes a reader spec: every call adds a statement, and
//! [`SpecBuilder::build`] prints the text and parses it with
//! [`Spec::parse`](gecode_iris_core::Spec::parse). The result is an ordinary spec, so
//! the language's own checks decide what is valid. The builder adds only the
//! checks that a text spec cannot need; see [`BuildErrorKind`].
//!
//! Every defining statement returns a typed handle, a [`Field`]. In the builder
//! a handle is an expression. After reading it is a key, and
//! [`Data::get`](gecode_iris_core::Data::get) returns the host type for its value type
//! and depth:
//!
//! ```
//! use gecode_iris_build::{SpecBuilder, int, lit};
//! use gecode_iris_core::Params;
//!
//! let mut b = SpecBuilder::new("row");
//! let n = b.read("n", int());
//! let row = b.read("row", int().dim(n));
//! b.check(n.gt(0)).message("the row is empty");
//! let spec = b.build()?;
//!
//! let data = spec.read(b"2 10 20", "input.txt", &Params::new())?;
//! assert_eq!(data.get(n), 2);
//! assert_eq!(data.get(row), &[10, 20]);
//! # let _ = lit(0);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Errors
//!
//! Every statement records the line that called it. A [`BuildError`] shows the
//! language error code and message, the line of emitted text, and that
//! [call site](BuildError::call_site). A failure while reading the built spec
//! gives one through [`Error::call_site`](gecode_iris_core::Error::call_site). A helper
//! function needs `#[track_caller]` to record its caller instead of itself.
//!
//! # Dynamic forms
//!
//! Every spec that `iris check` accepts can be built, including the ones
//! static types would refuse. Four forms step outside the static types:
//!
//! - [`Scope::forward`] gives a handle before its definition. A later defining
//!   statement takes it as the target, as does [`bind`] in a pattern.
//! - [`dynamic`](Expr::dynamic) gives an expression with no static type, which
//!   every operator accepts.
//! - A handle as the target of any defining statement redefines its name and
//!   gives a [`DynField`]. A name given as a string is a new name, and defining
//!   it twice fails [`build`](SpecBuilder::build). [`Scope::set`] is the form
//!   that keeps the type.
//! - [`Scope::undefined`] gives a handle for a name deliberately left undefined,
//!   as in `check !defined(name)`.
//!
//! ```
//! use gecode_iris_build::{DynField, Field, SpecBuilder, float, int, lit};
//!
//! let mut b = SpecBuilder::unnamed();
//! let x = b.read("x", int());
//! let x = b.read(x, float()); // redefined, so a `DynField`
//! let y: Field<gecode_iris_build::Int> = b.forward("y");
//! b.let_("sum", y + x.dynamic());
//! b.let_(y, lit(1));
//! let zz: DynField = b.undefined("zz");
//! b.check(!gecode_iris_build::defined(zz));
//! assert!(b.source().is_ok());
//! ```
mod check;
mod error;
mod expr;
mod field;
mod pattern;
mod print;
mod scope;
mod statement;
mod types;

pub use error::{BuildError, BuildErrorKind};
pub use expr::{
    Args, Bound, Definable, Domain, Equatable, Expr, ExprShape, Host, Integral, LetDepth, Literal,
    Numeric, Open, Operand, Over, RecordExpr, RecordList, SameDepth, Scalar, ValueType, all, any,
    count, count_over, defined, distinct, gecode, lit, max, min, over, size, sum, sum_over,
};
pub use field::{DynField, Field, Forward};
pub use gecode_iris_core::handle::{
    Array, D0, D1, D2, D3, D4, D5, D6, D7, D8, Depth, Dyn, DynDepth, Float, Int, Leaf, Maybe, Num,
    Number, Numbers, Presence, Required, Str,
};
pub use pattern::{
    CaptureType, Part, Pattern, PatternPart, Untyped, bind, blank, discard_as, reference,
    rest_of_line, untyped, validate,
};
pub use scope::{Arm, Arms, Check, If, Nested, Repeat, RepeatUntil, Scope, Sections, SpecBuilder};
pub use statement::{Discard, Record, Strings, Target, discard};
pub use types::{
    Alphabet, BaseType, Eol, IntoDeclared, ListEnd, ListReading, NamedType, alphabet, eol, float,
    int, num, rest, ty, word,
};

#[doc(hidden)]
pub mod __private {
    pub use crate::pattern::Pattern;
    pub use gecode_iris_macros::pattern;
}

/// A line pattern, written like `format!`.
///
/// `pattern!("n = {n}")` interpolates the capture handle `n` from the scope;
/// `pattern!("{a}", a = expr)` takes a named argument. Anything that implements
/// [`PatternPart`] can be interpolated. Text is literal, except that `{{`, an
/// unclosed `{` and `...` are compile errors, since no pattern can spell them.
/// Use [`rest_of_line`] for the trailing `...`.
#[macro_export]
macro_rules! pattern {
    ($($input:tt)*) => {
        $crate::__private::pattern!([$crate] $($input)*)
    };
}
