//! Rust expressions for postfix Iris expressions.
//!
//! The typed builder API decides what each operand may be, so every value on
//! the stack carries its [`Class`], and an operand that a method refuses goes
//! through `.dynamic()`. An expression that would be too wide for one line is
//! split with `let eN = ...;` statements, which also bounds how deep any one
//! Rust expression gets.
use super::shape::{self, Shape, ShapeId};
use super::{CALL_WIDTH, LINE_WIDTH, Line, Renderer, TAB, quoted};
use crate::program::{
    BinaryOp, ComprehensionKind, Expr, ExprOp, Function, Host, UnaryOp, VariableId,
};

/// What the Rust type of a value allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Class {
    /// A `DynField`, which has no `.get`.
    Handle,
    /// An `Expr<Dyn, _>`.
    Dyn,
    /// An `Expr<Int>` or an integer literal.
    Int,
    /// An `Expr<Float>` or a float literal.
    Float,
    /// An `Expr<Str>` or a string literal.
    Str,
    /// A host constant.
    Host,
    /// A `Record`, which has no `.dynamic`.
    Record,
    /// A `RecordExpr`.
    RecordItem,
    /// A `RecordList`.
    RecordList,
    /// Something only other builder calls take, such as a reading type.
    Other,
}

/// Rust's binding strengths, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Precedence {
    Range,
    Or,
    And,
    Sum,
    Product,
    Unary,
    Postfix,
}

/// A rendered value.
#[derive(Debug, Clone)]
pub(super) struct Value {
    pub(super) text: String,
    pub(super) class: Class,
    pub(super) precedence: Precedence,
    /// A bare Rust literal, an operand as it is and a receiver through `lit`.
    pub(super) literal: bool,
    /// What `clippy::eq_op` compares, if the text is free of calls.
    pub(super) shape: Option<ShapeId>,
}

impl Value {
    pub(super) fn new(text: String, class: Class) -> Self {
        Self {
            text,
            class,
            precedence: Precedence::Postfix,
            literal: false,
            shape: None,
        }
    }

    fn literal(text: String, class: Class) -> Self {
        Self {
            literal: true,
            ..Self::new(text, class)
        }
    }

    fn with_shape(self, shape: ShapeId) -> Self {
        Self {
            shape: Some(shape),
            ..self
        }
    }

    /// The shape of this value where a literal goes through `lit`, a call.
    fn receiver_shape(&self) -> Option<ShapeId> {
        self.shape.filter(|_| !self.literal)
    }

    /// The text as the left side of `level`, or as a receiver at `Postfix`.
    pub(super) fn bound(&self, level: Precedence) -> String {
        if self.literal {
            format!("lit({})", self.text)
        } else if self.precedence < level {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }

    /// The text as the left operand of a left-associative `operator`.
    fn left_of(&self, operator: Precedence) -> String {
        if self.is_arithmetic_beside_bitwise(operator) {
            format!("({})", self.text)
        } else {
            self.bound(operator)
        }
    }

    /// The text as the right operand of a left-associative `operator`, where a
    /// literal needs no `lit`.
    fn right_of(&self, operator: Precedence) -> String {
        let weak = !self.literal && self.precedence <= operator;
        if weak || self.is_arithmetic_beside_bitwise(operator) {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }

    /// `clippy::precedence` wants this in parentheses.
    fn is_arithmetic_beside_bitwise(&self, operator: Precedence) -> bool {
        operator <= Precedence::And
            && matches!(self.precedence, Precedence::Sum | Precedence::Product)
    }

    /// This operand, kept away from lints that evaluate a bare literal beside
    /// `op`: `x & 0` is `clippy::erasing_op`, `x * 1` is `clippy::identity_op`.
    fn evaluated_by_clippy(self, op: BinaryOp) -> Self {
        let identity = matches!(
            op,
            BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Remainder
        );
        if self.literal && (self.text == "0" || (self.text == "1" && identity)) {
            Self::new(self.receiver(), self.class)
        } else {
            self
        }
    }

    /// The text as the receiver of a method.
    pub(super) fn receiver(&self) -> String {
        self.bound(Precedence::Postfix)
    }

    /// This value with no static type.
    pub(super) fn dynamic(&self) -> Self {
        Self::new(format!("{}.dynamic()", self.receiver()), Class::Dyn)
    }

    fn is_integral(&self) -> bool {
        !matches!(self.class, Class::Float | Class::Str)
    }

    /// This value where an integral operand is needed.
    fn integral(&self) -> Self {
        if self.is_integral() {
            self.clone()
        } else {
            self.dynamic()
        }
    }

    /// This value where a numeric operand is needed.
    fn numeric(&self) -> Self {
        if self.class == Class::Str {
            self.dynamic()
        } else {
            self.clone()
        }
    }

    /// This value as a comprehension's or a function's domain.
    fn domain(&self) -> Self {
        match self.class {
            Class::Handle | Class::Dyn | Class::Record | Class::RecordList => self.clone(),
            _ => self.dynamic(),
        }
    }

    /// The receiver for `.at` and the class of the result.
    fn indexable(&self) -> Self {
        match self.class {
            Class::Handle | Class::Dyn | Class::Record | Class::RecordItem | Class::RecordList => {
                self.clone()
            }
            _ => self.dynamic(),
        }
    }

    /// The receiver for `.get`.
    fn has_fields(&self) -> Self {
        match self.class {
            Class::Dyn | Class::Record | Class::RecordItem => self.clone(),
            _ => self.dynamic(),
        }
    }

    fn operator_result(text: String, precedence: Precedence, class: Class) -> Self {
        Self {
            precedence,
            ..Self::new(text, class)
        }
    }
}

/// An open comprehension: its domain is rendered, its body is being rendered.
struct OpenComprehension {
    variable: VariableId,
    domain: Value,
    /// The statements before the comprehension, set aside for the body's own.
    outer: Vec<Line>,
}

/// A comprehension variable in scope.
struct Binding {
    variable: VariableId,
    class: Class,
    used: bool,
}

/// The state of one expression's rendering.
#[derive(Default)]
struct Evaluation {
    values: Vec<Value>,
    comprehensions: Vec<OpenComprehension>,
    bindings: Vec<Binding>,
}

impl Evaluation {
    fn pop(&mut self) -> Value {
        self.values.pop().expect("postfix operations are balanced")
    }

    fn pop_many(&mut self, count: usize) -> Vec<Value> {
        let start = self.values.len() - count;
        self.values.split_off(start)
    }
}

fn operator(op: BinaryOp) -> Option<(&'static str, Precedence)> {
    Some(match op {
        BinaryOp::Or => ("|", Precedence::Or),
        BinaryOp::And => ("&", Precedence::And),
        BinaryOp::Add => ("+", Precedence::Sum),
        BinaryOp::Subtract => ("-", Precedence::Sum),
        BinaryOp::Multiply => ("*", Precedence::Product),
        BinaryOp::Divide => ("/", Precedence::Product),
        BinaryOp::Remainder => ("%", Precedence::Product),
        _ => return None,
    })
}

/// The operands of an arithmetic or logical `op`, in the form its text needs.
fn operator_operands(op: BinaryOp, operands: &[Value]) -> (Value, Value) {
    let (left, right) = (operands[0].integral(), operands[1].integral());
    (left, right.evaluated_by_clippy(op))
}

fn comparison(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Equal => "eq",
        BinaryOp::NotEqual => "ne",
        BinaryOp::Less => "lt",
        BinaryOp::LessEqual => "le",
        BinaryOp::Greater => "gt",
        BinaryOp::GreaterEqual => "ge",
        _ => unreachable!("every other operator is a Rust operator"),
    }
}

fn function_name(function: Function) -> &'static str {
    match function {
        Function::Defined => "defined",
        Function::Size => "size",
        Function::Count => "count",
        Function::Min => "min",
        Function::Max => "max",
        Function::Sum => "sum",
        Function::Distinct => "distinct",
    }
}

fn comprehension_name(kind: ComprehensionKind) -> &'static str {
    match kind {
        ComprehensionKind::All => "all",
        ComprehensionKind::Any => "any",
        ComprehensionKind::Sum => "sum_over",
        ComprehensionKind::Count => "count_over",
    }
}

/// Whether `value` is close to a constant `clippy::approx_constant` knows.
fn near_math_constant(value: f64) -> bool {
    use std::f64::consts as c;
    const TOLERANCE: f64 = 0.01;
    [
        c::E,
        c::FRAC_1_PI,
        c::FRAC_1_SQRT_2,
        c::FRAC_2_PI,
        c::FRAC_2_SQRT_PI,
        c::FRAC_PI_2,
        c::FRAC_PI_3,
        c::FRAC_PI_4,
        c::FRAC_PI_6,
        c::FRAC_PI_8,
        c::LN_2,
        c::LN_10,
        c::LOG2_10,
        c::LOG2_E,
        c::LOG10_2,
        c::LOG10_E,
        c::PI,
        c::SQRT_2,
        c::TAU,
    ]
    .iter()
    .any(|constant| (value - constant).abs() < TOLERANCE)
}

/// Operands shorter than this gain nothing from a `let` of their own.
const SHORTEST_HOIST: usize = 12;

impl Renderer<'_> {
    /// Render `expr`, which may emit `let` statements first.
    pub(super) fn expr(&mut self, expr: Expr) -> Value {
        let mut state = Evaluation::default();
        let ops = self.program.ops(expr);
        for (index, op) in ops.iter().enumerate() {
            let is_body = matches!(ops.get(index + 1), Some(ExprOp::Comprehension(_)));
            self.step(op, is_body, &mut state);
        }
        state.pop()
    }

    /// Apply `op`. `is_body` says that its result ends a comprehension's body.
    fn step(&mut self, op: &ExprOp, is_body: bool, state: &mut Evaluation) {
        let value = match op {
            ExprOp::Int(value) => Value::literal(integer_literal(*value), Class::Int)
                .with_shape(self.shapes.intern(Shape::Int(*value))),
            ExprOp::Float(value) => {
                self.lints.approx_constant |= near_math_constant(*value);
                Value::literal(format!("{value:?}"), Class::Float)
            }
            ExprOp::Str(text) => Value::literal(quoted(text), Class::Str),
            ExprOp::Host(host) => {
                let text = match host {
                    Host::IntMax => "gecode::INT_MAX",
                    Host::IntMin => "gecode::INT_MIN",
                };
                Value::new(text.to_owned(), Class::Host)
                    .with_shape(self.shapes.intern(Shape::Host(*host)))
            }
            ExprOp::Name(name) => Value::new(self.value_names[name.0].clone(), Class::Handle)
                .with_shape(self.shapes.intern(Shape::Name(*name))),
            ExprOp::Var(variable) => self.variable(*variable, state),
            ExprOp::Each(record) => Value::new(self.record_names[record.0].clone(), Class::Record)
                .with_shape(self.shapes.intern(Shape::Each(*record))),
            ExprOp::List(record) => Value::new(
                format!("{}.list()", self.record_names[record.0]),
                Class::RecordList,
            ),
            ExprOp::Unary(op) => {
                let operand = state.pop();
                self.unary(*op, operand)
            }
            ExprOp::Binary(op) => {
                let right = state.pop();
                let left = state.pop();
                self.binary(*op, left, right)
            }
            ExprOp::Power => {
                let right = state.pop();
                let left = state.pop();
                self.combine(vec![left, right], |values| {
                    let (left, right) = (values[0].integral(), values[1].integral());
                    Value::new(
                        format!("{}.pow({})", left.receiver(), right.text),
                        Class::Int,
                    )
                })
            }
            ExprOp::Index => {
                let index = state.pop();
                let base = state.pop();
                self.combine(vec![base, index], |values| {
                    let (base, index) = (values[0].indexable(), values[1].integral());
                    let class = match base.class {
                        Class::Record | Class::RecordItem | Class::RecordList => Class::RecordItem,
                        _ => Class::Dyn,
                    };
                    Value::new(format!("{}.at({})", base.receiver(), index.text), class)
                })
            }
            ExprOp::Member(field) => {
                let base = state.pop();
                let field = Value::new(self.value_names[field.0].clone(), Class::Handle);
                self.combine(vec![base, field], |values| {
                    let base = values[0].has_fields();
                    let text = format!("{}.get({})", base.receiver(), values[1].text);
                    Value::new(text, Class::Dyn)
                })
            }
            ExprOp::ListMember(record) => {
                let base = state.pop();
                let inner = Value::new(self.record_names[record.0].clone(), Class::Record);
                self.combine(vec![base, inner], |values| {
                    let text = format!("{}.records({})", values[0].receiver(), values[1].text);
                    Value::new(text, Class::RecordList)
                })
            }
            ExprOp::Call {
                function,
                arguments,
            } => {
                let arguments = state.pop_many(*arguments);
                self.function_call(*function, arguments)
            }
            ExprOp::ComprehensionBody(variable) => {
                let domain = state.pop();
                self.open_comprehension(*variable, domain, state);
                return;
            }
            ExprOp::Comprehension(kind) => {
                let body = state.pop();
                self.close_comprehension(*kind, body, is_body, state)
            }
        };
        state.values.push(value);
    }

    fn variable(&mut self, variable: VariableId, state: &mut Evaluation) -> Value {
        let binding = state
            .bindings
            .iter_mut()
            .rev()
            .find(|binding| binding.variable == variable)
            .expect("a checked variable is bound by a comprehension");
        binding.used = true;
        Value::new(
            format!("{}.clone()", self.variable_names[variable.0]),
            binding.class,
        )
    }

    fn unary(&mut self, op: UnaryOp, operand: Value) -> Value {
        let mut operands = [operand];
        let budget = self.budget();
        let value = self.fit(&mut operands, budget, |values| match op {
            UnaryOp::Minus => {
                let operand = values[0].numeric();
                // `--x` is a lint, and `-(-x)` is the same expression.
                let level = if operand.precedence == Precedence::Unary {
                    Precedence::Postfix
                } else {
                    Precedence::Unary
                };
                let class = match operand.class {
                    Class::Handle => Class::Dyn,
                    Class::Host => Class::Int,
                    class => class,
                };
                let text = format!("-{}", operand.bound(level));
                Value::operator_result(text, Precedence::Unary, class)
            }
            UnaryOp::Not => {
                let operand = values[0].integral();
                let text = format!("!{}", operand.bound(Precedence::Unary));
                Value::operator_result(text, Precedence::Unary, Class::Int)
            }
        });
        let [operand] = operands;
        Value {
            shape: operand
                .receiver_shape()
                .map(|operand| self.shapes.intern(Shape::Unary(op, operand))),
            ..value
        }
    }

    fn binary(&mut self, op: BinaryOp, left: Value, right: Value) -> Value {
        if let Some((symbol, precedence)) = operator(op) {
            let mut operands = [left, right];
            let budget = self.budget();
            let value = self.fit(&mut operands, budget, |values| {
                let (left, right) = operator_operands(op, values);
                let left = left.left_of(precedence);
                let right = right.right_of(precedence);
                let text = format!("{left} {symbol} {right}");
                Value::operator_result(text, precedence, Class::Int)
            });
            let (left, right) = operator_operands(op, &operands);
            return Value {
                shape: self.operator_shape(op, left.receiver_shape(), right.shape),
                ..value
            };
        }
        let name = comparison(op);
        let equality = matches!(op, BinaryOp::Equal | BinaryOp::NotEqual);
        self.combine(vec![left, right], |values| {
            let (mut left, mut right) = (values[0].clone(), values[1].clone());
            if equality {
                let mixed = (left.class == Class::Str) != (right.class == Class::Str);
                let open = |value: &Value| matches!(value.class, Class::Handle | Class::Dyn);
                if mixed && !open(&left) && !open(&right) {
                    left = left.dynamic();
                }
            } else {
                left = left.numeric();
                right = right.numeric();
            }
            Value::new(
                format!("{}.{name}({})", left.receiver(), right.text),
                Class::Int,
            )
        })
    }

    /// The shape of `left op right`, which also tells whether the operands are
    /// ones that `clippy::eq_op` refuses.
    fn operator_shape(
        &mut self,
        op: BinaryOp,
        left: Option<ShapeId>,
        right: Option<ShapeId>,
    ) -> Option<ShapeId> {
        let (left, right) = (left?, right?);
        self.lints.eq_op |= left == right && shape::refused_with_equal_operands(op);
        Some(self.shapes.intern(Shape::Binary(op, left, right)))
    }

    fn function_call(&mut self, function: Function, arguments: Vec<Value>) -> Value {
        let name = function_name(function);
        match function {
            Function::Defined => self.combine(arguments, |values| {
                Value::new(format!("{name}({})", values[0].text), Class::Int)
            }),
            Function::Size | Function::Count => self.combine(arguments, |values| {
                Value::new(format!("{name}({})", values[0].domain().text), Class::Int)
            }),
            Function::Min | Function::Max | Function::Sum | Function::Distinct => {
                let width = self.budget().saturating_sub(name.len() + "()".len());
                let argument = if let [only] = arguments.as_slice() {
                    self.narrow(only.clone(), width).text
                } else {
                    let items = arguments
                        .into_iter()
                        .map(|value| {
                            let dynamic = |values: &[Value]| values[0].dynamic();
                            self.combine(vec![value], dynamic).text
                        })
                        .collect();
                    self.array(items, width)
                };
                Value::new(format!("{name}({argument})"), Class::Int)
            }
        }
    }

    fn open_comprehension(&mut self, variable: VariableId, domain: Value, state: &mut Evaluation) {
        let spelling = self.program.variables[variable.0].clone();
        let class = match domain.class {
            Class::Record | Class::RecordList => Class::RecordItem,
            _ => Class::Dyn,
        };
        let spelling = Value::new(quoted(&spelling), Class::Str);
        let domain = self.combine(vec![domain, spelling], |values| {
            let text = format!(
                "over({}).named({})",
                values[0].domain().text,
                values[1].text
            );
            Value::new(text, Class::Other)
        });
        state.comprehensions.push(OpenComprehension {
            variable,
            domain,
            outer: std::mem::take(&mut self.lines),
        });
        state.bindings.push(Binding {
            variable,
            class,
            used: false,
        });
        self.indent += 1;
    }

    fn close_comprehension(
        &mut self,
        kind: ComprehensionKind,
        body: Value,
        is_body: bool,
        state: &mut Evaluation,
    ) -> Value {
        let open = state
            .comprehensions
            .pop()
            .expect("a body is inside a comprehension");
        let binding = state.bindings.pop().expect("a body has its binding");
        let body_lines = std::mem::replace(&mut self.lines, open.outer);
        self.indent -= 1;
        let parameter = if binding.used {
            self.variable_names[open.variable.0].clone()
        } else {
            "_".to_owned()
        };
        let name = comprehension_name(kind);
        let domain = self.narrow(open.domain, self.domain_width(name, &parameter));
        let head = format!("{name}({}, |{parameter}|", domain.text);
        let one_line = format!("{head} {})", body.text);
        let narrow = one_line.len() - name.len() - "()".len() <= CALL_WIDTH;
        // The end of a block has no `let` around it, so it has more room.
        let fits_block_end = is_body && narrow && self.fits(&one_line);
        if body_lines.is_empty() && (one_line.len() <= self.budget() || fits_block_end) {
            return Value::new(one_line, Class::Int);
        }
        let temporary = self.temporary();
        if body_lines.is_empty() {
            let head = format!("let {temporary}");
            let statement = format!("{head} = {one_line};");
            if narrow && (self.fits(&statement) || self.fits_next_line(&format!("{one_line};"))) {
                self.let_statement(&head, &one_line);
                return Value::new(temporary, Class::Int);
            }
            // rustfmt puts no block around a literal.
            if body.literal {
                self.line(format!("let {temporary} = {name}("));
                self.indent += 1;
                self.line(format!("{},", domain.text));
                self.line(format!("|{parameter}| {},", body.text));
                self.indent -= 1;
                self.line(");".to_owned());
                return Value::new(temporary, Class::Int);
            }
        }
        self.line(format!("let {temporary} = {head} {{"));
        self.lines.extend(body_lines);
        self.finish_block_value(body);
        self.line("});".to_owned());
        Value::new(temporary, Class::Int)
    }

    /// The width that keeps `let eN = name(domain, |parameter| {` on one line.
    fn domain_width(&self, name: &str, parameter: &str) -> usize {
        // The domain's own `let` may come first.
        let temporary = self.temporaries + 2;
        let closure = format!("|{parameter}| {{");
        // rustfmt keeps room on the first line for the `);` of the last line.
        let first_line = format!("let e{temporary} = {name}(, {closure});");
        let line = LINE_WIDTH.saturating_sub(self.indent * TAB + first_line.len());
        line.min(CALL_WIDTH.saturating_sub(", ".len() + closure.len()))
    }

    /// End a closure's block with `value`. A `let` of that value just before it
    /// would be `clippy::let_and_return`, so that statement becomes the value.
    fn finish_block_value(&mut self, value: Value) {
        let start = format!("let {} = ", value.text);
        let position = self
            .lines
            .iter()
            .rposition(|line| line.text.starts_with(&start));
        if let Some(position) = position {
            let text = &mut self.lines[position].text;
            *text = text[start.len()..].to_owned();
            let last = self.lines.last_mut().expect("the statement has lines");
            last.text.pop();
            return;
        }
        self.indent += 1;
        self.line(value.text);
        self.indent -= 1;
    }

    /// Build a value from `operands`, moving the widest ones into `let`
    /// statements while the result is wider than a line allows.
    pub(super) fn combine(
        &mut self,
        mut operands: Vec<Value>,
        build: impl Fn(&[Value]) -> Value,
    ) -> Value {
        self.fit(&mut operands, self.budget(), build)
    }

    /// Build a value no wider than `width` if `let` statements can make it so,
    /// and leave each operand as the value uses it.
    pub(super) fn fit(
        &mut self,
        operands: &mut [Value],
        width: usize,
        build: impl Fn(&[Value]) -> Value,
    ) -> Value {
        loop {
            let value = build(operands);
            if value.text.len() <= width {
                return value;
            }
            let widest = operands
                .iter_mut()
                .filter(|operand| operand.text.len() >= SHORTEST_HOIST)
                .max_by_key(|operand| operand.text.len());
            let Some(widest) = widest else {
                return value;
            };
            self.hoist(widest);
        }
    }

    /// `value`, through a `let` if it is wider than `width`.
    pub(super) fn narrow(&mut self, mut value: Value, width: usize) -> Value {
        if value.text.len() > width && value.text.len() >= SHORTEST_HOIST {
            self.hoist(&mut value);
        }
        value
    }

    /// Move `value` into a `let` statement and leave its name in its place.
    fn hoist(&mut self, value: &mut Value) {
        let name = self.temporary();
        let hoisted = std::mem::replace(&mut value.text, name);
        value.precedence = Precedence::Postfix;
        value.shape = None;
        self.let_statement(&format!("let {}", value.text), &hoisted);
    }
}

fn integer_literal(value: i64) -> String {
    if i32::try_from(value).is_ok() {
        value.to_string()
    } else {
        format!("{value}_i64")
    }
}
