use crate::cancellation::{Cancelled, Control, VALUE_CHUNK};
use std::{collections::HashMap, fmt};

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
/// Spec-local identity. Text is shared for output and checked diagnostics only.
#[derive(Debug, Clone)]
pub(crate) struct Name {
    pub id: NameId,
    text: std::sync::Arc<str>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NameId(pub usize);
impl NameId {
    pub const NAME: Self = Self(0);
    pub const INSTANCES: Self = Self(1);
    pub const FAILED: Self = Self(2);
    pub const INSTANCE: Self = Self(3);
}
impl Name {
    pub fn new(id: usize, text: &str) -> Self {
        Self {
            id: NameId(id),
            text: text.into(),
        }
    }
}
impl std::ops::Deref for Name {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}
#[derive(Debug, Default)]
pub(crate) struct Frame {
    entries: Vec<FrameEntry>,
    names: HashMap<NameId, usize>,
}
#[derive(Debug)]
struct FrameEntry {
    name: Name,
    value: Option<Value>,
    buffer: Option<Value>,
}
/// An invalid entry lookup, path, or requested value type in successfully read data.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
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
    pub(crate) fn empty(value_type: &str) -> Self {
        Self {
            leaves: match value_type {
                "float" => LeafStorage::Float(vec![]),
                "string" => LeafStorage::String(vec![]),
                _ => LeafStorage::Int(vec![]),
            },
            levels: vec![],
        }
    }
    pub(crate) fn leaf_count(&self) -> usize {
        match &self.leaves {
            LeafStorage::Int(v) => v.len(),
            LeafStorage::Float(v) => v.len(),
            LeafStorage::String(v) => v.len(),
        }
    }
    #[cfg(test)]
    pub(crate) fn append(&mut self, scalar: Value) -> Result<(), crate::ErrorCode> {
        self.append_controlled(scalar, &Control::default())
            .map_err(|e| match e {
                BuildError::Type(code) => code,
                BuildError::Host(_) => unreachable!(),
            })
    }
    #[inline]
    pub(crate) fn append_controlled(
        &mut self,
        scalar: Value,
        control: &Control,
    ) -> Result<(), BuildError> {
        if control.is_active() {
            self.append_with::<true>(scalar, control)
        } else {
            self.append_with::<false>(scalar, control)
        }
    }
    #[inline]
    fn append_with<const CANCEL: bool>(
        &mut self,
        scalar: Value,
        control: &Control,
    ) -> Result<(), BuildError> {
        match (&mut self.leaves, scalar.leaves) {
            (LeafStorage::Int(a), LeafStorage::Int(b)) => {
                append_leaves::<CANCEL, _>(a, b, control)?
            }
            (LeafStorage::Float(a), LeafStorage::Float(b)) => {
                append_leaves::<CANCEL, _>(a, b, control)?
            }
            (LeafStorage::String(a), LeafStorage::String(b)) => {
                append_leaves::<CANCEL, _>(a, b, control)?
            }
            (LeafStorage::Float(a), LeafStorage::Int(b)) => {
                if !CANCEL {
                    a.extend(b.into_iter().map(|n| n as f64));
                } else {
                    for values in b.chunks(VALUE_CHUNK) {
                        if CANCEL {
                            control.check()?;
                        }
                        a.extend(values.iter().map(|&n| n as f64));
                    }
                }
            }
            (LeafStorage::Int(a), LeafStorage::Float(b)) => {
                let mut promoted = Vec::with_capacity(a.len());
                let chunk = if CANCEL { VALUE_CHUNK } else { usize::MAX };
                for values in a.chunks(chunk) {
                    if CANCEL {
                        control.check()?;
                    }
                    promoted.extend(values.iter().map(|&n| n as f64));
                }
                append_leaves::<CANCEL, _>(&mut promoted, b, control)?;
                self.leaves = LeafStorage::Float(promoted);
            }
            _ => return Err(crate::ErrorCode::ValueTypeMix.into()),
        }
        Ok(())
    }
    /// Attach rectangular structure only after all input leaves have been read.
    #[cfg(test)]
    pub(crate) fn set_dimensions(&mut self, dimensions: &[usize]) {
        self.set_dimensions_controlled(dimensions, &Control::default())
            .unwrap();
    }
    pub(crate) fn set_dimensions_controlled(
        &mut self,
        dimensions: &[usize],
        control: &Control,
    ) -> Result<(), Cancelled> {
        let mut rows: usize = 1;
        self.levels.clear();
        for &width in dimensions {
            control.check()?;
            let mut offsets = Vec::with_capacity(rows + 1);
            let chunk = if control.is_active() {
                VALUE_CHUNK
            } else {
                usize::MAX
            };
            let mut start: usize = 0;
            while start <= rows {
                control.check()?;
                let end = start.saturating_add(chunk).min(rows + 1);
                control.worked(end - start);
                offsets.extend((start..end).map(|i| i * width));
                start = end;
            }
            rows *= width;
            self.levels.push(ArrayLevel { offsets });
        }
        Ok(())
    }
    pub(crate) fn set_list_dimension(&mut self, control: &Control) -> Result<(), Cancelled> {
        control.check()?;
        control.check()?;
        control.worked(2);
        let size = self.leaf_count();
        if self.levels.len() == 1 {
            self.levels[0].offsets.clear();
            self.levels[0].offsets.extend([0, size]);
        } else {
            self.levels = vec![ArrayLevel {
                offsets: vec![0, size],
            }];
        }
        Ok(())
    }
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
    #[cfg(test)]
    pub(crate) fn to_owned(self) -> Value {
        self.copy_with(&Control::default(), |_| {}).unwrap()
    }
    pub(crate) fn copy_with(
        self,
        control: &Control,
        mut copied: impl FnMut(usize),
    ) -> Result<Value, Cancelled> {
        let mut rows = self.row..self.row + 1;
        let mut levels = vec![];
        for level in &self.value.levels[self.level..] {
            let start = level.offsets[rows.start];
            let end = level.offsets[rows.end];
            copied(rows.len() + 1);
            let mut offsets = Vec::with_capacity(rows.len() + 1);
            let chunk = if control.is_active() {
                VALUE_CHUNK
            } else {
                usize::MAX
            };
            for values in level.offsets[rows.start..=rows.end].chunks(chunk) {
                control.check()?;
                offsets.extend(values.iter().map(|n| n - start));
            }
            levels.push(ArrayLevel { offsets });
            rows = start..end;
        }
        copied(rows.len());
        let leaves = match &self.value.leaves {
            LeafStorage::Int(v) => LeafStorage::Int(clone_leaves(&v[rows], control)?),
            LeafStorage::Float(v) => LeafStorage::Float(clone_leaves(&v[rows], control)?),
            LeafStorage::String(v) => LeafStorage::String({
                let mut out = Vec::with_capacity(rows.len());
                for text in &v[rows] {
                    control.check()?;
                    out.push(control.copy_text(text)?);
                }
                out
            }),
        };
        Ok(Value { leaves, levels })
    }
    pub fn depth(self) -> usize {
        self.value.depth() - self.level
    }
    pub fn value_type(self) -> &'static str {
        self.value.value_type()
    }
    /// Number of elements in this array row; scalars return `ExpectedArray`.
    /// This reads stored offsets in constant time, independent of depth.
    pub fn size(self) -> Result<usize, AccessError> {
        Ok(self.children()?.len())
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
/// Stable while a frame retains its entries, including across clear/reuse.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EntryId(usize);

impl Frame {
    pub fn set(&mut self, name: Name, value: Value) {
        self.set_controlled(name, value, &Control::default())
            .expect("uncancelled frame");
    }
    pub fn set_controlled(
        &mut self,
        name: Name,
        value: Value,
        control: &Control,
    ) -> Result<(), Cancelled> {
        self.set_indexed(name, value, control).map(|_| ())
    }
    pub fn set_indexed(
        &mut self,
        name: Name,
        value: Value,
        control: &Control,
    ) -> Result<EntryId, Cancelled> {
        control.check()?;
        let index = if let Some(&index) = self.names.get(&name.id) {
            self.entries[index].value = Some(value);
            index
        } else {
            let index = self.entries.len();
            self.names.insert(name.id, index);
            self.entries.push(FrameEntry {
                name,
                value: Some(value),
                buffer: None,
            });
            index
        };
        Ok(EntryId(index))
    }
    pub fn entry(&self, name: NameId) -> Option<EntryId> {
        self.names.get(&name).copied().map(EntryId)
    }
    pub fn value_at(&self, entry: EntryId) -> Option<&Value> {
        self.entries.get(entry.0)?.value.as_ref()
    }
    pub fn set_at(
        &mut self,
        entry: EntryId,
        value: Value,
        control: &Control,
    ) -> Result<(), Cancelled> {
        control.check()?;
        self.entries
            .get_mut(entry.0)
            .expect("entry belongs to reused frame")
            .value = Some(value);
        Ok(())
    }
    pub fn take_at(&mut self, entry: EntryId) -> Option<Value> {
        self.entries.get_mut(entry.0)?.value.take()
    }
    pub fn take_buffer_at(&mut self, entry: EntryId) -> Option<Value> {
        self.entries.get_mut(entry.0)?.buffer.take()
    }
    pub fn recycle_at(&mut self, entry: EntryId, buffer: Option<Value>) {
        self.entries
            .get_mut(entry.0)
            .expect("collected entry belongs to frame")
            .buffer = buffer;
    }
    pub fn lookup(&self, name: NameId) -> Option<&Value> {
        self.entries.get(*self.names.get(&name)?)?.value.as_ref()
    }
    pub fn take(&mut self, name: NameId) -> Option<Value> {
        self.entries.get_mut(*self.names.get(&name)?)?.value.take()
    }
    pub fn into_data(self, control: &Control) -> Result<Data, Cancelled> {
        Ok(Data {
            entries: self
                .entries
                .into_iter()
                .map(|entry| {
                    Ok(Entry {
                        name: control.copy_text(&entry.name)?,
                        value: entry
                            .value
                            .expect("published frame values have not been collected"),
                    })
                })
                .collect::<Result<_, Cancelled>>()?,
        })
    }
    pub fn extend(&mut self, other: Frame, control: &Control) -> Result<(), Cancelled> {
        for entry in other.entries {
            control.check()?;
            self.set_controlled(entry.name, entry.value.expect("published value"), control)?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn clear(&mut self, control: &Control) -> Result<(), Cancelled> {
        for (index, entry) in self.entries.iter_mut().enumerate() {
            if index.is_multiple_of(VALUE_CHUNK) {
                control.check()?;
            }
            control.worked(1);
            entry.value = None;
        }
        Ok(())
    }
    pub fn take_buffer(&mut self, name: NameId) -> Option<Value> {
        self.entries.get_mut(*self.names.get(&name)?)?.buffer.take()
    }
    pub fn recycle(&mut self, name: NameId, buffer: Option<Value>) {
        self.entries[*self.names.get(&name).expect("collected field")].buffer = buffer;
    }
}

/// Append moved local rows without rebuilding previous rows or nesting Values.
pub(crate) struct Column {
    fallback_type: &'static str,
    fallback_depth: usize,
    rows: usize,
    value: Option<Value>,
}
impl Column {
    pub fn new(fallback_type: &'static str, fallback_depth: usize) -> Self {
        Self {
            fallback_type,
            fallback_depth,
            rows: 0,
            value: None,
        }
    }
    #[cfg(test)]
    pub fn add_row(&mut self, row: Option<Value>) -> Result<(), crate::ErrorCode> {
        self.add_row_controlled(row, &Control::default())
            .map_err(|e| match e {
                BuildError::Type(c) => c,
                _ => unreachable!(),
            })
    }
    #[cfg(test)]
    pub fn add_row_controlled(
        &mut self,
        row: Option<Value>,
        control: &Control,
    ) -> Result<(), BuildError> {
        self.add_row_reusing(row, control).map(|_| ())
    }
    pub fn add_row_reusing(
        &mut self,
        row: Option<Value>,
        control: &Control,
    ) -> Result<Option<Value>, BuildError> {
        use crate::ErrorCode;
        let Some(mut row) = row else {
            if let Some(value) = &mut self.value {
                if value.depth() == 1 {
                    return Err(ErrorCode::RepeatMissing.into());
                }
                let offsets = &mut value.levels[1].offsets;
                offsets.push(*offsets.last().expect("initial offset"));
            }
            self.rows += 1;
            return Ok(None);
        };
        if let Some(value) = &self.value {
            if value.depth() != row.depth() + 1 {
                return Err(ErrorCode::RepeatDepth.into());
            }
        } else {
            if row.depth() == 0 && self.rows > 0 {
                return Err(ErrorCode::RepeatMissing.into());
            }
            self.value = Some(Self::empty_rows(
                row.value_type(),
                row.depth(),
                self.rows,
                control,
            )?);
        }
        let value = self
            .value
            .as_mut()
            .expect("first actual row initialized column");
        for (destination, source) in value.levels[1..].iter_mut().zip(&row.levels) {
            let base = *destination.offsets.last().expect("initial offset");
            extend_offsets(
                &mut destination.offsets,
                &source.offsets[1..],
                base,
                control,
            )?;
        }
        // Empty rows only choose a provisional type. The first nonempty row
        // replaces it; subsequent nonempty rows use ordinary numeric promotion.
        if row.leaf_count() > 0 {
            if value.leaf_count() == 0 {
                value.leaves = std::mem::replace(&mut row.leaves, LeafStorage::Int(Vec::new()));
            } else {
                if let (LeafStorage::Int(destination), LeafStorage::Int(source)) =
                    (&mut value.leaves, &mut row.leaves)
                {
                    for chunk in source.chunks(if control.is_active() {
                        VALUE_CHUNK
                    } else {
                        usize::MAX
                    }) {
                        control.check()?;
                        control.worked(chunk.len());
                        destination.extend_from_slice(chunk);
                    }
                    source.clear();
                } else {
                    value.append_controlled(row, control)?;
                    self.rows += 1;
                    return Ok(None);
                }
            }
        }
        self.rows += 1;
        Ok(Some(row))
    }
    #[cfg(test)]
    pub fn finish(self) -> Result<Value, crate::ErrorCode> {
        self.finish_controlled(&Control::default())
            .map_err(|e| match e {
                BuildError::Type(c) => c,
                _ => unreachable!(),
            })
    }
    pub fn finish_controlled(self, control: &Control) -> Result<Value, BuildError> {
        let mut value = if let Some(value) = self.value {
            value
        } else {
            if self.rows > 0 && self.fallback_depth == 0 {
                return Err(crate::ErrorCode::RepeatMissing.into());
            }
            Self::empty_rows(self.fallback_type, self.fallback_depth, self.rows, control)?
        };
        value.levels[0].offsets[1] = self.rows;
        Ok(value)
    }
    fn empty_rows(
        value_type: &str,
        local_depth: usize,
        rows: usize,
        control: &Control,
    ) -> Result<Value, Cancelled> {
        let mut value = Value::empty(value_type);
        value.levels.push(ArrayLevel {
            offsets: vec![0, rows],
        });
        for level in 0..local_depth {
            control.check()?;
            let length = if level == 0 { rows + 1 } else { 1 };
            let mut offsets = Vec::with_capacity(length);
            while offsets.len() < length {
                control.check()?;
                let end = if control.is_active() {
                    offsets.len().saturating_add(VALUE_CHUNK).min(length)
                } else {
                    length
                };
                offsets.resize(end, 0);
            }
            value.levels.push(ArrayLevel { offsets });
        }
        Ok(value)
    }
}

pub(crate) enum BuildError {
    Type(crate::ErrorCode),
    Host(Cancelled),
}
impl From<Cancelled> for BuildError {
    fn from(e: Cancelled) -> Self {
        Self::Host(e)
    }
}
impl From<crate::ErrorCode> for BuildError {
    fn from(e: crate::ErrorCode) -> Self {
        Self::Type(e)
    }
}
fn extend_offsets(
    out: &mut Vec<usize>,
    values: &[usize],
    base: usize,
    control: &Control,
) -> Result<(), Cancelled> {
    let chunk = if control.is_active() {
        VALUE_CHUNK
    } else {
        usize::MAX
    };
    for values in values.chunks(chunk) {
        control.check()?;
        out.extend(values.iter().map(|n| base + n));
    }
    Ok(())
}
fn clone_leaves<T: Clone>(values: &[T], control: &Control) -> Result<Vec<T>, Cancelled> {
    if !control.is_active() {
        return Ok(values.to_vec());
    }
    let mut out = Vec::with_capacity(values.len());
    for chunk in values.chunks(VALUE_CHUNK) {
        control.check()?;
        out.extend_from_slice(chunk);
    }
    Ok(out)
}

#[inline]
fn append_leaves<const CANCEL: bool, T>(
    out: &mut Vec<T>,
    values: Vec<T>,
    control: &Control,
) -> Result<(), Cancelled> {
    if !CANCEL {
        out.extend(values);
        return Ok(());
    }
    let mut values = values.into_iter();
    while values.len() > 0 {
        control.check()?;
        out.extend(values.by_ref().take(VALUE_CHUNK));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn borrowed_ragged_rows_and_empty_depth() {
        let mut rows = Column::new("float", 0);
        for integers in [vec![1, 2], vec![], vec![3]] {
            let mut row = Value {
                leaves: LeafStorage::Int(integers),
                levels: vec![],
            };
            row.set_dimensions(&[row.leaf_count()]);
            rows.add_row(Some(row)).unwrap();
        }
        let empty = Column::new("float", 1);
        let mut nested = Column::new("int", 0);
        nested.add_row(None).unwrap();
        let mut actual = Value::float(2.5);
        actual.set_dimensions(&[1, 1]);
        nested.add_row(Some(actual)).unwrap();
        nested.add_row(None).unwrap();
        let mut data = Data {
            entries: vec![
                Entry {
                    name: "rows".into(),
                    value: rows.finish().unwrap(),
                },
                Entry {
                    name: "empty".into(),
                    value: empty.finish().unwrap(),
                },
                Entry {
                    name: "nested".into(),
                    value: nested.finish().unwrap(),
                },
            ],
        };
        assert_eq!(data.value("nested", &[]).unwrap().depth(), 3);
        assert_eq!(
            data.value("nested", &[1, 0, 0])
                .unwrap()
                .as_float()
                .unwrap(),
            2.5
        );
        assert_eq!(
            data.value("nested", &[0])
                .unwrap()
                .children()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            data.value("nested", &[2])
                .unwrap()
                .children()
                .unwrap()
                .len(),
            0
        );
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
        let copy = data.value("rows", &[2]).unwrap().to_owned();
        assert_eq!(copy.depth(), 1);
        assert_eq!(copy.levels[0].offsets, [0, 1]);
        assert_eq!(copy.root().as_ints().unwrap(), &[3]);
        let empty = data.value("empty", &[]).unwrap().to_owned();
        assert_eq!(empty.depth(), 2);
        assert_eq!(empty.value_type(), "float");
        assert_eq!(empty.levels[0].offsets, [0, 0]);
        assert_eq!(empty.levels[1].offsets, [0]);
        // An empty selected row retains the deeper, nonexistent rows.
        let mut nested = Value::empty("float");
        nested.set_dimensions(&[2, 0, 3]);
        let row = nested.select(&[1]).unwrap().to_owned();
        assert_eq!(row.depth(), 2);
        assert_eq!(row.levels[0].offsets, [0, 0]);
        assert_eq!(row.levels[1].offsets, [0]);
        let LeafStorage::Int(original) = &mut data.entries[0].value.leaves else {
            unreachable!()
        };
        original[2] = 99;
        assert_eq!(copy.root().as_ints().unwrap(), &[3]);
        assert_eq!(data.int("rows"), Err(AccessError::ExpectedScalar));
        assert!(matches!(
            data.int("missing"),
            Err(AccessError::UnknownEntry(_))
        ));
    }
    #[test]
    fn incompatible_leaf_append() {
        assert_eq!(
            Value::empty("int").append(Value::string("x".into())),
            Err(crate::ErrorCode::ValueTypeMix)
        );
    }
}
