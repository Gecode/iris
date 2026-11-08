//! The public Iris reader API, re-exported from `gecode-iris-core`.
//!
//! Parse a reader spec once with [`Spec::parse`], supply caller parameters through
//! [`Params`], then read input bytes with [`Spec::read`] or a file with
//! [`Spec::read_path`]. The resulting [`Data`] owns its entries and provides
//! borrowed row slices. [`Spec::catalog`] lists the instances in a file.
//! For a reader spec fixed at compile time, [`iris!`] checks the spec when the
//! program compiles and returns the same [`Spec`] with its typed handles.
//!
//! ```
//! use gecode_iris::{Params, Spec};
//!
//! let spec = Spec::parse(b"n:int row[n]:int", "example.iris")?;
//! let data = spec.read(b"3 10 20 30", "input.txt", &Params::new())?;
//! assert_eq!(data.int("n")?, 3);
//! assert_eq!(data.ints("row", &[])?, &[10, 20, 30]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Failures
//!
//! Parsing and reading return [`Failure`]. [`Failure::Language`] contains an
//! [`Error`] with a stable language error code, source locations, and, when
//! applicable, a record path. This includes Iris work-budget exhaustion.
//! [`Failure::Host`] contains a [`HostError`], such as file I/O failure, host
//! cancellation, or a source larger than the syntax tree can represent. These
//! failures have no language error code. [`Failure::Unsupported`] describes an
//! implementation feature that has not been implemented.
//!
//! After a successful read, entry lookup and typed access return [`AccessError`]
//! for an unknown entry, invalid index path, or incompatible requested type.
//! These are host API usage errors, separate from reading failures.
//! [`Failure`], [`HostError`], and [`AccessError`] are non-exhaustive; callers
//! matching their variants must include a fallback arm.
pub use gecode_iris_core::*;

/// Build reader specs in Rust, with typed handles.
///
/// See the [`gecode_iris_build`] crate documentation.
pub mod build {
    pub use gecode_iris_build::*;
}

#[doc(hidden)]
pub mod __private {
    pub use gecode_iris_macros::iris;
}

/// Check a reader spec at compile time and return its [`Spec`] and typed
/// handles.
///
/// The value has a `spec` field, the [`Spec`], and one field per entry of the
/// spec: the typed handle that `iris gen --lang rust` prints for it, with the
/// same name, type and depth. [`Data::get`] reads through a handle without a
/// name or type to get wrong. The spec is written inline as a string literal.
/// A raw literal keeps Iris syntax that Rust's tokenizer would reject, such as
/// `#` comments and unbalanced quotes:
///
/// ```
/// use gecode_iris::{Params, iris};
///
/// let row = iris!(r#"
///     n:int  # the row length
///     row[n]:int
/// "#);
/// let data = row.spec.read(b"3 10 20 30", "input.txt", &Params::new())?;
/// let n: i64 = data.get(row.n);
/// let values: &[i64] = data.get(row.row);
/// assert_eq!(n, 3);
/// assert_eq!(values, &[10, 20, 30]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// `iris!(file = "specs/dimacs-cnf.iris")` reads the spec from a file instead.
/// The path is relative to the calling package's `CARGO_MANIFEST_DIR`; an
/// absolute path is used as written. The program embeds the file's bytes,
/// and Cargo rebuilds it when the file changes. The file may contain any
/// bytes: invalid UTF-8 is reported as `E-SPEC-UTF8` at compile time.
///
/// The origin in diagnostics is `iris!` for an inline spec and the path as
/// written for a file. The `spec` field behaves exactly like [`Spec::parse`]
/// on the same bytes and origin.
///
/// The argument is a string literal without a suffix, or `file = ` followed
/// by such a literal. `iris!` sees macro calls such as `concat!` or
/// `include_str!` before they expand, so it rejects them; use the file form
/// instead.
///
/// # Handles
///
/// A handle's type is a [`build::Field`]: `Field<Int>` for an `int` scalar,
/// `Field<Int, D2>` for an array of arrays, and `Field<Str, D0, Maybe>` for a
/// word that may be absent, which [`Data::get`] returns as an `Option`. An
/// entry whose value type or depth the spec leaves open, or that is deeper
/// than [`build::D8`], has a [`build::DynField`]. A field is named like its
/// entry, in snake case. An entry whose name is a Rust keyword or `spec`, or
/// that would get the name of another entry, gets a numeric suffix:
/// `fn` is the field `fn_2`, and `spec` is `spec_2`. The handle keeps the
/// entry's name, so it reads the right entry whatever its field is called.
///
/// The value's type is a struct that the expansion declares, so code outside
/// the expansion cannot name it. Bind the value with `let`. To keep the parts
/// in a type of your own, store `spec` and copy the handles, whose types are
/// those `iris gen --lang rust` prints; or generate that module with
/// `iris gen` and name it yourself.
///
/// # Errors
///
/// An invalid spec fails to compile. The message is the error code followed
/// by the text `iris check` prints for the same spec:
///
/// ```text
/// error: E-SPEC-UNKNOWN-TYPE: iris!:1: unknown type 'integer'
///        1 | x:integer
///          |          ^
/// ```
///
/// The compiler marks the whole string literal, or the file path. On a
/// nightly compiler with the `proc_macro_span` feature it marks the exact
/// position inside the literal.
///
/// # Cost
///
/// Each evaluation parses the spec again at run time, which takes
/// microseconds. To parse once, keep the value and reuse it. `Spec` is not
/// `Sync`, so a lazily initialized global spec belongs in a `thread_local!`,
/// whose type names the parts that it keeps:
///
/// ```
/// use gecode_iris::build::{Field, Int};
/// use gecode_iris::{Params, Spec, iris};
///
/// thread_local! {
///     static PAIR: (Spec, Field<Int>, Field<Int>) = {
///         let pair = iris!("a:int b:int");
///         (pair.spec, pair.a, pair.b)
///     };
/// }
/// let sum = PAIR.with(|(spec, a, b)| -> Result<i64, Box<dyn std::error::Error>> {
///     let data = spec.read(b"1 2", "input.txt", &Params::new())?;
///     Ok(data.get(*a) + data.get(*b))
/// })?;
/// assert_eq!(sum, 3);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[macro_export]
macro_rules! iris {
    ($($input:tt)*) => {
        $crate::__private::iris!([$crate] $($input)*)
    };
}
