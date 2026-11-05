//! Typed handles and the names behind them.
use gecode_iris_core::handle::{self, D0, Depth, Dyn, DynDepth, Leaf, Maybe, Presence, Required};
use gecode_iris_core::{AccessError, Data, Handle};
use std::collections::HashSet;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use crate::expr::ValueType;

/// Which builder made a handle. It only compares handles; it is never printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuilderId(u64);

impl BuilderId {
    /// The owner of a [`Field::declared`] handle, which no builder accepts.
    const NONE: Self = Self(0);

    pub(crate) fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// One copy of each distinct name, kept for the life of the process. Handles
/// are `Copy` and outlive their builder, so they need `&'static` names.
///
/// Every distinct name supplied stays allocated, including the names of dropped
/// builders, so a host that generates names without limit grows without limit.
pub(crate) fn intern(name: &str) -> &'static str {
    static NAMES: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(Mutex::default);
    let mut names = NAMES.lock().expect("no panic happens while interning");
    if let Some(interned) = names.get(name) {
        return interned;
    }
    let interned: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.insert(interned);
    interned
}

type Marker<V, D, O> = PhantomData<fn() -> (V, D, O)>;

/// A typed handle: the name of an entry with its value type `V`, depth `D` and
/// presence `O`.
///
/// In the builder a handle is an expression. After reading it is a key:
/// [`Data::get`] returns the host type for its value type and depth. A handle's
/// type is its data type from the moment it is created, so a handle that leaves
/// a repeat already is the collected array.
pub struct Field<V, D = D0, O = Required> {
    pub(crate) name: &'static str,
    pub(crate) owner: BuilderId,
    marker: Marker<V, D, O>,
}

/// A handle for a name whose value type or depth is open.
pub type DynField = Field<Dyn, DynDepth, Maybe>;

impl<V, D, O> Clone for Field<V, D, O> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<V, D, O> Copy for Field<V, D, O> {}

impl<V, D, O> fmt::Debug for Field<V, D, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Field").field(&self.name).finish()
    }
}

impl<V, D, O> Field<V, D, O> {
    /// A handle for an entry of a text spec. It reads data, but no builder
    /// accepts it, since it comes from no builder.
    pub const fn declared(name: &'static str) -> Self {
        Self::new(name, BuilderId::NONE)
    }

    pub(crate) const fn new(name: &'static str, owner: BuilderId) -> Self {
        Self {
            name,
            owner,
            marker: PhantomData,
        }
    }

    pub fn name(self) -> &'static str {
        self.name
    }
}

impl<V: Leaf, D: Depth, O: Presence> Handle for Field<V, D, O> {
    type View<'a> = O::Wrap<D::View<'a, V>>;

    fn name(self) -> &'static str {
        self.name
    }

    fn view(self, data: &Data) -> Result<Self::View<'_>, AccessError> {
        handle::view::<V, D, O>(data, self.name)
    }
}

/// What a [`Forward`] handle promises about its definitions.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    pub(crate) value: crate::expr::ValueTag,
    pub(crate) levels: Option<usize>,
    pub(crate) required: bool,
}

/// A handle type that [`Scope::forward`](crate::Scope::forward) can declare.
pub trait Forward: Sized {
    #[doc(hidden)]
    fn shape() -> Shape;
    #[doc(hidden)]
    fn handle(name: &'static str, owner: BuilderId) -> Self;
}

impl<V: ValueType, D: Depth, O: Presence> Forward for Field<V, D, O> {
    fn shape() -> Shape {
        Shape {
            value: V::TAG,
            levels: D::LEVELS,
            required: O::REQUIRED,
        }
    }

    fn handle(name: &'static str, owner: BuilderId) -> Self {
        Self::new(name, owner)
    }
}
