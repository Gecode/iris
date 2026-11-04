use gecode_iris::{AccessError, Data, Value, ValueRef};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyImportError, PyIndexError, PyKeyError, PyRuntimeError, PyTypeError},
    prelude::*,
    sync::PyOnceLock,
    types::{PyByteArray, PyDict, PyList, PyTuple, PyType},
};

/// The data from one read or catalog.
///
/// Values are copied into Python objects on each access: scalars become
/// `int`, `float` or `str`, and arrays become nested lists.
#[pyclass(name = "Data", module = "gecode_iris", frozen)]
pub struct PyData(Data);

impl From<Data> for PyData {
    fn from(data: Data) -> Self {
        Self(data)
    }
}

#[pymethods]
impl PyData {
    /// The entries, as `gecode_iris.Entry` tuples, in the order their names
    /// were defined.
    #[getter]
    fn entries<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        static ENTRY_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();
        let entry_type = ENTRY_TYPE.import(py, "gecode_iris._data", "Entry")?;
        let entries = self.0.entries().iter().map(|entry| {
            let value = entry.value();
            entry_type.call1((entry.name(), value.value_type(), value.depth()))
        });
        PyTuple::new(py, entries.collect::<PyResult<Vec<_>>>()?)
    }

    /// The whole value of an entry.
    fn __getitem__<'py>(&self, py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
        to_python(py, self.value(name)?, &mut Vec::new())
    }

    fn __contains__(&self, name: &Bound<'_, PyAny>) -> bool {
        name.extract::<&str>()
            .is_ok_and(|name| self.0.entries().iter().any(|e| e.name() == name))
    }

    /// The integer row of an entry at a path.
    #[pyo3(signature = (name, /, *indices))]
    fn ints<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        indices: Vec<isize>,
    ) -> PyResult<Bound<'py, PyList>> {
        let row = self.row(name, &indices)?;
        let ints = match row.depth() {
            1 => row.as_ints(),
            0 => Err(AccessError::ExpectedArray),
            depth => {
                return Err(PyTypeError::new_err(format!(
                    "{}: expected a row of integers, but the selection has depth {depth}",
                    location(name, &indices),
                )));
            }
        };
        PyList::new(py, ints.map_err(|e| access_error(name, &indices, e))?)
    }

    /// An array of an entry, or of its row at a path: a `numpy.ndarray` when
    /// numpy is importable and the array is rectangular, and nested lists
    /// otherwise.
    #[pyo3(signature = (name, /, *indices))]
    fn array<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        indices: Vec<isize>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value = self.value(name)?;
        let mut path = resolve(value, &indices).map_err(|e| access_error(name, &indices, e))?;
        if value.select(&path).map_err(internal_error)?.depth() == 0 {
            return Err(access_error(name, &indices, AccessError::ExpectedArray));
        }
        if let Some(numpy) = import_numpy(py)?
            && let Some(rectangle) = rectangle(value, &path).map_err(internal_error)?
        {
            return rectangle.into_ndarray(&numpy);
        }
        to_python(py, value, &mut path)
    }

    /// All entries as a `dict` from name to value, in entry order.
    fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for entry in self.0.entries() {
            dict.set_item(entry.name(), to_python(py, entry.value(), &mut Vec::new())?)?;
        }
        Ok(dict)
    }

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

    fn __repr__(&self) -> String {
        let names: Vec<&str> = self.0.entries().iter().map(|e| e.name()).collect();
        format!("<gecode_iris.Data: {}>", names.join(", "))
    }
}

impl PyData {
    fn value(&self, name: &str) -> PyResult<&Value> {
        self.0
            .entries()
            .iter()
            .find(|entry| entry.name() == name)
            .map(|entry| entry.value())
            .ok_or_else(|| PyKeyError::new_err(name.to_owned()))
    }

    fn row(&self, name: &str, indices: &[isize]) -> PyResult<ValueRef<'_>> {
        let value = self.value(name)?;
        resolve(value, indices)
            .and_then(|path| value.select(&path))
            .map_err(|e| access_error(name, indices, e))
    }
}

/// Turns Python indices, which may count from the end, into a path.
fn resolve(value: &Value, indices: &[isize]) -> Result<Vec<usize>, AccessError> {
    let mut path = Vec::with_capacity(indices.len());
    for &index in indices {
        let size = value.select(&path)?.size()?;
        let resolved = if index < 0 {
            size.checked_add_signed(index)
        } else {
            Some(index.unsigned_abs())
        };
        path.push(
            resolved
                .filter(|&i| i < size)
                .ok_or(AccessError::IndexOutOfBounds)?,
        );
    }
    Ok(path)
}

/// Copies the value at `path` into a scalar or nested lists.
fn to_python<'py>(
    py: Python<'py>,
    value: &Value,
    path: &mut Vec<usize>,
) -> PyResult<Bound<'py, PyAny>> {
    let row = value.select(path).map_err(internal_error)?;
    if row.depth() == 0 {
        return match row.value_type() {
            "int" => row.as_int().map_err(internal_error)?.into_bound_py_any(py),
            "float" => row
                .as_float()
                .map_err(internal_error)?
                .into_bound_py_any(py),
            _ => row.as_str().map_err(internal_error)?.into_bound_py_any(py),
        };
    }
    if row.depth() == 1
        && let Ok(ints) = row.as_ints()
    {
        return PyList::new(py, ints).map(Bound::into_any);
    }
    let list = PyList::empty(py);
    for index in 0..row.size().map_err(internal_error)? {
        path.push(index);
        list.append(to_python(py, value, path)?)?;
        path.pop();
    }
    Ok(list.into_any())
}

/// numpy, or `None` when it cannot be imported.
fn import_numpy(py: Python<'_>) -> PyResult<Option<Bound<'_, PyModule>>> {
    match py.import("numpy") {
        Ok(numpy) => Ok(Some(numpy)),
        Err(error) if error.is_instance_of::<PyImportError>(py) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The leaves of a rectangular array in row-major order, with its shape.
struct Rectangle<'a> {
    shape: Vec<usize>,
    leaves: Leaves<'a>,
}

enum Leaves<'a> {
    Int(Vec<i64>),
    Float(Vec<f64>),
    String(Vec<&'a str>),
}

/// The array at `path` as a rectangle, or `None` when it is ragged.
fn rectangle<'a>(value: &'a Value, path: &[usize]) -> Result<Option<Rectangle<'a>>, AccessError> {
    // The first row at each level gives the shape; the walk checks the others.
    let depth = value.select(path)?.depth();
    let mut shape = Vec::with_capacity(depth);
    let mut first = path.to_vec();
    while shape.len() < depth {
        let size = value.select(&first)?.size()?;
        shape.push(size);
        if size == 0 {
            shape.resize(depth, 0);
        }
        first.push(0);
    }
    let mut leaves = match value.value_type() {
        "int" => Leaves::Int(Vec::new()),
        "float" => Leaves::Float(Vec::new()),
        _ => Leaves::String(Vec::new()),
    };
    let rectangular = collect(value, &mut path.to_vec(), &shape, &mut leaves)?;
    Ok(rectangular.then_some(Rectangle { shape, leaves }))
}

/// Appends the leaves under `path` if each row's size matches `shape`.
fn collect<'a>(
    value: &'a Value,
    path: &mut Vec<usize>,
    shape: &[usize],
    leaves: &mut Leaves<'a>,
) -> Result<bool, AccessError> {
    let row = value.select(path)?;
    let Some((&width, inner)) = shape.split_first() else {
        match leaves {
            Leaves::Int(v) => v.push(row.as_int()?),
            Leaves::Float(v) => v.push(row.as_float()?),
            Leaves::String(v) => v.push(row.as_str()?),
        }
        return Ok(true);
    };
    if row.size()? != width {
        return Ok(false);
    }
    if let (Leaves::Int(v), []) = (&mut *leaves, inner) {
        v.extend_from_slice(row.as_ints()?);
        return Ok(true);
    }
    for index in 0..width {
        path.push(index);
        let rectangular = collect(value, path, inner, leaves)?;
        path.pop();
        if !rectangular {
            return Ok(false);
        }
    }
    Ok(true)
}

impl Rectangle<'_> {
    fn into_ndarray<'py>(self, numpy: &Bound<'py, PyModule>) -> PyResult<Bound<'py, PyAny>> {
        let py = numpy.py();
        let flat = match self.leaves {
            Leaves::Int(v) => numeric_array(numpy, "int64", &v, i64::to_ne_bytes)?,
            Leaves::Float(v) => numeric_array(numpy, "float64", &v, f64::to_ne_bytes)?,
            Leaves::String(v) => {
                numpy.call_method1("array", (PyList::new(py, v)?, numpy.getattr("str_")?))?
            }
        };
        flat.call_method1("reshape", (PyTuple::new(py, self.shape)?,))
    }
}

/// A flat, writable ndarray over a copy of `values` in a `bytearray`.
fn numeric_array<'py, T: Copy, const N: usize>(
    numpy: &Bound<'py, PyModule>,
    dtype: &str,
    values: &[T],
    to_bytes: fn(T) -> [u8; N],
) -> PyResult<Bound<'py, PyAny>> {
    let buffer = PyByteArray::new_with(numpy.py(), values.len() * N, |bytes| {
        for (chunk, &value) in bytes.as_chunks_mut::<N>().0.iter_mut().zip(values) {
            *chunk = to_bytes(value);
        }
        Ok(())
    })?;
    numpy.call_method1("frombuffer", (buffer, numpy.getattr(dtype)?))
}

/// Builds the exception for an invalid name or path given by the caller.
fn access_error(name: &str, indices: &[isize], error: AccessError) -> PyErr {
    let message = format!("{}: {error}", location(name, indices));
    match error {
        AccessError::UnknownEntry(_) => PyKeyError::new_err(name.to_owned()),
        AccessError::IndexOutOfBounds => PyIndexError::new_err(message),
        _ => PyTypeError::new_err(message),
    }
}

fn location(name: &str, indices: &[isize]) -> String {
    if indices.is_empty() {
        format!("entry '{name}'")
    } else {
        let indices: Vec<String> = indices.iter().map(isize::to_string).collect();
        format!("entry '{name}' at path [{}]", indices.join(", "))
    }
}

/// Reports a failed access that the data's own structure rules out.
fn internal_error(error: AccessError) -> PyErr {
    PyRuntimeError::new_err(format!("inconsistent data: {error}"))
}

fn rendering_error(error: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(format!("cannot render data: {error}"))
}
