//! The `gecode_iris._native` extension module. The `gecode_iris` package
//! re-exports its classes; the Rust reader in `rust/gecode-iris` does all the
//! parsing, reading and rendering.
mod data;
mod error;
mod spec;

use pyo3::prelude::*;

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<spec::PySpec>()?;
    module.add_class::<data::PyData>()?;
    Ok(())
}
