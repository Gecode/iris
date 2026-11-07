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

/// The line a built spec's error adds after its location and context.
const CALL_SITE: &str = "\ncall site: ";
/// The line that starts the summary of an earlier rejected candidate, which
/// comes after the call site.
const EARLIER_REJECTION: &str = "\n  after an earlier error in the instance: ";

/// Converts a failure; `path` is the file being read, when there is one, and
/// `call_sites` maps the statements of a built spec to their call sites.
pub fn failure_to_err(
    py: Python<'_>,
    failure: Failure,
    path: Option<&Path>,
    call_sites: Option<&Py<PyAny>>,
) -> PyErr {
    match failure {
        Failure::Language(error) => {
            language_error(py, &error, call_sites).unwrap_or_else(|err| err)
        }
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

fn language_error(
    py: Python<'_>,
    error: &Error,
    call_sites: Option<&Py<PyAny>>,
) -> PyResult<PyErr> {
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
    let mut text = error.to_string();
    if let Some(site) = call_site(py, error, call_sites)? {
        // The core error shows its call site where `CallSites::locate` puts
        // it: before the summary of an earlier rejected candidate.
        let line = format!("{CALL_SITE}{}", safe_text(&site.str()?.to_cow()?));
        let at = text.find(EARLIER_REJECTION).unwrap_or(text.len());
        text.insert_str(at, &line);
        fields.set_item("call_site", site)?;
    }
    let instance = error_type.call((text,), Some(&fields))?;
    Ok(PyErr::from_value(instance))
}

/// The call site of the statement an error belongs to, by the rule of
/// `CallSites::locate`: the statement that owns the error, or else the one
/// its spec span starts in.
fn call_site<'py>(
    py: Python<'py>,
    error: &Error,
    call_sites: Option<&Py<PyAny>>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let (Some(call_sites), Some(offset)) = (
        call_sites,
        error.owner().or_else(|| Some(error.spec_span()?.start())),
    ) else {
        return Ok(None);
    };
    let site = call_sites
        .bind(py)
        .call_method1("at", (u32::from(offset),))?;
    Ok((!site.is_none()).then_some(site))
}

/// Escapes control characters as the core error's text does.
fn safe_text(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}
