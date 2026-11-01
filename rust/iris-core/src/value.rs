use std::fmt;

/// Owned typed leaves with cumulative row offsets, outermost level first.
#[derive(Debug, Clone)]
pub struct Value {
    pub(crate) leaves: LeafStorage,
    pub(crate) levels: Vec<ArrayLevel>,
}
#[derive(Debug, Clone)]
pub(crate) enum LeafStorage {
    Int(Vec<i64>),
    Float(Vec<f64>),
    String(Vec<String>),
}
#[derive(Debug, Clone)]
pub(crate) struct ArrayLevel {
    pub offsets: Vec<usize>,
}
#[derive(Debug, Clone, Copy)]
pub struct ValueRef<'a> {
    pub(crate) value: &'a Value,
    pub(crate) level: usize,
    pub(crate) row: usize,
}
#[derive(Debug, Clone)]
pub struct Entry {
    pub(crate) name: String,
    pub(crate) value: Value,
}
#[derive(Debug, Default, Clone)]
pub struct Data {
    pub(crate) entries: Vec<Entry>,
}
#[derive(Debug, Default)]
pub(crate) struct Frame {
    entries: Vec<Entry>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessError {
    UnknownEntry(String),
    IndexOutOfBounds,
    ExpectedArray,
    ExpectedInt,
    ExpectedFloat,
    ExpectedString,
    ExpectedScalar,
}
impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownEntry(name) => write!(f, "unknown entry '{name}'"),
            Self::IndexOutOfBounds => f.write_str("path index out of bounds"),
            Self::ExpectedArray => f.write_str("expected an array"),
            Self::ExpectedInt => f.write_str("expected integer leaves"),
            Self::ExpectedFloat => f.write_str("expected float leaves"),
            Self::ExpectedString => f.write_str("expected string leaves"),
            Self::ExpectedScalar => f.write_str("expected a scalar"),
        }
    }
}
impl std::error::Error for AccessError {}
impl Data {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn value(&self, name: &str, path: &[usize]) -> Result<ValueRef<'_>, AccessError> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| AccessError::UnknownEntry(name.into()))?
            .value
            .select(path)
    }
    pub fn int(&self, name: &str) -> Result<i64, AccessError> {
        self.value(name, &[])?.as_int()
    }
    pub fn ints(&self, name: &str, path: &[usize]) -> Result<&[i64], AccessError> {
        self.value(name, path)?.as_ints()
    }
}
impl Entry {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
}
impl Value {
    pub fn depth(&self) -> usize {
        self.levels.len()
    }
    pub fn value_type(&self) -> &'static str {
        match self.leaves {
            LeafStorage::Int(_) => "int",
            LeafStorage::Float(_) => "float",
            LeafStorage::String(_) => "string",
        }
    }
    pub fn select(&self, path: &[usize]) -> Result<ValueRef<'_>, AccessError> {
        let mut selected = ValueRef {
            value: self,
            level: 0,
            row: 0,
        };
        for &index in path {
            let range = selected.children()?;
            if index >= range.len() {
                return Err(AccessError::IndexOutOfBounds);
            }
            selected.row = range.start + index;
            selected.level += 1;
        }
        Ok(selected)
    }
    pub(crate) fn int(value: i64) -> Self {
        Self {
            leaves: LeafStorage::Int(vec![value]),
            levels: vec![],
        }
    }
    pub(crate) fn float(value: f64) -> Self {
        Self {
            leaves: LeafStorage::Float(vec![value]),
            levels: vec![],
        }
    }
    pub(crate) fn string(value: String) -> Self {
        Self {
            leaves: LeafStorage::String(vec![value]),
            levels: vec![],
        }
    }
    pub(crate) fn root(&self) -> ValueRef<'_> {
        ValueRef {
            value: self,
            level: 0,
            row: 0,
        }
    }
}
impl<'a> ValueRef<'a> {
    pub fn depth(self) -> usize {
        self.value.depth() - self.level
    }
    pub fn value_type(self) -> &'static str {
        self.value.value_type()
    }
    pub fn as_int(self) -> Result<i64, AccessError> {
        self.require_scalar()?;
        match &self.value.leaves {
            LeafStorage::Int(v) => Ok(v[self.row]),
            _ => Err(AccessError::ExpectedInt),
        }
    }
    pub fn as_float(self) -> Result<f64, AccessError> {
        self.require_scalar()?;
        match &self.value.leaves {
            LeafStorage::Float(v) => Ok(v[self.row]),
            _ => Err(AccessError::ExpectedFloat),
        }
    }
    pub fn as_str(self) -> Result<&'a str, AccessError> {
        self.require_scalar()?;
        match &self.value.leaves {
            LeafStorage::String(v) => Ok(&v[self.row]),
            _ => Err(AccessError::ExpectedString),
        }
    }
    pub fn as_ints(self) -> Result<&'a [i64], AccessError> {
        if self.depth() != 1 {
            return Err(AccessError::ExpectedArray);
        }
        let range = self.children()?;
        match &self.value.leaves {
            LeafStorage::Int(v) => Ok(&v[range]),
            _ => Err(AccessError::ExpectedInt),
        }
    }
    pub(crate) fn children(self) -> Result<std::ops::Range<usize>, AccessError> {
        let level = self
            .value
            .levels
            .get(self.level)
            .ok_or(AccessError::ExpectedArray)?;
        Ok(level.offsets[self.row]..level.offsets[self.row + 1])
    }
    fn require_scalar(self) -> Result<(), AccessError> {
        if self.depth() == 0 {
            Ok(())
        } else {
            Err(AccessError::ExpectedScalar)
        }
    }
}
impl Frame {
    pub fn set(&mut self, name: String, value: Value) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.name == name) {
            entry.value = value;
        } else {
            self.entries.push(Entry { name, value });
        }
    }
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| &e.value)
    }
    pub fn into_data(self) -> Data {
        Data {
            entries: self.entries,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn borrowed_ragged_rows_and_empty_depth() {
        let data = Data {
            entries: vec![
                Entry {
                    name: "rows".into(),
                    value: Value {
                        leaves: LeafStorage::Int(vec![1, 2, 3]),
                        levels: vec![
                            ArrayLevel {
                                offsets: vec![0, 3],
                            },
                            ArrayLevel {
                                offsets: vec![0, 2, 2, 3],
                            },
                        ],
                    },
                },
                Entry {
                    name: "empty".into(),
                    value: Value {
                        leaves: LeafStorage::Float(vec![]),
                        levels: vec![
                            ArrayLevel {
                                offsets: vec![0, 0],
                            },
                            ArrayLevel { offsets: vec![0] },
                        ],
                    },
                },
            ],
        };
        assert_eq!(data.ints("rows", &[0]).unwrap(), &[1, 2]);
        assert!(data.ints("rows", &[1]).unwrap().is_empty());
        assert_eq!(data.value("rows", &[2, 0]).unwrap().as_int().unwrap(), 3);
        let LeafStorage::Int(leaves) = &data.entries[0].value.leaves else {
            unreachable!()
        };
        assert_eq!(data.ints("rows", &[0]).unwrap().as_ptr(), leaves.as_ptr());
        assert_eq!(data.value("empty", &[]).unwrap().depth(), 2);
        assert_eq!(data.value("empty", &[]).unwrap().value_type(), "float");
        assert!(matches!(
            data.value("empty", &[0]),
            Err(AccessError::IndexOutOfBounds)
        ));
        assert_eq!(data.int("rows"), Err(AccessError::ExpectedScalar));
        assert!(matches!(
            data.int("missing"),
            Err(AccessError::UnknownEntry(_))
        ));
    }
}
