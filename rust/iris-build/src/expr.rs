//! Expressions: flat postfix operations, so that a chain of any length prints
//! and drops without recursion.
use gecode_iris_core::handle::{
    D0, D1, D2, D3, D4, D5, D6, D7, D8, Depth, Dyn, DynDepth, Float, Int, Leaf, Num, Str,
};
use std::marker::PhantomData;
use std::ops::{Add, BitAnd, BitOr, Div, Mul, Neg, Not, Rem, Sub};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::field::{BuilderId, Field, intern};
use crate::statement::Record;

/// A value type as data, for comparing shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTag {
    Int,
    Float,
    Str,
    Num,
    Dyn,
}

impl ValueTag {
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::Str => "string",
            Self::Num => "num",
            Self::Dyn => "dynamic",
        }
    }
}

/// A value type that expressions and handles can carry.
pub trait ValueType: Leaf {
    #[doc(hidden)]
    const TAG: ValueTag;
    /// The depth of a `let` of an expression of this type in a scope at depth
    /// `D`. A dynamic value can have any depth.
    type LetDepth<S: ExprShape, D: Depth>: Depth;
}

macro_rules! static_value_types {
    ($($value:ident),*) => {$(
        impl ValueType for $value {
            const TAG: ValueTag = ValueTag::$value;
            type LetDepth<S: ExprShape, D: Depth> = S::Depth<D>;
        }
    )*};
}
static_value_types!(Int, Float, Str, Num);

impl ValueType for Dyn {
    const TAG: ValueTag = ValueTag::Dyn;
    type LetDepth<S: ExprShape, D: Depth> = DynDepth;
}

/// Whether an expression is known to be a scalar.
pub trait ExprShape {
    /// The depth of a value of this shape computed in a scope at depth `D`.
    type Depth<D: Depth>: Depth;
}

/// A literal, operator result, call, or comprehension.
#[derive(Debug, Clone, Copy)]
pub enum Scalar {}
/// A bare handle, index, or field, which may be an array.
#[derive(Debug, Clone, Copy)]
pub enum Open {}

impl ExprShape for Scalar {
    type Depth<D: Depth> = D;
}
impl ExprShape for Open {
    type Depth<D: Depth> = DynDepth;
}

/// Value types that `+ - * / % ^`, `!`, `&` and `|` accept.
pub trait Integral: ValueType {}
impl Integral for Int {}
impl Integral for Dyn {}

/// Value types that unary `-` and the ordering comparisons accept.
pub trait Numeric: ValueType {}
impl Numeric for Int {}
impl Numeric for Float {}
impl Numeric for Num {}
impl Numeric for Dyn {}

/// Value types a range bound accepts.
pub trait Bound: ValueType {}
impl Bound for Int {}
impl Bound for Num {}
impl Bound for Dyn {}

/// Pairs of value types that `.eq()` and `.ne()` accept. Use `.dynamic()` to
/// compare any other pair.
pub trait Equatable<Rhs> {}
macro_rules! equatable {
    ($($left:ident $right:ident;)*) => {$(impl Equatable<$right> for $left {})*};
}
equatable! {
    Int Int; Int Float; Int Num; Float Int; Float Float; Float Num; Num Int; Num Float; Num Num;
    Str Str;
    Dyn Dyn; Dyn Int; Dyn Float; Dyn Num; Dyn Str; Int Dyn; Float Dyn; Num Dyn; Str Dyn;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingId(u64);

impl BindingId {
    /// Identifies a comprehension variable. Printed names never depend on this
    /// number; they are numbered by first appearance in the text.
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Name {
    pub(crate) text: &'static str,
    pub(crate) owner: BuilderId,
}

#[derive(Debug, Clone, Copy)]
pub struct RecordRef {
    pub(crate) id: usize,
    pub(crate) owner: BuilderId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Minus,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Power,
}

/// One postfix operation. Each pops its operands' results and pushes one.
#[derive(Debug, Clone)]
pub enum Op {
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    Host(&'static str),
    Name(Name),
    Binding(BindingId),
    /// The `each` name of a repeat.
    Record(RecordRef),
    /// The `as` name of a repeat.
    RecordList(RecordRef),
    Unary(UnaryOp),
    Binary(BinaryOp),
    Index,
    /// `.name` on the preceding operand.
    Member(Name),
    /// `.records` on the preceding operand, with a repeat's `as` name.
    ListMember(RecordRef),
    Call {
        function: &'static str,
        arguments: usize,
    },
    Comprehension {
        function: &'static str,
        binding: BindingId,
        name: Option<&'static str>,
    },
}

impl Op {
    /// How many operands this operation pops.
    pub(crate) fn operands(&self) -> usize {
        match self {
            Self::Unary(_) | Self::Member(_) | Self::ListMember(_) => 1,
            Self::Binary(_) | Self::Index | Self::Comprehension { .. } => 2,
            Self::Call { arguments, .. } => *arguments,
            _ => 0,
        }
    }
}

/// An expression of value type `V`. `S` says whether it is known to be a
/// scalar.
#[derive(Debug)]
pub struct Expr<V, S = Scalar> {
    pub(crate) ops: Vec<Op>,
    marker: PhantomData<fn() -> (V, S)>,
}

impl<V, S> Clone for Expr<V, S> {
    fn clone(&self) -> Self {
        Self::from_ops(self.ops.clone())
    }
}

impl<V, S> Expr<V, S> {
    pub(crate) fn from_ops(ops: Vec<Op>) -> Self {
        Self {
            ops,
            marker: PhantomData,
        }
    }

    /// Whether this expression looks up a stored value.
    fn is_lookup(&self) -> bool {
        matches!(
            self.ops.last(),
            Some(Op::Name(_) | Op::Index | Op::Member(_))
        )
    }
}

/// Anything that can be an operand: a handle, an expression, a host constant,
/// or a Rust literal.
pub trait Operand {
    type Value: ValueType;
    type Shape: ExprShape;
    /// The depth of this operand's value in a scope at depth `D`, as far as its
    /// type says.
    type DepthIn<D: Depth>: Depth;
    fn into_expr(self) -> Expr<Self::Value, Self::Shape>;
}

impl<V: ValueType, S: ExprShape> Operand for Expr<V, S> {
    type Value = V;
    type Shape = S;
    type DepthIn<D: Depth> = V::LetDepth<S, D>;
    fn into_expr(self) -> Self {
        self
    }
}

impl<V: ValueType, H: Depth, O> Operand for Field<V, H, O> {
    type Value = V;
    type Shape = Open;
    type DepthIn<D: Depth> = D::Lookup<H>;
    fn into_expr(self) -> Expr<V, Open> {
        Expr::from_ops(vec![Op::Name(Name {
            text: self.name,
            owner: self.owner,
        })])
    }
}

/// The depth a `let` of `E` has in a scope at depth `D`.
pub type LetDepth<E, D> = <<E as Operand>::Value as ValueType>::LetDepth<<E as Operand>::Shape, D>;

/// The array levels of `E`'s value when its type says it is a scalar.
pub(crate) fn scalar_levels<E: Operand>() -> Option<usize> {
    <LetDepth<E, D0> as Depth>::LEVELS
}

/// The array levels a `let` of `value` adds in a scope at depth `D`, when `E`'s
/// type knows them. `ex()` only asserts that a lookup is a scalar, so it says
/// nothing about the value.
pub(crate) fn let_levels<E: Operand, D: Depth>(value: &Expr<E::Value, E::Shape>) -> Option<usize> {
    if scalar_levels::<E>().is_some() && value.is_lookup() {
        return None;
    }
    Some(<E::DepthIn<D> as Depth>::LEVELS? - D::LEVELS?)
}

/// Depths that can be the same: equal ones, or any depth and [`DynDepth`].
#[diagnostic::on_unimplemented(
    message = "`set` keeps the handle's depth `{Other}`, but this value has depth `{Self}`",
    note = "to change the shape, redefine the name with `let_(handle, value)`"
)]
pub trait SameDepth<Other> {}
impl<D: Depth> SameDepth<D> for D {}
macro_rules! open_depths {
    ($($depth:ident),*) => {$(
        impl SameDepth<DynDepth> for $depth {}
        impl SameDepth<$depth> for DynDepth {}
    )*};
}
open_depths!(D0, D1, D2, D3, D4, D5, D6, D7, D8);

/// A Rust value with a spelling in Iris.
pub trait Literal {
    type Value: ValueType;
    #[doc(hidden)]
    fn into_literal(self) -> LiteralOp;
}

/// One literal operation.
#[derive(Debug)]
pub struct LiteralOp(pub(crate) Op);

macro_rules! integer_literals {
    ($($integer:ty),*) => {$(
        impl Literal for $integer {
            type Value = Int;
            fn into_literal(self) -> LiteralOp {
                LiteralOp(Op::Int(i64::from(self)))
            }
        }
    )*};
}
integer_literals!(i32, i64, u32);

impl Literal for f64 {
    type Value = Float;
    fn into_literal(self) -> LiteralOp {
        LiteralOp(Op::Float(self))
    }
}
impl Literal for &str {
    type Value = Str;
    fn into_literal(self) -> LiteralOp {
        LiteralOp(Op::Str(self.into()))
    }
}
impl Literal for String {
    type Value = Str;
    fn into_literal(self) -> LiteralOp {
        LiteralOp(Op::Str(self.into()))
    }
}

impl<L: Literal> Operand for L {
    type Value = L::Value;
    type Shape = Scalar;
    type DepthIn<D: Depth> = <L::Value as ValueType>::LetDepth<Scalar, D>;
    fn into_expr(self) -> Expr<L::Value> {
        Expr::from_ops(vec![self.into_literal().0])
    }
}

/// An Iris literal.
pub fn lit<L: Literal>(value: L) -> Expr<L::Value> {
    value.into_expr()
}

/// A host constant; see [`gecode`].
#[derive(Debug, Clone, Copy)]
pub struct Host {
    name: &'static str,
}

impl Operand for Host {
    type Value = Int;
    type Shape = Scalar;
    type DepthIn<D: Depth> = D;
    fn into_expr(self) -> Expr<Int> {
        Expr::from_ops(vec![Op::Host(self.name)])
    }
}

/// The host constants of Iris 0.1.
pub mod gecode {
    use super::Host;

    /// `gecode.int_max`, 2147483646.
    pub const INT_MAX: Host = Host {
        name: "gecode.int_max",
    };
    /// `gecode.int_min`, -2147483646.
    pub const INT_MIN: Host = Host {
        name: "gecode.int_min",
    };
}

fn binary<Output, L: Operand, R: Operand>(left: L, right: R, op: BinaryOp) -> Expr<Output> {
    let mut ops = left.into_expr().ops;
    ops.extend(right.into_expr().ops);
    ops.push(Op::Binary(op));
    Expr::from_ops(ops)
}

fn unary<Output, E: Operand>(operand: E, op: UnaryOp) -> Expr<Output> {
    let mut ops = operand.into_expr().ops;
    ops.push(Op::Unary(op));
    Expr::from_ops(ops)
}

macro_rules! binary_operator {
    ($trait:ident $method:ident $op:ident) => {
        impl<V: Integral, D: Depth, O, R: Operand> $trait<R> for Field<V, D, O>
        where
            R::Value: Integral,
        {
            type Output = Expr<Int>;
            fn $method(self, right: R) -> Expr<Int> {
                binary(self, right, BinaryOp::$op)
            }
        }
        impl<V: Integral, S: ExprShape, R: Operand> $trait<R> for Expr<V, S>
        where
            R::Value: Integral,
        {
            type Output = Expr<Int>;
            fn $method(self, right: R) -> Expr<Int> {
                binary(self, right, BinaryOp::$op)
            }
        }
        impl<R: Operand> $trait<R> for Host
        where
            R::Value: Integral,
        {
            type Output = Expr<Int>;
            fn $method(self, right: R) -> Expr<Int> {
                binary(self, right, BinaryOp::$op)
            }
        }
        binary_operator!(@literal $trait $method $op i32);
        binary_operator!(@literal $trait $method $op i64);
        binary_operator!(@literal $trait $method $op u32);
    };
    (@literal $trait:ident $method:ident $op:ident $integer:ty) => {
        impl<V: Integral, D: Depth, O> $trait<Field<V, D, O>> for $integer {
            type Output = Expr<Int>;
            fn $method(self, right: Field<V, D, O>) -> Expr<Int> {
                binary(self, right, BinaryOp::$op)
            }
        }
        impl<V: Integral, S: ExprShape> $trait<Expr<V, S>> for $integer {
            type Output = Expr<Int>;
            fn $method(self, right: Expr<V, S>) -> Expr<Int> {
                binary(self, right, BinaryOp::$op)
            }
        }
        impl $trait<Host> for $integer {
            type Output = Expr<Int>;
            fn $method(self, right: Host) -> Expr<Int> {
                binary(self, right, BinaryOp::$op)
            }
        }
    };
}

binary_operator!(Add add Add);
binary_operator!(Sub sub Subtract);
binary_operator!(Mul mul Multiply);
binary_operator!(Div div Divide);
binary_operator!(Rem rem Remainder);
binary_operator!(BitAnd bitand And);
binary_operator!(BitOr bitor Or);

impl<V: Numeric, D: Depth, O> Neg for Field<V, D, O> {
    type Output = Expr<V>;
    fn neg(self) -> Expr<V> {
        unary(self, UnaryOp::Minus)
    }
}
impl<V: Numeric, S: ExprShape> Neg for Expr<V, S> {
    type Output = Expr<V>;
    fn neg(self) -> Expr<V> {
        unary(self, UnaryOp::Minus)
    }
}
impl Neg for Host {
    type Output = Expr<Int>;
    fn neg(self) -> Expr<Int> {
        unary(self, UnaryOp::Minus)
    }
}

impl<V: Integral, D: Depth, O> Not for Field<V, D, O> {
    type Output = Expr<Int>;
    fn not(self) -> Expr<Int> {
        unary(self, UnaryOp::Not)
    }
}
impl<V: Integral, S: ExprShape> Not for Expr<V, S> {
    type Output = Expr<Int>;
    fn not(self) -> Expr<Int> {
        unary(self, UnaryOp::Not)
    }
}
impl Not for Host {
    type Output = Expr<Int>;
    fn not(self) -> Expr<Int> {
        unary(self, UnaryOp::Not)
    }
}

/// The named operators, for a left operand of value type `$value`.
macro_rules! named_operators {
    ($value:ty) => {
        /// `^`.
        pub fn pow<R: Operand>(self, exponent: R) -> Expr<Int>
        where
            $value: Integral,
            R::Value: Integral,
        {
            binary(self, exponent, BinaryOp::Power)
        }
        /// `==`.
        pub fn eq<R: Operand>(self, right: R) -> Expr<Int>
        where
            $value: Equatable<R::Value>,
        {
            binary(self, right, BinaryOp::Equal)
        }
        /// `!=`.
        pub fn ne<R: Operand>(self, right: R) -> Expr<Int>
        where
            $value: Equatable<R::Value>,
        {
            binary(self, right, BinaryOp::NotEqual)
        }
        /// `<`.
        pub fn lt<R: Operand>(self, right: R) -> Expr<Int>
        where
            $value: Numeric,
            R::Value: Numeric,
        {
            binary(self, right, BinaryOp::Less)
        }
        /// `<=`.
        pub fn le<R: Operand>(self, right: R) -> Expr<Int>
        where
            $value: Numeric,
            R::Value: Numeric,
        {
            binary(self, right, BinaryOp::LessEqual)
        }
        /// `>`.
        pub fn gt<R: Operand>(self, right: R) -> Expr<Int>
        where
            $value: Numeric,
            R::Value: Numeric,
        {
            binary(self, right, BinaryOp::Greater)
        }
        /// `>=`.
        pub fn ge<R: Operand>(self, right: R) -> Expr<Int>
        where
            $value: Numeric,
            R::Value: Numeric,
        {
            binary(self, right, BinaryOp::GreaterEqual)
        }
        /// This expression with no static type, accepted anywhere.
        pub fn dynamic(self) -> Expr<Dyn> {
            Expr::from_ops(self.into_expr().ops)
        }
    };
}

impl<V: ValueType, D: Depth, O> Field<V, D, O> {
    named_operators!(V);

    /// This handle as a scalar expression, as a range bound requires. It only
    /// asserts that the value is a scalar, so a `let` of the result has the
    /// static type of one whatever the value turns out to be.
    pub fn ex(self) -> Expr<V> {
        Expr::from_ops(self.into_expr().ops)
    }

    /// `self[index]`.
    pub fn at<I: Operand>(self, index: I) -> Expr<V, Open>
    where
        I::Value: Integral,
    {
        index_ops(self.into_expr().ops, index)
    }
}

impl<V: ValueType, S: ExprShape> Expr<V, S> {
    named_operators!(V);
}

impl Host {
    named_operators!(Int);
}

fn index_ops<V, I: Operand>(mut ops: Vec<Op>, index: I) -> Expr<V, Open> {
    ops.extend(index.into_expr().ops);
    ops.push(Op::Index);
    Expr::from_ops(ops)
}

fn member_ops<V, D, O, Output>(mut ops: Vec<Op>, field: Field<V, D, O>) -> Expr<Output, Open> {
    ops.push(Op::Member(Name {
        text: field.name,
        owner: field.owner,
    }));
    Expr::from_ops(ops)
}

impl<V: ValueType> Expr<V, Open> {
    /// This expression as a scalar, as a range bound requires; see
    /// [`Field::ex`].
    pub fn ex(self) -> Expr<V> {
        Expr::from_ops(self.ops)
    }

    /// `self[index]`.
    pub fn at<I: Operand>(self, index: I) -> Self
    where
        I::Value: Integral,
    {
        index_ops(self.ops, index)
    }

    /// `self.field`.
    pub fn get<F, D, O>(self, field: Field<F, D, O>) -> Expr<F, Open> {
        member_ops(self.ops, field)
    }
}

impl Expr<Dyn> {
    /// `self[index]`.
    pub fn at<I: Operand>(self, index: I) -> Expr<Dyn, Open>
    where
        I::Value: Integral,
    {
        index_ops(self.ops, index)
    }

    /// `self.field`.
    pub fn get<F, D, O>(self, field: Field<F, D, O>) -> Expr<F, Open> {
        member_ops(self.ops, field)
    }
}

/// One record of a repeat: a [`Record`] selected by index, or a comprehension
/// variable over a record list.
#[derive(Debug, Clone)]
pub struct RecordExpr {
    ops: Vec<Op>,
}

/// A list of records: all the records of a repeat, or a nested list.
#[derive(Debug, Clone)]
pub struct RecordList {
    ops: Vec<Op>,
}

impl RecordExpr {
    /// `self.field`.
    pub fn get<V, D, O>(self, field: Field<V, D, O>) -> Expr<V, Open> {
        member_ops(self.ops, field)
    }

    /// `self.inner`, the records of the nested repeat `inner`.
    pub fn records(mut self, inner: Record) -> RecordList {
        self.ops.push(Op::ListMember(inner.reference()));
        RecordList { ops: self.ops }
    }

    /// `self[index]`.
    pub fn at<I: Operand>(mut self, index: I) -> Self
    where
        I::Value: Integral,
    {
        self.ops.extend(index.into_expr().ops);
        self.ops.push(Op::Index);
        self
    }
}

impl RecordList {
    /// `self[index]`.
    pub fn at<I: Operand>(mut self, index: I) -> RecordExpr
    where
        I::Value: Integral,
    {
        self.ops.extend(index.into_expr().ops);
        self.ops.push(Op::Index);
        RecordExpr { ops: self.ops }
    }
}

impl Record {
    /// `job.field`, the field of the current record.
    pub fn get<V, D, O>(self, field: Field<V, D, O>) -> Expr<V, Open> {
        member_ops(vec![Op::Record(self.reference())], field)
    }

    /// `job.inner`, the records of the nested repeat `inner`.
    pub fn records(self, inner: Record) -> RecordList {
        RecordList {
            ops: vec![
                Op::Record(self.reference()),
                Op::ListMember(inner.reference()),
            ],
        }
    }

    /// `jobs`, all the records of this repeat.
    pub fn list(self) -> RecordList {
        RecordList {
            ops: vec![Op::RecordList(self.reference())],
        }
    }

    /// `jobs[index]`.
    pub fn at<I: Operand>(self, index: I) -> RecordExpr
    where
        I::Value: Integral,
    {
        self.list().at(index)
    }
}

/// What `all`, `any`, `sum_over`, `count_over`, `size` and `count` range over.
pub trait Domain {
    /// What the comprehension's body receives.
    type Item;
    #[doc(hidden)]
    fn into_domain(self) -> DomainOps;
    #[doc(hidden)]
    fn item(ops: BindingOps) -> Self::Item;
}

/// The operations of a domain, with the name a user chose for its variable.
#[derive(Debug)]
pub struct DomainOps {
    ops: Vec<Op>,
    name: Option<&'static str>,
}

/// The operations of a comprehension variable.
#[derive(Debug)]
pub struct BindingOps(Vec<Op>);

impl DomainOps {
    fn unnamed(ops: Vec<Op>) -> Self {
        Self { ops, name: None }
    }
}

impl<V: ValueType, D: Depth, O> Domain for Field<V, D, O> {
    type Item = Expr<V, Open>;
    fn into_domain(self) -> DomainOps {
        DomainOps::unnamed(self.into_expr().ops)
    }
    fn item(binding: BindingOps) -> Self::Item {
        Expr::from_ops(binding.0)
    }
}

impl<V: ValueType, S: ExprShape> Domain for Expr<V, S> {
    type Item = Expr<V, Open>;
    fn into_domain(self) -> DomainOps {
        DomainOps::unnamed(self.ops)
    }
    fn item(binding: BindingOps) -> Self::Item {
        Expr::from_ops(binding.0)
    }
}

impl Domain for Record {
    type Item = RecordExpr;
    fn into_domain(self) -> DomainOps {
        DomainOps::unnamed(self.list().ops)
    }
    fn item(binding: BindingOps) -> RecordExpr {
        RecordExpr { ops: binding.0 }
    }
}

impl Domain for RecordList {
    type Item = RecordExpr;
    fn into_domain(self) -> DomainOps {
        DomainOps::unnamed(self.ops)
    }
    fn item(binding: BindingOps) -> RecordExpr {
        RecordExpr { ops: binding.0 }
    }
}

/// A domain with a chosen variable name; see [`over`].
#[derive(Debug)]
pub struct Over<T> {
    domain: T,
    name: Option<&'static str>,
}

/// A domain whose comprehension variable can be named for readable output.
pub fn over<T: Domain>(domain: T) -> Over<T> {
    Over { domain, name: None }
}

impl<T> Over<T> {
    /// Print the comprehension variable as `name`.
    pub fn named(mut self, name: &str) -> Self {
        self.name = Some(intern(name));
        self
    }
}

impl<T: Domain> Domain for Over<T> {
    type Item = T::Item;
    fn into_domain(self) -> DomainOps {
        DomainOps {
            name: self.name,
            ..self.domain.into_domain()
        }
    }
    fn item(binding: BindingOps) -> T::Item {
        T::item(binding)
    }
}

fn comprehension<T: Domain, B: Operand>(
    function: &'static str,
    domain: T,
    body: impl FnOnce(T::Item) -> B,
) -> Expr<Int> {
    let binding = BindingId::next();
    let DomainOps { mut ops, name } = domain.into_domain();
    ops.extend(
        body(T::item(BindingOps(vec![Op::Binding(binding)])))
            .into_expr()
            .ops,
    );
    ops.push(Op::Comprehension {
        function,
        binding,
        name,
    });
    Expr::from_ops(ops)
}

/// `all(x in domain: body)`.
pub fn all<T: Domain, B: Operand>(domain: T, body: impl FnOnce(T::Item) -> B) -> Expr<Int> {
    comprehension("all", domain, body)
}

/// `any(x in domain: body)`.
pub fn any<T: Domain, B: Operand>(domain: T, body: impl FnOnce(T::Item) -> B) -> Expr<Int> {
    comprehension("any", domain, body)
}

/// `sum(x in domain: body)`. Rust has no overloading, so this is not [`sum`].
pub fn sum_over<T: Domain, B: Operand>(domain: T, body: impl FnOnce(T::Item) -> B) -> Expr<Int> {
    comprehension("sum", domain, body)
}

/// `count(x in domain: body)`. Rust has no overloading, so this is not
/// [`count`].
pub fn count_over<T: Domain, B: Operand>(domain: T, body: impl FnOnce(T::Item) -> B) -> Expr<Int> {
    comprehension("count", domain, body)
}

fn call(function: &'static str, mut ops: Vec<Op>, arguments: usize) -> Expr<Int> {
    ops.push(Op::Call {
        function,
        arguments,
    });
    Expr::from_ops(ops)
}

/// `size(domain)`.
pub fn size<T: Domain>(domain: T) -> Expr<Int> {
    call("size", domain.into_domain().ops, 1)
}

/// `count(domain)`.
pub fn count<T: Domain>(domain: T) -> Expr<Int> {
    call("count", domain.into_domain().ops, 1)
}

/// The arguments of `min`, `max`, `sum` and `distinct`: one operand, or an
/// array, `Vec` or tuple of them.
pub trait Args {
    #[doc(hidden)]
    fn into_arguments(self) -> Arguments;
}

/// The operations of a call's arguments, one after the other.
#[derive(Debug, Default)]
pub struct Arguments {
    ops: Vec<Op>,
    count: usize,
}

impl Arguments {
    fn push<E: Operand>(&mut self, argument: E) {
        self.ops.extend(argument.into_expr().ops);
        self.count += 1;
    }
}

impl<E: Operand> Args for E {
    fn into_arguments(self) -> Arguments {
        let mut arguments = Arguments::default();
        arguments.push(self);
        arguments
    }
}

impl<E: Operand, const N: usize> Args for [E; N] {
    fn into_arguments(self) -> Arguments {
        let mut arguments = Arguments::default();
        self.into_iter()
            .for_each(|argument| arguments.push(argument));
        arguments
    }
}

impl<E: Operand> Args for Vec<E> {
    fn into_arguments(self) -> Arguments {
        let mut arguments = Arguments::default();
        self.into_iter()
            .for_each(|argument| arguments.push(argument));
        arguments
    }
}

macro_rules! tuple_arguments {
    ($(($($name:ident),+))*) => {$(
        impl<$($name: Operand),+> Args for ($($name,)+) {
            #[allow(non_snake_case)]
            fn into_arguments(self) -> Arguments {
                let ($($name,)+) = self;
                let mut arguments = Arguments::default();
                $(arguments.push($name);)+
                arguments
            }
        }
    )*};
}
tuple_arguments! {
    (A, B)
    (A, B, C)
    (A, B, C, E)
    (A, B, C, E, F)
    (A, B, C, E, F, G)
    (A, B, C, E, F, G, H)
    (A, B, C, E, F, G, H, I)
}

fn aggregate(function: &'static str, arguments: impl Args) -> Expr<Int> {
    let Arguments { ops, count } = arguments.into_arguments();
    call(function, ops, count)
}

/// `min(a, b, ...)`.
pub fn min(arguments: impl Args) -> Expr<Int> {
    aggregate("min", arguments)
}

/// `max(a, b, ...)`.
pub fn max(arguments: impl Args) -> Expr<Int> {
    aggregate("max", arguments)
}

/// `sum(a, b, ...)`.
pub fn sum(arguments: impl Args) -> Expr<Int> {
    aggregate("sum", arguments)
}

/// `distinct(a, b, ...)`.
pub fn distinct(arguments: impl Args) -> Expr<Int> {
    aggregate("distinct", arguments)
}

/// What `defined` accepts: a handle, a record, a record list, or a
/// comprehension variable.
pub trait Definable {
    #[doc(hidden)]
    fn name_ops(self) -> NameOps;
}

/// The operations of a name.
#[derive(Debug)]
pub struct NameOps(Vec<Op>);

impl<V: ValueType, D: Depth, O> Definable for Field<V, D, O> {
    fn name_ops(self) -> NameOps {
        NameOps(self.into_expr().ops)
    }
}
impl<V, S> Definable for Expr<V, S> {
    fn name_ops(self) -> NameOps {
        NameOps(self.ops)
    }
}
impl Definable for Record {
    /// `defined(jobs)`: a single record is not a value, so this is the list.
    fn name_ops(self) -> NameOps {
        NameOps(self.list().ops)
    }
}
impl Definable for RecordExpr {
    fn name_ops(self) -> NameOps {
        NameOps(self.ops)
    }
}
impl Definable for RecordList {
    fn name_ops(self) -> NameOps {
        NameOps(self.ops)
    }
}

/// `defined(name)`.
pub fn defined(name: impl Definable) -> Expr<Int> {
    call("defined", name.name_ops().0, 1)
}
