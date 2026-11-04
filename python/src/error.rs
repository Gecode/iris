//! Maps reader failures to Python exceptions. Only language errors become
//! `gecode_iris.Error`; host failures use Python's built-in exceptions and
//! carry no error code.
use gecode_iris::{Error, Failure, HostError};
use pyo3::{
    exceptions::{PyNotImplementedError, PyOSError, PyRuntimeError, PyValueError},
    prelude::*,
    sync::PyOnceLock,
    types::{PyDict, PyTuple, PyType},
};
use std::{io, path::Path};

/// Converts a failure; `path` is the file being read, when there is one.
pub fn failure_to_err(py: Python<'_>, failure: Failure, path: Option<&Path>) -> PyErr {
    match failure {
        Failure::Language(error) => language_error(py, &error).unwrap_or_else(|err| err),
        Failure::Host(HostError::Io(error)) => io_error(py, error, path),
        Failure::Host(error @ HostError::SourceTooLarge { .. }) => {
            PyValueError::new_err(error.to_string())
        }
        Failure::Unsupported(feature) => PyNotImplementedError::new_err(feature.to_string()),
        other => PyRuntimeError::new_err(other.to_string()),
    }
}

/// Builds `OSError(errno, strerror, filename)`, from which CPython picks the
/// subclass, such as `FileNotFoundError`.
pub fn io_error(py: Python<'_>, error: io::Error, path: Option<&Path>) -> PyErr {
    let Some(code) = error.raw_os_error() else {
        return PyOSError::new_err(error.to_string());
    };
    let filename = path.map(|path| path.as_os_str().to_owned());
    #[cfg(not(windows))]
    let arguments = (
        code,
        strerror(py, code).unwrap_or_else(|| error.to_string()),
        filename,
    );
    // On Windows the code is a Windows error, which OSError takes as `winerror`.
    #[cfg(windows)]
    let arguments = (py.None(), error.to_string(), filename, code);
    PyOSError::new_err(arguments)
}

/// Python's own text for an errno, so messages match those of `open`.
#[cfg(not(windows))]
fn strerror(py: Python<'_>, code: i32) -> Option<String> {
    py.import("os")
        .and_then(|os| os.call_method1("strerror", (code,)))
        .and_then(|text| text.extract())
        .ok()
}

fn language_error(py: Python<'_>, error: &Error) -> PyResult<PyErr> {
    static ERROR_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();
    let error_type = ERROR_TYPE.import(py, "gecode_iris._error", "Error")?;
    // Display, the canonical error document and the conformance projection
    // all report the input location when there is one.
    let location = error.input_location().or(error.spec_location());
    let document = error
        .to_json()
        .map_err(|e| PyRuntimeError::new_err(format!("cannot render error: {e}")))?;
    let fields = PyDict::new(py);
    fields.set_item("code", error.code.as_str())?;
    fields.set_item("message", error.message())?;
    fields.set_item("line", location.and_then(|l| l.line))?;
    fields.set_item("column", location.and_then(|l| l.column))?;
    let path = error
        .record_path()
        .map(|path| PyTuple::new(py, path))
        .transpose()?;
    fields.set_item("path", path)?;
    fields.set_item("origin", location.and_then(|l| l.origin.as_deref()))?;
    fields.set_item("document", String::from_utf8_lossy(&document))?;
    let instance = error_type.call((error.to_string(),), Some(&fields))?;
    Ok(PyErr::from_value(instance))
}
