//! The public Iris reader API, re-exported from `iris-core`.
//!
//! Parse a reader spec once with [`Spec::parse`], supply caller parameters through
//! [`Params`], then read input bytes with [`Spec::read`] or a file with
//! [`Spec::read_path`]. The resulting [`Data`] owns its entries and provides
//! borrowed row slices. [`Spec::catalog`] lists the instances in a file.
//! For a reader spec fixed at compile time, [`iris!`] checks the spec when the
//! program compiles and returns the same [`Spec`].
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
pub use iris_core::*;

/// Build reader specs in Rust, with typed handles.
///
/// See the [`iris_build`] crate documentation.
pub mod build {
    pub use iris_build::*;
}

#[doc(hidden)]
pub mod __private {
    pub use iris_macros::iris;
}

/// Check a reader spec at compile time and return its [`Spec`].
///
/// The spec is written inline as a string literal. A raw literal keeps Iris
/// syntax that Rust's tokenizer would reject, such as `#` comments and
/// unbalanced quotes:
///
/// ```
/// use gecode_iris::{Params, iris};
///
/// let spec = iris!(r#"
///     n:int  # the row length
///     row[n]:int
/// "#);
/// let data = spec.read(b"3 10 20 30", "input.txt", &Params::new())?;
/// assert_eq!(data.int("n")?, 3);
/// assert_eq!(data.ints("row", &[])?, &[10, 20, 30]);
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
/// written for a file. The expansion behaves exactly like [`Spec::parse`] on
/// the same bytes and origin.
///
/// The argument is a string literal without a suffix, or `file = ` followed
/// by such a literal. `iris!` sees macro calls such as `concat!` or
/// `include_str!` before they expand, so it rejects them; use the file form
/// instead.
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
/// microseconds. To parse once, keep the [`Spec`] and reuse it. `Spec` is not
/// `Sync`, so a lazily initialized global spec belongs in a `thread_local!`:
///
/// ```
/// use gecode_iris::{Params, Spec, iris};
///
/// thread_local! {
///     static PAIR: Spec = iris!("a:int b:int");
/// }
/// let sum = PAIR.with(|spec| -> Result<i64, Box<dyn std::error::Error>> {
///     let data = spec.read(b"1 2", "input.txt", &Params::new())?;
///     Ok(data.int("a")? + data.int("b")?)
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
