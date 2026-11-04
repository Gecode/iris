use gecode_iris::Data;
use pyo3::{exceptions::PyRuntimeError, prelude::*};

/// The data from one read or catalog.
#[pyclass(name = "Data", module = "gecode_iris", frozen)]
pub struct PyData(Data);

impl From<Data> for PyData {
    fn from(data: Data) -> Self {
        Self(data)
    }
}

#[pymethods]
impl PyData {
    /// The canonical JSON document, ending in a newline.
    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        let document = py.detach(|| self.0.to_json()).map_err(rendering_error)?;
        String::from_utf8(document).map_err(rendering_error)
    }

    /// The MiniZinc data text defined in the Specification's appendix B.
    fn to_dzn(&self, py: Python<'_>) -> PyResult<String> {
        let document = py.detach(|| self.0.to_dzn()).map_err(rendering_error)?;
        String::from_utf8(document).map_err(rendering_error)
    }
}

fn rendering_error(error: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(format!("cannot render data: {error}"))
}
