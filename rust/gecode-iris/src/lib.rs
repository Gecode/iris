//! The public Iris reader API, re-exported from `iris-core`.
//!
//! Parse a reader spec once with [`Spec::parse`], supply caller parameters through
//! [`Params`], then read input bytes with [`Spec::read`] or a file with
//! [`Spec::read_path`]. The resulting [`Data`] owns its entries and provides
//! borrowed row slices. [`Spec::catalog`] lists the instances in a file.
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
