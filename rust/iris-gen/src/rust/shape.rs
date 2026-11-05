//! Which operands `clippy::eq_op` takes to be the same.
//!
//! The lint compares the operands of an operator as syntax trees, ignoring
//! parentheses. It keeps the order of operands of a type of ours, and it never
//! takes a call to be equal to anything, because a call may have side effects.
//! So a value has a shape only while its Rust text is free of calls, and equal
//! shapes get one [`ShapeId`] from the postfix expression, however deep the
//! value is.
use std::collections::HashMap;

use crate::program::{BinaryOp, Host, NameId, RecordId, UnaryOp};

/// Shapes with the same id are the same to `clippy::eq_op`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ShapeId(usize);

#[derive(Debug, PartialEq, Eq, Hash)]
pub(super) enum Shape {
    Int(i64),
    Host(Host),
    Name(NameId),
    Each(RecordId),
    Unary(UnaryOp, ShapeId),
    Binary(BinaryOp, ShapeId, ShapeId),
}

/// The shapes of one program.
#[derive(Debug, Default)]
pub(super) struct Shapes {
    ids: HashMap<Shape, ShapeId>,
}

impl Shapes {
    pub(super) fn intern(&mut self, shape: Shape) -> ShapeId {
        let next = ShapeId(self.ids.len());
        *self.ids.entry(shape).or_insert(next)
    }
}

/// Whether `clippy::eq_op` refuses `op` between equal operands. `+`, `*` and `%`
/// mean something between equal operands, so it leaves them alone.
pub(super) fn refused_with_equal_operands(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Or | BinaryOp::And | BinaryOp::Subtract | BinaryOp::Divide
    )
}
