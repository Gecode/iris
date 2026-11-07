//! Python expressions for postfix Iris expressions.
//!
//! The stubs of `gecode_iris.build` decide what each operand may be, so every
//! value on the stack carries its [`Class`], and an operand that an operator
//! refuses goes through `.dynamic()`. A Python number or string on the left of
//! an operator goes through `ib.lit`, because Python would otherwise swap a
//! comparison to call the expression's reflected method.
use super::{Arg, HELPER_BODY_INDENT, LINE_WIDTH, MAX_INDENT, Renderer, TAB, quoted};
use crate::program::{
    BinaryOp, ComprehensionKind, Expr, ExprOp, Function, Host, UnaryOp, VariableId,
};

/// What the Python type of a value allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Class {
    /// A `DynField` handle.
    Handle,
    /// A `DynExpr`.
    Dyn,
    /// An `IntExpr` or an `int`.
    Int,
    /// A `FloatExpr` or a `float`.
    Float,
    /// A `StrExpr` or a `str`.
    Str,
    /// A `Record`, which has no `.dynamic`.
    Record,
    /// A `RecordExpr`.
    RecordItem,
    /// A `RecordList`.
    RecordList,
    /// Something only other builder calls take, such as a reading type.
    Other,
}

/// Python's binding strengths, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Precedence {
    Lambda,
    Comparison,
    Or,
    And,
    Sum,
    Product,
    Unary,
    Postfix,
}

/// How `ruff format` may split a value that is too wide for its line.
#[derive(Debug, Clone)]
pub(super) enum Form {
    /// A literal or attribute, which no layout can split.
    Atom,
    /// A plain identifier, which no layout can split.
    Name,
    /// `callee(arguments)`, which splits at its parentheses.
    Call { callee: String, arguments: Vec<Arg> },
    /// `(items)`, which splits one item to a line.
    Tuple(Vec<Value>),
    /// Anything else, which must fit its line.
    Other,
}

/// A rendered value.
#[derive(Debug, Clone)]
pub(super) struct Value {
    pub(super) text: String,
    pub(super) class: Class,
    pub(super) precedence: Precedence,
    /// A Python `int`, `float` or `str`, or a temporary that holds one: an
    /// operand as it is, and a receiver or left operand through `ib.lit`.
    pub(super) literal: bool,
    pub(super) form: Form,
}

impl Value {
    pub(super) fn new(text: String, class: Class) -> Self {
        Self {
            text,
            class,
            precedence: Precedence::Postfix,
            literal: false,
            form: Form::Other,
        }
    }

    pub(super) fn atom(text: String, class: Class) -> Self {
        Self {
            form: Form::Atom,
            ..Self::new(text, class)
        }
    }

    pub(super) fn name(text: String, class: Class) -> Self {
        Self {
            form: Form::Name,
            ..Self::new(text, class)
        }
    }

    fn literal(text: String, class: Class) -> Self {
        Self {
            literal: true,
            ..Self::atom(text, class)
        }
    }

    /// `callee(arguments)`.
    pub(super) fn call(callee: &str, arguments: Vec<Arg>, class: Class) -> Self {
        let texts: Vec<String> = arguments.iter().map(Arg::text).collect();
        Self {
            form: Form::Call {
                callee: callee.to_owned(),
                arguments,
            },
            ..Self::new(format!("{callee}({})", texts.join(", ")), class)
        }
    }

    /// `callee(values)`, with positional arguments.
    pub(super) fn call_of(callee: &str, values: &[Value], class: Class) -> Self {
        Self::call(
            callee,
            values.iter().cloned().map(Arg::new).collect(),
            class,
        )
    }

    /// `(items)`, with the trailing comma of a single item.
    pub(super) fn tuple(items: Vec<Value>) -> Self {
        let texts: Vec<&str> = items.iter().map(|item| item.text.as_str()).collect();
        let text = match texts.as_slice() {
            [only] => format!("({only},)"),
            _ => format!("({})", texts.join(", ")),
        };
        Self {
            form: Form::Tuple(items),
            ..Self::new(text, Class::Other)
        }
    }

    fn operator_result(text: String, precedence: Precedence, class: Class) -> Self {
        Self {
            precedence,
            ..Self::new(text, class)
        }
    }

    /// Whether no layout can split the value, so it may run past the line.
    pub(super) fn is_unsplittable(&self) -> bool {
        matches!(self.form, Form::Atom | Form::Name)
    }

    /// The text as the left side of `level`, or as a receiver at `Postfix`.
    pub(super) fn bound(&self, level: Precedence) -> String {
        if self.literal {
            format!("ib.lit({})", self.text)
        } else if self.precedence < level {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }

    /// The text as the right operand of a left-associative operator at `level`.
    fn right_of(&self, level: Precedence) -> String {
        if !self.literal && self.precedence <= level {
            format!("({})", self.text)
        } else {
            self.text.clone()
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

    /// This value as an `ib.lit` call if it is a Python literal.
    pub(super) fn expression(&self) -> Self {
        if self.literal {
            Self::call_of("ib.lit", std::slice::from_ref(&self.unlit()), self.class)
        } else {
            self.clone()
        }
    }

    /// The same text, no longer taken for a literal.
    fn unlit(&self) -> Self {
        Self {
            literal: false,
            ..self.clone()
        }
    }

    /// This value where an `int | IntExpr | DynExpr` is needed.
    pub(super) fn integral(&self) -> Self {
        if matches!(self.class, Class::Float | Class::Str) {
            self.dynamic()
        } else {
            self.clone()
        }
    }

    /// This value where a number or an expression of one is needed.
    fn numeric(&self) -> Self {
        if self.class == Class::Str {
            self.dynamic()
        } else {
            self.clone()
        }
    }

    /// This value where an `int | Expr` is needed: a condition or a body.
    pub(super) fn condition(&self) -> Self {
        if self.literal && matches!(self.class, Class::Float | Class::Str) {
            self.expression()
        } else {
            self.clone()
        }
    }

    /// This value as a comprehension's or a function's domain.
    fn domain(&self) -> Self {
        match self.class {
            Class::Handle | Class::Dyn | Class::Record | Class::RecordItem | Class::RecordList => {
                self.clone()
            }
            _ => self.dynamic(),
        }
    }

    /// The annotation of a function that returns this value as a body.
    fn annotation(&self) -> &'static str {
        if self.literal && self.class == Class::Int {
            "int"
        } else {
            "ib.Expr"
        }
    }
}

/// The annotation of a comprehension variable of this class.
fn variable_annotation(class: Class) -> &'static str {
    if class == Class::RecordItem {
        "ib.RecordExpr"
    } else {
        "ib.DynExpr"
    }
}

/// An open comprehension: its domain is rendered, its body is being rendered.
struct OpenComprehension {
    variable: VariableId,
    domain: Value,
    /// The statements before the comprehension, set aside for the body's own.
    outer: Vec<super::Line>,
    outer_indent: usize,
    /// The pending `with` statement of the outer block.
    pending: (Vec<super::PendingWith>, usize),
    /// Whether a function for the body goes to the top of `builder`.
    outlined: bool,
    /// The variables of enclosing comprehensions that the body uses.
    captures: Vec<VariableId>,
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
    /// One binding for each open comprehension, in the same order.
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

fn operator(op: BinaryOp) -> (&'static str, Precedence) {
    match op {
        BinaryOp::Or => ("|", Precedence::Or),
        BinaryOp::And => ("&", Precedence::And),
        BinaryOp::Add => ("+", Precedence::Sum),
        BinaryOp::Subtract => ("-", Precedence::Sum),
        BinaryOp::Multiply => ("*", Precedence::Product),
        BinaryOp::Equal => ("==", Precedence::Comparison),
        BinaryOp::NotEqual => ("!=", Precedence::Comparison),
        BinaryOp::Less => ("<", Precedence::Comparison),
        BinaryOp::LessEqual => ("<=", Precedence::Comparison),
        BinaryOp::Greater => (">", Precedence::Comparison),
        BinaryOp::GreaterEqual => (">=", Precedence::Comparison),
        BinaryOp::Divide => ("ib.div", Precedence::Postfix),
        BinaryOp::Remainder => ("ib.rem", Precedence::Postfix),
    }
}

fn function_name(function: Function) -> &'static str {
    match function {
        Function::Defined => "ib.defined",
        Function::Size => "ib.size",
        Function::Count => "ib.count",
        Function::Min => "ib.min",
        Function::Max => "ib.max",
        Function::Sum => "ib.sum",
        Function::Distinct => "ib.distinct",
    }
}

fn comprehension_name(kind: ComprehensionKind) -> &'static str {
    match kind {
        ComprehensionKind::All => "ib.all",
        ComprehensionKind::Any => "ib.any",
        ComprehensionKind::Sum => "ib.sum",
        ComprehensionKind::Count => "ib.count",
    }
}

/// A float as a Python literal that reads back as the same value.
fn float_literal(value: f64) -> String {
    format!("{value:?}")
}

/// Operands shorter than this gain nothing from a temporary of their own.
const SHORTEST_HOIST: usize = 12;

impl Renderer<'_> {
    /// Render `expr`, which may emit statements first.
    pub(super) fn expr(&mut self, expr: Expr) -> Value {
        let mut state = Evaluation::default();
        for op in self.program.ops(expr) {
            self.step(op, &mut state);
        }
        state.pop()
    }

    fn step(&mut self, op: &ExprOp, state: &mut Evaluation) {
        let value = match op {
            ExprOp::Int(value) => Value::literal(value.to_string(), Class::Int),
            ExprOp::Float(value) => Value::literal(float_literal(*value), Class::Float),
            ExprOp::Str(text) => Value::literal(quoted(text), Class::Str),
            ExprOp::Host(host) => {
                let text = match host {
                    Host::IntMax => "ib.gecode.INT_MAX",
                    Host::IntMin => "ib.gecode.INT_MIN",
                };
                Value::atom(text.to_owned(), Class::Int)
            }
            ExprOp::Name(name) => Value::name(self.value_names[name.0].clone(), Class::Handle),
            ExprOp::Var(variable) => self.variable(*variable, state),
            ExprOp::Each(record) => Value::name(self.record_names[record.0].clone(), Class::Record),
            ExprOp::List(record) => Value::atom(
                format!("{}.list", self.record_names[record.0]),
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
                    let operands = [values[0].integral(), values[1].integral()];
                    Value::call_of("ib.pow", &operands, Class::Int)
                })
            }
            ExprOp::Index => {
                let index = state.pop();
                let base = state.pop();
                self.combine(vec![base, index], |values| {
                    let (base, index) = (&values[0], values[1].integral());
                    match base.class {
                        Class::Record | Class::RecordItem => {
                            let text = format!("{}.at({})", base.text, index.text);
                            Value::new(text, Class::RecordItem)
                        }
                        Class::RecordList => {
                            Value::new(format!("{}[{}]", base.text, index.text), Class::RecordItem)
                        }
                        Class::Handle | Class::Dyn => {
                            Value::new(format!("{}[{}]", base.receiver(), index.text), Class::Dyn)
                        }
                        _ => {
                            let base = base.dynamic();
                            Value::new(format!("{}[{}]", base.text, index.text), Class::Dyn)
                        }
                    }
                })
            }
            ExprOp::Member(field) => {
                let base = state.pop();
                let field = Value::name(self.value_names[field.0].clone(), Class::Handle);
                self.combine(vec![base, field], |values| {
                    let (base, field) = (&values[0], &values[1]);
                    let text = match base.class {
                        Class::Record | Class::RecordItem => {
                            format!("{}[{}]", base.text, field.text)
                        }
                        Class::Handle | Class::Dyn => {
                            format!("{}.field({})", base.receiver(), field.text)
                        }
                        _ => format!("{}.field({})", base.dynamic().text, field.text),
                    };
                    Value::new(text, Class::Dyn)
                })
            }
            ExprOp::ListMember(record) => {
                let base = state.pop();
                let inner = Value::name(self.record_names[record.0].clone(), Class::Record);
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
                self.close_comprehension(*kind, body, state)
            }
        };
        state.values.push(value);
    }

    fn variable(&mut self, variable: VariableId, state: &mut Evaluation) -> Value {
        let position = state
            .bindings
            .iter()
            .rposition(|binding| binding.variable == variable)
            .expect("a checked variable is bound by a comprehension");
        let binding = &mut state.bindings[position];
        binding.used = true;
        let class = binding.class;
        // Each comprehension inside the binding one needs the variable from
        // outside if its body becomes a function of its own.
        for open in &mut state.comprehensions[position + 1..] {
            if !open.captures.contains(&variable) {
                open.captures.push(variable);
            }
        }
        Value::name(self.variable_names[variable.0].clone(), class)
    }

    fn unary(&mut self, op: UnaryOp, operand: Value) -> Value {
        self.combine(vec![operand], |values| match op {
            UnaryOp::Minus => {
                let mut operand = values[0].numeric();
                // Ruff's B002 takes `-(-x)` for a mistaken `--x`.
                if operand.precedence == Precedence::Unary && operand.text.starts_with('-') {
                    operand = operand.dynamic();
                }
                let level = Precedence::Unary;
                let class = match operand.class {
                    Class::Handle => Class::Dyn,
                    class => class,
                };
                let text = format!("-{}", operand.bound(level));
                Value::operator_result(text, Precedence::Unary, class)
            }
            UnaryOp::Not => {
                let operand = values[0].integral();
                let text = format!("~{}", operand.bound(Precedence::Unary));
                Value::operator_result(text, Precedence::Unary, Class::Int)
            }
        })
    }

    fn binary(&mut self, op: BinaryOp, left: Value, right: Value) -> Value {
        let (symbol, precedence) = operator(op);
        self.combine(vec![left, right], |values| {
            let (mut left, mut right) = (values[0].clone(), values[1].clone());
            match op {
                BinaryOp::Divide | BinaryOp::Remainder => {
                    let operands = [left.integral(), right.integral()];
                    return Value::call_of(symbol, &operands, Class::Int);
                }
                BinaryOp::Equal | BinaryOp::NotEqual => {
                    let mixed = (left.class == Class::Str) != (right.class == Class::Str);
                    let open = |value: &Value| matches!(value.class, Class::Handle | Class::Dyn);
                    if mixed && !open(&left) && !open(&right) {
                        left = left.dynamic();
                    }
                }
                BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual => {
                    left = left.numeric();
                    right = right.numeric();
                }
                _ => {
                    left = left.integral();
                    right = right.integral();
                }
            }
            // Ruff's PLR0124 takes a name compared with itself for a mistake.
            let same_name = matches!(left.form, Form::Name) && left.text == right.text;
            if precedence == Precedence::Comparison && same_name && !left.literal {
                left = left.dynamic();
            }
            let text = if precedence == Precedence::Comparison {
                // Python chains comparisons, so a comparison operand needs
                // parentheses on either side.
                let left = left.bound(Precedence::Or);
                format!("{left} {symbol} {}", right.right_of(precedence))
            } else {
                let left = left.bound(precedence);
                format!("{left} {symbol} {}", right.right_of(precedence))
            };
            Value::operator_result(text, precedence, Class::Int)
        })
    }

    fn function_call(&mut self, function: Function, arguments: Vec<Value>) -> Value {
        let name = function_name(function);
        self.combine(arguments, |values| match function {
            Function::Defined => Value::call_of(name, values, Class::Int),
            Function::Size | Function::Count => {
                Value::call_of(name, &[values[0].domain()], Class::Int)
            }
            Function::Min | Function::Max | Function::Sum | Function::Distinct => {
                let values: Vec<Value> = values.iter().map(Value::condition).collect();
                Value::call_of(name, &values, Class::Int)
            }
        })
    }

    fn open_comprehension(&mut self, variable: VariableId, domain: Value, state: &mut Evaluation) {
        let spelling = self.program.variables[variable.0].clone();
        let class = match domain.class {
            Class::Record | Class::RecordList => Class::RecordItem,
            _ => Class::Dyn,
        };
        let spelling = Value::literal(quoted(&spelling), Class::Str);
        let domain = self.combine(vec![domain, spelling], |values| {
            let operands = [values[0].domain(), values[1].clone()];
            Value::call_of("ib.over", &operands, Class::Other)
        });
        let outlined = self.indent + 1 > MAX_INDENT;
        state.comprehensions.push(OpenComprehension {
            variable,
            domain,
            outer: std::mem::take(&mut self.lines),
            outer_indent: self.indent,
            pending: self.set_pending_aside(),
            outlined,
            captures: Vec::new(),
        });
        state.bindings.push(Binding {
            variable,
            class,
            used: false,
        });
        self.indent = if outlined {
            HELPER_BODY_INDENT
        } else {
            self.indent + 1
        };
    }

    fn close_comprehension(
        &mut self,
        kind: ComprehensionKind,
        body: Value,
        state: &mut Evaluation,
    ) -> Value {
        let open = state
            .comprehensions
            .pop()
            .expect("a body is inside a comprehension");
        let binding = state.bindings.pop().expect("a body has its binding");
        let body = body.condition();
        let parameter = if binding.used {
            self.variable_names[open.variable.0].clone()
        } else {
            "_".to_owned()
        };
        let name = comprehension_name(kind);
        let lambda = format!("lambda {parameter}: {}", body.text);
        let lambda_fits = (open.outer_indent + 1) * TAB + lambda.len() + ",".len() <= LINE_WIDTH;
        if self.lines.is_empty() && lambda_fits {
            self.lines = open.outer;
            self.indent = open.outer_indent;
            self.restore_pending(open.pending);
            // A lambda that fits its line needs no layout.
            let lambda = Value {
                precedence: Precedence::Lambda,
                ..Value::atom(lambda, Class::Other)
            };
            return Value::call_of(name, &[open.domain, lambda], Class::Int);
        }
        // The body needs statements, or a line of its own: it becomes a function.
        self.let_statement("return ", body.clone());
        let body_lines = std::mem::replace(&mut self.lines, open.outer);
        self.indent = open.outer_indent;
        self.restore_pending(open.pending);
        let function = format!("body_{}", self.next_function());
        let own = format!("{parameter}: {}", variable_annotation(binding.class));
        let mut parameters = vec![Value::name(own, Class::Other)];
        let mut call = Value::name(function.clone(), Class::Other);
        if open.outlined && !open.captures.is_empty() {
            let mut arguments = Vec::new();
            if binding.used {
                arguments.push(parameter.clone());
            } else {
                parameters.clear();
            }
            for &captured in &open.captures {
                let position = state
                    .bindings
                    .iter()
                    .rposition(|binding| binding.variable == captured)
                    .expect("a captured variable is bound outside");
                let identifier = self.variable_names[captured.0].clone();
                let annotation = variable_annotation(state.bindings[position].class);
                parameters.push(Value::name(
                    format!("{identifier}: {annotation}"),
                    Class::Other,
                ));
                arguments.push(identifier);
            }
            let text = format!("lambda {parameter}: {function}({})", arguments.join(", "));
            call = Value {
                precedence: Precedence::Lambda,
                ..Value::atom(text, Class::Other)
            };
        }
        let signature = (function, parameters, body.annotation());
        if open.outlined {
            self.outlined_function(signature, body_lines);
        } else {
            self.local_function(signature, body_lines);
        }
        Value::call_of(name, &[open.domain, call], Class::Int)
    }

    /// Build a value from `operands`, moving the widest ones into temporaries
    /// while the result is wider than an expression's budget.
    pub(super) fn combine(
        &mut self,
        mut operands: Vec<Value>,
        build: impl Fn(&[Value]) -> Value,
    ) -> Value {
        let width = self.budget();
        loop {
            let value = build(&operands);
            if value.text.len() <= width {
                return value;
            }
            if !self.hoist_widest(&mut operands, |_| true) {
                return value;
            }
        }
    }

    /// Move the widest of `operands` that `eligible` accepts into a temporary,
    /// preferring anything over a plain name. Returns whether one moved.
    pub(super) fn hoist_widest(
        &mut self,
        operands: &mut [Value],
        eligible: impl Fn(&Value) -> bool,
    ) -> bool {
        let widest = operands
            .iter_mut()
            .filter(|operand| operand.text.len() >= SHORTEST_HOIST && eligible(operand))
            .max_by_key(|operand| (!matches!(operand.form, Form::Name), operand.text.len()));
        match widest {
            Some(widest) => {
                self.hoist(widest);
                true
            }
            None => false,
        }
    }

    /// Move `value` into a temporary and leave its name in its place.
    pub(super) fn hoist(&mut self, value: &mut Value) {
        let name = self.temporary();
        let hoisted = std::mem::replace(value, Value::name(name.clone(), value.class));
        value.literal = hoisted.literal;
        self.let_statement(&format!("{name} = "), hoisted);
    }
}
