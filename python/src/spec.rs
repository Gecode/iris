use crate::{data::PyData, error};
use gecode_iris::{Data, Failure, Params, Spec};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    pybacked::{PyBackedBytes, PyBackedStr},
    types::{PyMapping, PyString},
};
use std::path::{Path, PathBuf};

/// A parsed and checked reader spec.
///
/// `gecode_iris::Spec` holds syntax nodes that must stay on one thread, so a
/// `PySpec` keeps the source bytes instead and parses them again for each
/// operation, on the calling thread. Parsing takes microseconds, and one
/// `PySpec` can then be shared between Python threads.
#[pyclass(name = "Spec", module = "gecode_iris", frozen)]
pub struct PySpec {
    source: Box<[u8]>,
    origin: String,
}

/// Spec text: a `str` is encoded as UTF-8, and `bytes` are used as given, so
/// invalid UTF-8 reports `E-SPEC-UTF8`.
#[derive(FromPyObject)]
enum SpecText {
    Text(PyBackedStr),
    Bytes(PyBackedBytes),
}

/// The `source` argument of `read` and `catalog`: input bytes, or a path given
/// as `str` or `os.PathLike`. A `str` is never input text.
#[derive(FromPyObject)]
enum SourceArgument {
    Bytes(PyBackedBytes),
    Path(PathBuf),
}

/// Input as the reader takes it.
enum Source {
    Bytes {
        bytes: PyBackedBytes,
        origin: String,
    },
    Path(PathBuf),
}

#[derive(Clone, Copy)]
enum Operation {
    Read,
    Catalog,
}

#[pymethods]
impl PySpec {
    /// Parses and checks a reader spec given as `str` or `bytes`.
    #[staticmethod]
    #[pyo3(signature = (text, origin = "<string>".to_owned()))]
    fn parse(py: Python<'_>, text: SpecText, origin: String) -> PyResult<Self> {
        let source = match &text {
            SpecText::Text(text) => text.as_bytes(),
            SpecText::Bytes(bytes) => bytes.as_ref(),
        };
        Self::checked(py, source.into(), origin)
    }

    /// Reads and parses a reader spec file; the path is the error origin.
    #[staticmethod]
    fn load(py: Python<'_>, path: PathBuf) -> PyResult<Self> {
        let source = py
            .detach(|| std::fs::read(&path))
            .map_err(|e| error::io_error(py, e, Some(&path)))?;
        Self::checked(py, source.into(), path.to_string_lossy().into_owned())
    }

    /// Reads one instance from input bytes or from a path.
    #[pyo3(signature = (source, params = None, *, origin = None))]
    fn read(
        &self,
        py: Python<'_>,
        source: SourceArgument,
        params: Option<&Bound<'_, PyAny>>,
        origin: Option<String>,
    ) -> PyResult<PyData> {
        self.run(py, Operation::Read, source, params, origin)
    }

    /// Lists the instances in the input, with the same arguments as `read`.
    #[pyo3(signature = (source, params = None, *, origin = None))]
    fn catalog(
        &self,
        py: Python<'_>,
        source: SourceArgument,
        params: Option<&Bound<'_, PyAny>>,
        origin: Option<String>,
    ) -> PyResult<PyData> {
        self.run(py, Operation::Catalog, source, params, origin)
    }
}

impl PySpec {
    /// Parses once so that errors surface now; operations parse again.
    fn checked(py: Python<'_>, source: Box<[u8]>, origin: String) -> PyResult<Self> {
        // The parsed spec cannot leave this thread, so it is dropped here.
        py.detach(|| Spec::parse(&source, &origin).map(drop))
            .map_err(|failure| error::failure_to_err(py, failure, None))?;
        Ok(Self { source, origin })
    }

    fn run(
        &self,
        py: Python<'_>,
        operation: Operation,
        source: SourceArgument,
        params: Option<&Bound<'_, PyAny>>,
        origin: Option<String>,
    ) -> PyResult<PyData> {
        let source = Source::new(source, origin)?;
        let params = params
            .map(params_from_mapping)
            .transpose()?
            .unwrap_or_default();
        py.detach(|| {
            let spec = Spec::parse(&self.source, &self.origin)?;
            operation.apply(&spec, &source, &params)
        })
        .map(PyData::from)
        .map_err(|failure| error::failure_to_err(py, failure, source.path()))
    }
}

impl Source {
    fn new(argument: SourceArgument, origin: Option<String>) -> PyResult<Self> {
        match argument {
            SourceArgument::Bytes(bytes) => Ok(Self::Bytes {
                bytes,
                origin: origin.unwrap_or_else(|| "<input>".to_owned()),
            }),
            SourceArgument::Path(path) if origin.is_none() => Ok(Self::Path(path)),
            SourceArgument::Path(_) => Err(PyTypeError::new_err(
                "origin applies only to bytes input; a path is its own origin",
            )),
        }
    }

    fn path(&self) -> Option<&Path> {
        match self {
            Self::Bytes { .. } => None,
            Self::Path(path) => Some(path),
        }
    }
}

impl Operation {
    fn apply(self, spec: &Spec, source: &Source, params: &Params) -> Result<Data, Failure> {
        match (self, source) {
            (Self::Read, Source::Bytes { bytes, origin }) => spec.read(bytes, origin, params),
            (Self::Read, Source::Path(path)) => spec.read_path(path, params),
            (Self::Catalog, Source::Bytes { bytes, origin }) => spec.catalog(bytes, origin, params),
            (Self::Catalog, Source::Path(path)) => spec.catalog_path(path, params),
        }
    }
}

/// Parameter values are strings (Specification §9.1), so keys and values must
/// be `str`; nothing is converted with `str()`.
fn params_from_mapping(mapping: &Bound<'_, PyAny>) -> PyResult<Params> {
    let mapping = mapping
        .cast::<PyMapping>()
        .map_err(|_| PyTypeError::new_err("params must be a mapping of str to str"))?;
    let mut params = Params::new();
    for item in mapping.items()?.iter() {
        let (name, value): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item.extract()?;
        let name = name.cast::<PyString>().map_err(|_| {
            PyTypeError::new_err(format!(
                "parameter names must be str, not {}",
                type_name(&name)
            ))
        })?;
        let value = value.cast::<PyString>().map_err(|_| {
            PyTypeError::new_err(format!(
                "parameter '{name}' must be a str, not {}",
                type_name(&value)
            ))
        })?;
        params.set(name.to_str()?, value.to_str()?);
    }
    Ok(params)
}

fn type_name(object: &Bound<'_, PyAny>) -> String {
    object
        .get_type()
        .name()
        .map_or_else(|_| "an unknown type".to_owned(), |name| name.to_string())
}
