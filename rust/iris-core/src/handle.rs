//! Typed access to data through handles.
//!
//! A [`Handle`] names one entry and fixes its host type. The marker types here
//! say which: a value type ([`Int`], [`Float`], [`Str`], [`Num`] or [`Dyn`]), a
//! depth ([`D0`] to [`D8`], or [`DynDepth`]) and a presence ([`Required`] or
//! [`Maybe`]).
//!
//! | Depth | [`Int`] | [`Float`] | [`Str`] | [`Num`] | [`Dyn`] |
//! |---|---|---|---|---|---|
//! | [`D0`] | `i64` | `f64` | `&str` | [`Number`] | [`ValueRef`] |
//! | [`D1`] | `&[i64]` | `&[f64]` | `&[String]` | [`Numbers`] | [`ValueRef`] |
//! | [`D2`] to [`D8`] | [`Array`] | [`Array`] | [`Array`] | [`Array`] | [`Array`] |
//! | [`DynDepth`] | [`ValueRef`] | [`ValueRef`] | [`ValueRef`] | [`ValueRef`] | [`ValueRef`] |
//!
//! [`Array::row`] gives the view one level down. [`Maybe`] wraps the whole view
//! in an `Option`, which is `None` when the data has no such entry.
use crate::value::{AccessError, Data, LeafStorage, ValueRef};
use std::{fmt, marker::PhantomData};

/// Integer leaves.
#[derive(Debug, Clone, Copy)]
pub enum Int {}
/// Floating point leaves.
#[derive(Debug, Clone, Copy)]
pub enum Float {}
/// String leaves.
#[derive(Debug, Clone, Copy)]
pub enum Str {}
/// Integer or float leaves, decided by the data.
#[derive(Debug, Clone, Copy)]
pub enum Num {}
/// Leaves of any value type, read as a [`ValueRef`].
#[derive(Debug, Clone, Copy)]
pub enum Dyn {}

/// An entry that always exists after a successful read.
#[derive(Debug, Clone, Copy)]
pub enum Required {}
/// An entry the read may not have defined.
#[derive(Debug, Clone, Copy)]
pub enum Maybe {}

/// One `num` scalar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

/// One row of `num` leaves. All leaves of an entry share one storage type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Numbers<'a> {
    Ints(&'a [i64]),
    Floats(&'a [f64]),
}

impl Numbers<'_> {
    pub fn len(self) -> usize {
        match self {
            Self::Ints(row) => row.len(),
            Self::Floats(row) => row.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
}

/// A value type's scalar and row views.
pub trait Leaf {
    type Scalar<'a>;
    type Row<'a>;
    fn scalar(value: ValueRef<'_>) -> Result<Self::Scalar<'_>, AccessError>;
    fn row(value: ValueRef<'_>) -> Result<Self::Row<'_>, AccessError>;
    /// Whether the entry's storage holds this value type. An entry without
    /// leaves has only a provisional storage type, which matches every type.
    fn check(value: ValueRef<'_>) -> Result<(), AccessError>;
}

fn require_row(value: ValueRef<'_>) -> Result<(), AccessError> {
    if value.depth() == 1 {
        Ok(())
    } else {
        Err(AccessError::ExpectedArray)
    }
}

impl Leaf for Int {
    type Scalar<'a> = i64;
    type Row<'a> = &'a [i64];
    fn scalar(value: ValueRef<'_>) -> Result<i64, AccessError> {
        value.as_int()
    }
    fn row(value: ValueRef<'_>) -> Result<&[i64], AccessError> {
        require_row(value)?;
        if value.is_empty() {
            return Ok(&[]);
        }
        value.as_ints()
    }
    fn check(value: ValueRef<'_>) -> Result<(), AccessError> {
        match value.value.leaves {
            LeafStorage::Int(_) => Ok(()),
            _ if value.value.leaf_count() == 0 => Ok(()),
            _ => Err(AccessError::ExpectedInt),
        }
    }
}

impl Leaf for Float {
    type Scalar<'a> = f64;
    type Row<'a> = &'a [f64];
    fn scalar(value: ValueRef<'_>) -> Result<f64, AccessError> {
        value.as_float()
    }
    fn row(value: ValueRef<'_>) -> Result<&[f64], AccessError> {
        require_row(value)?;
        if value.is_empty() {
            return Ok(&[]);
        }
        value.as_floats()
    }
    fn check(value: ValueRef<'_>) -> Result<(), AccessError> {
        match value.value.leaves {
            LeafStorage::Float(_) => Ok(()),
            _ if value.value.leaf_count() == 0 => Ok(()),
            _ => Err(AccessError::ExpectedFloat),
        }
    }
}

impl Leaf for Str {
    type Scalar<'a> = &'a str;
    type Row<'a> = &'a [String];
    fn scalar(value: ValueRef<'_>) -> Result<&str, AccessError> {
        value.as_str()
    }
    fn row(value: ValueRef<'_>) -> Result<&[String], AccessError> {
        require_row(value)?;
        if value.is_empty() {
            return Ok(&[]);
        }
        value.as_strs()
    }
    fn check(value: ValueRef<'_>) -> Result<(), AccessError> {
        match value.value.leaves {
            LeafStorage::String(_) => Ok(()),
            _ if value.value.leaf_count() == 0 => Ok(()),
            _ => Err(AccessError::ExpectedString),
        }
    }
}

impl Leaf for Num {
    type Scalar<'a> = Number;
    type Row<'a> = Numbers<'a>;
    fn scalar(value: ValueRef<'_>) -> Result<Number, AccessError> {
        match value.as_int() {
            Ok(number) => Ok(Number::Int(number)),
            Err(AccessError::ExpectedInt) => value.as_float().map(Number::Float),
            Err(error) => Err(error),
        }
    }
    fn row(value: ValueRef<'_>) -> Result<Numbers<'_>, AccessError> {
        require_row(value)?;
        if value.is_empty() {
            return Ok(Numbers::Ints(&[]));
        }
        match value.as_ints() {
            Ok(row) => Ok(Numbers::Ints(row)),
            Err(AccessError::ExpectedInt) => value.as_floats().map(Numbers::Floats),
            Err(error) => Err(error),
        }
    }
    fn check(value: ValueRef<'_>) -> Result<(), AccessError> {
        match value.value.leaves {
            LeafStorage::String(_) if value.value.leaf_count() > 0 => {
                Err(AccessError::ExpectedFloat)
            }
            _ => Ok(()),
        }
    }
}

impl Leaf for Dyn {
    type Scalar<'a> = ValueRef<'a>;
    type Row<'a> = ValueRef<'a>;
    fn scalar(value: ValueRef<'_>) -> Result<ValueRef<'_>, AccessError> {
        Ok(value)
    }
    fn row(value: ValueRef<'_>) -> Result<ValueRef<'_>, AccessError> {
        Ok(value)
    }
    fn check(_: ValueRef<'_>) -> Result<(), AccessError> {
        Ok(())
    }
}

/// The number of array levels of an entry, as a type.
///
/// Depths form a closed set: [`D8`]`::Next` is [`DynDepth`], which is its own
/// successor, so no type grows with the depth of a reader spec.
pub trait Depth {
    type Next: Depth;
    /// This number of levels on top of `Other`.
    type Plus<Other: Depth>: Depth;
    /// The depth of a bare entry of depth `Entry` looked up in a scope at this
    /// depth. Outside every repeat that is `Entry`. Inside a repeat the entry
    /// can be the current record's value or a whole array, so it is [`DynDepth`].
    type Lookup<Entry: Depth>: Depth;
    /// The number of levels, or `None` for [`DynDepth`].
    const LEVELS: Option<usize>;
    type View<'a, V: Leaf>;
    fn view<V: Leaf>(value: ValueRef<'_>) -> Result<Self::View<'_, V>, AccessError>;
}

/// A scalar.
#[derive(Debug, Clone, Copy)]
pub enum D0 {}
/// An array.
#[derive(Debug, Clone, Copy)]
pub enum D1 {}
/// An array of arrays.
#[derive(Debug, Clone, Copy)]
pub enum D2 {}
#[derive(Debug, Clone, Copy)]
pub enum D3 {}
#[derive(Debug, Clone, Copy)]
pub enum D4 {}
#[derive(Debug, Clone, Copy)]
pub enum D5 {}
#[derive(Debug, Clone, Copy)]
pub enum D6 {}
#[derive(Debug, Clone, Copy)]
pub enum D7 {}
#[derive(Debug, Clone, Copy)]
pub enum D8 {}
/// A depth above [`D8`], or one known only at run time.
#[derive(Debug, Clone, Copy)]
pub enum DynDepth {}

impl Depth for D0 {
    type Next = D1;
    type Plus<Other: Depth> = Other;
    type Lookup<Entry: Depth> = Entry;
    const LEVELS: Option<usize> = Some(0);
    type View<'a, V: Leaf> = V::Scalar<'a>;
    fn view<V: Leaf>(value: ValueRef<'_>) -> Result<V::Scalar<'_>, AccessError> {
        V::scalar(value)
    }
}

impl Depth for D1 {
    type Next = D2;
    type Plus<Other: Depth> = Other::Next;
    type Lookup<Entry: Depth> = DynDepth;
    const LEVELS: Option<usize> = Some(1);
    type View<'a, V: Leaf> = V::Row<'a>;
    fn view<V: Leaf>(value: ValueRef<'_>) -> Result<V::Row<'_>, AccessError> {
        V::row(value)
    }
}

macro_rules! array_depths {
    ($($depth:ident $row:ident $next:ident $levels:literal;)*) => {$(
        impl Depth for $depth {
            type Next = $next;
            type Plus<Other: Depth> = <<$row as Depth>::Plus<Other> as Depth>::Next;
            type Lookup<Entry: Depth> = DynDepth;
            const LEVELS: Option<usize> = Some($levels);
            type View<'a, V: Leaf> = Array<'a, V, $row>;
            fn view<V: Leaf>(value: ValueRef<'_>) -> Result<Array<'_, V, $row>, AccessError> {
                Array::new(value, $levels)
            }
        }
    )*};
}

array_depths! {
    D2 D1 D3 2;
    D3 D2 D4 3;
    D4 D3 D5 4;
    D5 D4 D6 5;
    D6 D5 D7 6;
    D7 D6 D8 7;
    D8 D7 DynDepth 8;
}

impl Depth for DynDepth {
    type Next = DynDepth;
    type Plus<Other: Depth> = DynDepth;
    type Lookup<Entry: Depth> = DynDepth;
    const LEVELS: Option<usize> = None;
    type View<'a, V: Leaf> = ValueRef<'a>;
    fn view<V: Leaf>(value: ValueRef<'_>) -> Result<ValueRef<'_>, AccessError> {
        Ok(value)
    }
}

/// Whether an entry can be absent, and the host type that says so.
pub trait Presence {
    type Wrap<T>;
    /// `true` for [`Required`].
    const REQUIRED: bool;
    fn present<T>(view: T) -> Self::Wrap<T>;
    fn absent<T>(name: &str) -> Result<Self::Wrap<T>, AccessError>;
}

impl Presence for Required {
    type Wrap<T> = T;
    const REQUIRED: bool = true;
    fn present<T>(view: T) -> T {
        view
    }
    fn absent<T>(name: &str) -> Result<T, AccessError> {
        Err(AccessError::UnknownEntry(name.into()))
    }
}

impl Presence for Maybe {
    type Wrap<T> = Option<T>;
    const REQUIRED: bool = false;
    fn present<T>(view: T) -> Option<T> {
        Some(view)
    }
    fn absent<T>(_: &str) -> Result<Option<T>, AccessError> {
        Ok(None)
    }
}

/// An array of two or more levels. `R` is the depth of its rows.
pub struct Array<'a, V, R> {
    value: ValueRef<'a>,
    marker: PhantomData<fn() -> (V, R)>,
}

impl<V, R> Clone for Array<'_, V, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<V, R> Copy for Array<'_, V, R> {}

impl<V, R> fmt::Debug for Array<'_, V, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Array").field(&self.value).finish()
    }
}

impl<'a, V: Leaf, R: Depth> Array<'a, V, R> {
    fn new(value: ValueRef<'a>, levels: usize) -> Result<Self, AccessError> {
        if value.depth() != levels {
            return Err(AccessError::ExpectedArray);
        }
        V::check(value)?;
        Ok(Self {
            value,
            marker: PhantomData,
        })
    }

    pub fn len(self) -> usize {
        self.value.len()
    }

    pub fn is_empty(self) -> bool {
        self.value.is_empty()
    }

    /// The row at `index`, panicking like slice indexing when it is out of
    /// bounds.
    pub fn row(self, index: usize) -> R::View<'a, V> {
        let len = self.len();
        self.get(index).unwrap_or_else(|| {
            panic!("row index {index} is out of bounds for an array of {len} rows")
        })
    }

    pub fn get(self, index: usize) -> Option<R::View<'a, V>> {
        let row = self.value.get(index)?;
        Some(R::view::<V>(row).expect("the array's depth and leaves were checked on creation"))
    }

    pub fn rows(self) -> impl ExactSizeIterator<Item = R::View<'a, V>> {
        (0..self.len()).map(move |index| self.row(index))
    }
}

/// A key into [`Data`] that fixes the host type of the entry it names.
pub trait Handle: Copy {
    type View<'a>;
    fn name(self) -> &'static str;
    fn view(self, data: &Data) -> Result<Self::View<'_>, AccessError>;
}

/// The view of entry `name` as value type `V`, depth `D` and presence `O`.
pub fn view<'a, V: Leaf, D: Depth, O: Presence>(
    data: &'a Data,
    name: &str,
) -> Result<O::Wrap<D::View<'a, V>>, AccessError> {
    match data.value(name, &[]) {
        Ok(value) => D::view::<V>(value).map(O::present),
        Err(AccessError::UnknownEntry(_)) => O::absent(name),
        Err(error) => Err(error),
    }
}

impl Data {
    /// The entry that `handle` names, with its host type.
    ///
    /// # Panics
    ///
    /// Panics when the data does not match the handle, which means the handle
    /// belongs to another reader spec. Use [`Data::try_get`] for a handle whose
    /// spec is not known.
    pub fn get<H: Handle>(&self, handle: H) -> H::View<'_> {
        self.try_get(handle).unwrap_or_else(|error| {
            panic!(
                "entry '{}' does not match its handle: {error}; try_get reports this as an error",
                handle.name()
            )
        })
    }

    /// The entry that `handle` names, or why the data does not match it.
    pub fn try_get<H: Handle>(&self, handle: H) -> Result<H::View<'_>, AccessError> {
        handle.view(self)
    }
}
