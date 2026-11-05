//! The `iris` console script's entry point into the Rust command line.
use pyo3::prelude::*;
use std::ffi::OsString;
use std::panic::{self, AssertUnwindSafe};

/// The exit status of a Rust program whose main thread panics.
const PANIC_STATUS: u8 = 101;

/// Runs the `iris` command line on `argv`, the program name first, and
/// returns its exit status. Output goes straight to the process's standard
/// streams.
///
/// A panic ends the command as it ends the binary: the panic hook has
/// already printed the message, and the status is 101, so no Python
/// exception crosses the boundary.
#[pyfunction]
pub fn run_cli(py: Python<'_>, argv: Vec<OsString>) -> u8 {
    py.detach(|| {
        panic::catch_unwind(AssertUnwindSafe(|| gecode_iris_cli::run(argv))).unwrap_or(PANIC_STATUS)
    })
}
