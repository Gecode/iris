//! Reading types and the options of a declaration.
use gecode_iris_core::handle::{D0, Depth, DynDepth, Float, Int, Num, Str};
use std::marker::PhantomData;
use std::ops::{Range, RangeFrom, RangeInclusive, RangeTo, RangeToInclusive};

use crate::expr::{Bound, Expr, Name, Op, Operand, ValueType};

#[derive(Debug, Clone)]
pub(crate) enum TypeSpec {
    Base(&'static str),
    Alphabet {
        symbols: String,
        options: Vec<AlphabetOption>,
    },
    Named(TypeRef),
    Range {
        lower: Option<Vec<Op>>,
        upper: Option<Vec<Op>>,
        inclusive: bool,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum AlphabetOption {
    Blank(String),
    Strip(String),
    Base(i64),
}

/// A type with the dimensions and options of a declaration.
#[derive(Debug, Clone)]
pub struct Declared {
    pub(crate) dimensions: Vec<Vec<Op>>,
    pub(crate) ty: TypeSpec,
    pub(crate) layout: Option<Vec<Op>>,
    pub(crate) offset: Option<Vec<Op>>,
}

impl Declared {
    fn new(ty: TypeSpec) -> Self {
        Self {
            dimensions: Vec::new(),
            ty,
            layout: None,
            offset: None,
        }
    }

    /// Whether the text of this type ends in `..`, with no upper bound.
    pub(crate) fn is_open_ended(&self) -> bool {
        self.layout.is_none()
            && self.offset.is_none()
            && matches!(self.ty, TypeSpec::Range { upper: None, .. })
    }
}

/// Anything `read` accepts: a type, or a bare Rust range.
pub trait IntoDeclared {
    type Value: ValueType;
    /// The declaration's own array levels.
    type Dims: Depth;
    #[doc(hidden)]
    fn into_declared(self) -> Declared;
}

/// A reading type of value type `V` with `N` dimensions.
#[derive(Debug, Clone)]
pub struct BaseType<V, N = D0> {
    declared: Declared,
    marker: PhantomData<fn() -> (V, N)>,
}

impl<V, N> BaseType<V, N> {
    fn from_declared(declared: Declared) -> Self {
        Self {
            declared,
            marker: PhantomData,
        }
    }
}

impl<V: ValueType, N: Depth> IntoDeclared for BaseType<V, N> {
    type Value = V;
    type Dims = N;
    fn into_declared(self) -> Declared {
        self.declared
    }
}

fn base<V>(name: &'static str) -> BaseType<V> {
    BaseType::from_declared(Declared::new(TypeSpec::Base(name)))
}

/// `int`.
pub fn int() -> BaseType<Int> {
    base("int")
}

/// `float`.
pub fn float() -> BaseType<Float> {
    base("float")
}

/// `num`: an int or a float, decided by the data.
pub fn num() -> BaseType<Num> {
    base("num")
}

/// `word`.
pub fn word() -> BaseType<Str> {
    base("word")
}

/// `rest`.
pub fn rest() -> BaseType<Str> {
    base("rest")
}

/// A range or other type, with the methods of [`BaseType`].
pub fn ty<R: IntoDeclared>(reading: R) -> BaseType<R::Value, R::Dims> {
    BaseType::from_declared(reading.into_declared())
}

impl<V, N: Depth> BaseType<V, N> {
    /// Add a dimension: `x[size]: ...`.
    pub fn dim<E: Operand>(mut self, size: E) -> BaseType<V, N::Next> {
        self.declared.dimensions.push(size.into_expr().ops);
        BaseType::from_declared(self.declared)
    }

    /// Add a number of dimensions known only at run time.
    pub fn dims<E: Operand>(mut self, sizes: impl IntoIterator<Item = E>) -> BaseType<V, DynDepth> {
        self.declared
            .dimensions
            .extend(sizes.into_iter().map(|size| size.into_expr().ops));
        BaseType::from_declared(self.declared)
    }

    /// `layout expression`.
    pub fn layout<E: Operand>(mut self, layout: E) -> Self {
        self.declared.layout = Some(layout.into_expr().ops);
        self
    }

    /// `offset expression`.
    pub fn offset<E: Operand>(mut self, offset: E) -> Self {
        self.declared.offset = Some(offset.into_expr().ops);
        self
    }
}

impl<V> BaseType<V> {
    /// A list of this type ending at `end`: a sentinel, or [`eol`].
    pub fn until<E: ListEnd>(self, end: E) -> ListReading<V> {
        ListReading {
            declared: self.declared,
            end: end.into_list_end(),
            marker: PhantomData,
        }
    }
}

/// The end of a list.
#[derive(Debug, Clone)]
pub enum ListTerminator {
    Eol,
    Sentinel(Vec<Op>),
}

/// What [`BaseType::until`] accepts.
pub trait ListEnd {
    #[doc(hidden)]
    fn into_list_end(self) -> ListTerminator;
}

impl<E: Operand> ListEnd for E {
    fn into_list_end(self) -> ListTerminator {
        ListTerminator::Sentinel(self.into_expr().ops)
    }
}

/// The end-of-line marker of a list; see [`eol`].
#[derive(Debug, Clone, Copy)]
pub struct Eol;

/// `until eol`.
pub fn eol() -> Eol {
    Eol
}

impl ListEnd for Eol {
    fn into_list_end(self) -> ListTerminator {
        ListTerminator::Eol
    }
}

/// The type and end of a `list` statement.
#[derive(Debug, Clone)]
pub struct ListReading<V> {
    pub(crate) declared: Declared,
    pub(crate) end: ListTerminator,
    marker: PhantomData<fn() -> V>,
}

impl<V> ListReading<V> {
    /// `offset expression`, after the list's end.
    pub fn offset<E: Operand>(mut self, offset: E) -> Self {
        self.declared.offset = Some(offset.into_expr().ops);
        self
    }
}

/// `alphabet "symbols"`, with its options in call order.
#[derive(Debug, Clone)]
pub struct Alphabet {
    symbols: String,
    options: Vec<AlphabetOption>,
}

/// `alphabet symbols`.
pub fn alphabet(symbols: impl Into<String>) -> Alphabet {
    Alphabet {
        symbols: symbols.into(),
        options: Vec::new(),
    }
}

impl Alphabet {
    /// `blank "characters"`.
    pub fn blank(mut self, characters: impl Into<String>) -> Self {
        self.options.push(AlphabetOption::Blank(characters.into()));
        self
    }

    /// `strip "characters"`.
    pub fn strip(mut self, characters: impl Into<String>) -> Self {
        self.options.push(AlphabetOption::Strip(characters.into()));
        self
    }

    /// `base value`.
    pub fn base(mut self, value: i64) -> Self {
        self.options.push(AlphabetOption::Base(value));
        self
    }

    /// Add a dimension; see [`BaseType::dim`].
    pub fn dim<E: Operand>(self, size: E) -> BaseType<Int, <D0 as Depth>::Next> {
        ty(self).dim(size)
    }
}

impl IntoDeclared for Alphabet {
    type Value = Int;
    type Dims = D0;
    fn into_declared(self) -> Declared {
        Declared::new(TypeSpec::Alphabet {
            symbols: self.symbols,
            options: self.options,
        })
    }
}

/// A use of the named type declared by the statement `declaration`.
#[derive(Debug, Clone, Copy)]
pub struct TypeRef {
    pub(crate) name: Name,
    pub(crate) declaration: usize,
}

/// A named type, usable wherever a type is. A handle belongs to its declaration:
/// `build()` reports a use where that declaration is not the visible one, even
/// if a sibling block declares the same name.
#[derive(Debug)]
pub struct NamedType<V> {
    reference: TypeRef,
    marker: PhantomData<fn() -> V>,
}

impl<V> Clone for NamedType<V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<V> Copy for NamedType<V> {}

impl<V> NamedType<V> {
    pub(crate) fn new(name: Name, declaration: usize) -> Self {
        Self {
            reference: TypeRef { name, declaration },
            marker: PhantomData,
        }
    }
}

impl<V: ValueType> IntoDeclared for NamedType<V> {
    type Value = V;
    type Dims = D0;
    fn into_declared(self) -> Declared {
        Declared::new(TypeSpec::Named(self.reference))
    }
}

fn range(lower: Option<Vec<Op>>, upper: Option<Vec<Op>>, inclusive: bool) -> Declared {
    Declared::new(TypeSpec::Range {
        lower,
        upper,
        inclusive,
    })
}

/// The forms of a Rust range over the bound type `$bound`. Rust's `lo..hi`
/// excludes `hi`, so it prints as `lo..<hi`, and `lo..=hi` prints as `lo..hi`.
macro_rules! range_types {
    ([$($generics:tt)*] $bound:ty) => {
        impl<$($generics)*> IntoDeclared for Range<$bound> {
            type Value = Int;
            type Dims = D0;
            fn into_declared(self) -> Declared {
                let (lower, upper) = (self.start.into_expr().ops, self.end.into_expr().ops);
                range(Some(lower), Some(upper), false)
            }
        }
        impl<$($generics)*> IntoDeclared for RangeInclusive<$bound> {
            type Value = Int;
            type Dims = D0;
            fn into_declared(self) -> Declared {
                let (lower, upper) = self.into_inner();
                let (lower, upper) = (lower.into_expr().ops, upper.into_expr().ops);
                range(Some(lower), Some(upper), true)
            }
        }
        impl<$($generics)*> IntoDeclared for RangeFrom<$bound> {
            type Value = Int;
            type Dims = D0;
            fn into_declared(self) -> Declared {
                range(Some(self.start.into_expr().ops), None, true)
            }
        }
        impl<$($generics)*> IntoDeclared for RangeTo<$bound> {
            type Value = Int;
            type Dims = D0;
            fn into_declared(self) -> Declared {
                range(None, Some(self.end.into_expr().ops), false)
            }
        }
        impl<$($generics)*> IntoDeclared for RangeToInclusive<$bound> {
            type Value = Int;
            type Dims = D0;
            fn into_declared(self) -> Declared {
                range(None, Some(self.end.into_expr().ops), true)
            }
        }
    };
}
range_types!([V: Bound] Expr<V>);
range_types!([] i32);
range_types!([] i64);
