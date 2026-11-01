//! The public Iris reader API, re-exported from `iris-core`.
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
pub use iris_core::*;
