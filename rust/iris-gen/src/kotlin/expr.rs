//! Kotlin expressions for postfix Iris expressions.
//!
//! Arithmetic and `!` are Kotlin's operators, which the builder's Java
//! methods and the Kotlin module's extensions provide; comparisons and the
//! logical operators are infix functions, and the rest are methods. The
//! interfaces of `dev.gecode.iris.build` decide what each operator takes, so
//! every value on the stack carries its [`Class`], and an operand that an
//! operator refuses goes through `.dynamic()`. A Kotlin literal has no
//! operators of the builder, so one on the left goes through `lit`. Every
//! value also carries its [`Level`], so an operand that binds less tightly
//! than its operator wants gets parentheses.
use super::{BODY_COLUMN, Line, Renderer, fits, quoted};
use crate::program::{
    BinaryOp, ComprehensionKind, Expr, ExprOp, Function, Host, NameId, UnaryOp, VariableId,
};

/// What the Kotlin type of a value allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Class {
    /// A `DynField`, which is a `DynExpr`.
    Handle,
    /// A `DynExpr`.
    Dyn,
    /// An `IntExpr`, or an integer literal.
    Int,
    /// A `FloatExpr`, or a float literal.
    Float,
    /// A `StringExpr`, or a string literal or constant.
    Str,
    /// A `RecordName`, which has no `.dynamic()`.
    Record,
    /// A `RecordExpr`, which is no domain.
    RecordItem,
    /// A `RecordList`, which has no `[]`.
    RecordList,
    /// Something only other builder calls take, such as a reading type.
    Other,
}

impl Class {
    /// The Kotlin type of a comprehension variable or a parameter of this
    /// class.
    pub(super) fn kotlin_type(self) -> &'static str {
        match self {
            Self::Handle => "DynField",
            Self::Int => "IntExpr",
            Self::Float => "FloatExpr",
            Self::Str => "StringExpr",
            Self::Record => "RecordName",
            Self::RecordItem => "RecordExpr",
            Self::RecordList => "RecordList",
            Self::Dyn | Self::Other => "DynExpr",
        }
    }
}

/// How tightly a value's text binds, from Kotlin's grammar: an operand that
/// binds less tightly than its operator wants gets parentheses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Level {
    /// An infix function call: `lt`, `and` and the others.
    Infix,
    /// `..`.
    Range,
    /// `+` and `-`.
    Additive,
    /// `*`, `/` and `%`.
    Multiplicative,
    /// Prefix `-` and `!`, and a negative literal.
    Prefix,
    /// A name, a literal, a call, a member access or a parenthesized value.
    Postfix,
}

/// How ktfmt may split a value that is too wide for its line.
#[derive(Debug, Clone)]
pub(super) enum Form {
    /// An identifier or a fixed piece of text, which no layout splits and no
    /// local value shortens.
    Name,
    /// A string literal, which a constant can shorten.
    Str,
    /// `callee(arguments)` with a plain function name, which splits between
    /// its arguments.
    Call {
        callee: String,
        arguments: Vec<Value>,
    },
    /// Anything else, which must fit its line.
    Other,
}

/// A rendered value.
#[derive(Debug, Clone)]
pub(super) struct Value {
    pub(super) text: String,
    pub(super) class: Class,
    pub(super) level: Level,
    /// A Kotlin literal, or a constant that holds one: a `Long`, `Double` or
    /// `String` argument as it is, and an expression through `lit`.
    pub(super) literal: bool,
    pub(super) form: Form,
}

impl Value {
    pub(super) fn new(text: String, class: Class) -> Self {
        Self {
            text,
            class,
            level: Level::Postfix,
            literal: false,
            form: Form::Other,
        }
    }

    pub(super) fn name(text: String, class: Class) -> Self {
        Self {
            form: Form::Name,
            ..Self::new(text, class)
        }
    }

    /// A string literal.
    pub(super) fn string(text: &str) -> Self {
        Self {
            literal: true,
            form: Form::Str,
            ..Self::new(quoted(text), Class::Str)
        }
    }

    fn literal(text: String, class: Class) -> Self {
        let level = if text.starts_with('-') {
            Level::Prefix
        } else {
            Level::Postfix
        };
        Self {
            literal: true,
            level,
            ..Self::new(text, class)
        }
    }

    /// `callee(arguments)`.
    pub(super) fn call(callee: &str, arguments: Vec<Value>, class: Class) -> Self {
        let texts: Vec<&str> = arguments.iter().map(|value| value.text.as_str()).collect();
        let text = format!("{callee}({})", texts.join(", "));
        Self {
            form: Form::Call {
                callee: callee.to_owned(),
                arguments,
            },
            ..Self::new(text, class)
        }
    }

    /// The text, in parentheses if it binds less tightly than `level`.
    fn operand(&self, level: Level) -> String {
        if self.level < level {
            format!("({})", self.text)
        } else {
            self.text.clone()
        }
    }

    /// `receiver.method(arguments)`, which no layout splits.
    pub(super) fn method(&self, method: &str, arguments: &[Value], class: Class) -> Self {
        let texts: Vec<&str> = arguments.iter().map(|value| value.text.as_str()).collect();
        Self::new(
            format!("{}.{method}({})", self.receiver(), texts.join(", ")),
            class,
        )
    }

    /// `receiver[field]`.
    fn member(&self, field: &Value) -> Self {
        Self::new(format!("{}[{}]", self.receiver(), field.text), Class::Dyn)
    }

    /// `left operator right`, whose operator binds as `level` and takes its
    /// left operand at that level too if `associative`.
    fn binary(
        left: &Value,
        operator: &str,
        right: &Value,
        level: Level,
        associative: bool,
    ) -> Self {
        let left_level = if associative {
            level
        } else {
            Self::tighter(level)
        };
        Self {
            level,
            ..Self::new(
                format!(
                    "{} {operator} {}",
                    left.operand(left_level),
                    right.operand(Self::tighter(level))
                ),
                Class::Int,
            )
        }
    }

    /// The level just above `level`.
    fn tighter(level: Level) -> Level {
        match level {
            Level::Infix => Level::Range,
            Level::Range => Level::Additive,
            Level::Additive => Level::Multiplicative,
            Level::Multiplicative => Level::Prefix,
            Level::Prefix | Level::Postfix => Level::Postfix,
        }
    }

    /// `operator operand`.
    fn prefix(operator: &str, operand: &Value, class: Class) -> Self {
        Self {
            level: Level::Prefix,
            ..Self::new(
                format!("{operator}{}", operand.operand(Level::Postfix)),
                class,
            )
        }
    }

    /// Whether no local value can make the value shorter.
    pub(super) fn is_fixed(&self) -> bool {
        matches!(self.form, Form::Name)
    }

    /// The text as the receiver of a method: a literal goes through `lit`.
    pub(super) fn receiver(&self) -> String {
        if self.literal {
            format!("lit({})", self.text)
        } else {
            self.operand(Level::Postfix)
        }
    }

    /// This value where an `Expr` is needed.
    pub(super) fn expr(&self) -> Self {
        if self.literal {
            Self::call("lit", vec![self.clone()], self.class)
        } else {
            self.clone()
        }
    }

    /// This value with no static type.
    pub(super) fn dynamic(&self) -> Self {
        Self::new(format!("{}.dynamic()", self.receiver()), Class::Dyn)
    }

    fn is_integral(&self) -> bool {
        matches!(self.class, Class::Handle | Class::Dyn | Class::Int)
    }

    /// This value where an `IntegralExpr` or a `Long` is needed.
    pub(super) fn integral(&self) -> Self {
        if self.is_integral() || !self.can_be_dynamic() {
            self.clone()
        } else {
            self.dynamic()
        }
    }

    /// This value as the left operand of an `IntegralExpr` operator.
    fn integral_receiver(&self) -> Self {
        self.integral().expr()
    }

    /// This value where a `NumericExpr`, a `Long` or a `Double` is needed.
    pub(super) fn numeric(&self) -> Self {
        if self.class == Class::Str {
            self.dynamic()
        } else {
            self.clone()
        }
    }

    /// This value where an `Expr` or a `Long` is needed.
    pub(super) fn long_or_expr(&self) -> Self {
        if self.literal && self.class != Class::Int {
            self.expr()
        } else {
            self.clone()
        }
    }

    /// This value as a range bound: a `Long`, or a `NumericExpr` that is no
    /// `FloatExpr`.
    pub(super) fn bound(&self) -> Self {
        match self.class {
            Class::Float | Class::Str => self.dynamic(),
            _ => self.clone(),
        }
    }

    /// `lower..upper`, with a `lit` on the left if both are literals, which
    /// Kotlin would make a range of its own, or `range(lower).below(upper)`.
    ///
    /// ktfmt 0.64 takes the import of `rangeUntil` for unused and removes
    /// it, so a program cannot spell an exclusive range `lower..<upper`.
    pub(super) fn range(lower: &Value, upper: &Value, exclusive: bool) -> Self {
        if exclusive {
            let range = Self::call("range", vec![lower.clone()], Class::Other);
            return range.method("below", std::slice::from_ref(upper), Class::Other);
        }
        let lower = if lower.literal && upper.literal {
            lower.expr()
        } else {
            lower.clone()
        };
        let tighter = Self::tighter(Level::Range);
        Self {
            level: Level::Range,
            ..Self::new(
                format!("{}..{}", lower.operand(tighter), upper.operand(tighter)),
                Class::Other,
            )
        }
    }

    /// This value as a domain.
    fn domain(&self) -> Self {
        self.expr()
    }

    /// Whether `.dynamic()` applies.
    fn can_be_dynamic(&self) -> bool {
        !matches!(
            self.class,
            Class::Record | Class::RecordItem | Class::RecordList | Class::Other
        )
    }
}

/// A local value that a function may need from the one around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Capture {
    pub(super) identifier: String,
    pub(super) kotlin_type: String,
}

/// An open comprehension: its domain is rendered, its body is being rendered.
struct OpenComprehension {
    domain: Value,
    /// The statements before the comprehension, set aside for the body's own.
    outer: Vec<Line>,
    outer_column: usize,
    /// The variables and locals from outside that the body uses, which a
    /// function for the body takes.
    captures: Vec<Capture>,
}

/// A comprehension variable in scope.
struct Binding {
    variable: VariableId,
    identifier: String,
    class: Class,
    used: bool,
}

/// The state of one expression's rendering.
#[derive(Default)]
pub(super) struct Evaluation {
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

    /// Note that every open comprehension from `from` on uses `capture`.
    fn capture(&mut self, from: usize, capture: &Capture) {
        for open in &mut self.comprehensions[from..] {
            if !open.captures.contains(capture) {
                open.captures.push(capture.clone());
            }
        }
    }
}

/// The Kotlin operator of `op`, how tightly it binds, and whether its left
/// operand may be one of the same level without parentheses.
fn operator(op: BinaryOp) -> (&'static str, Level, bool) {
    match op {
        BinaryOp::Or => ("or", Level::Infix, false),
        BinaryOp::And => ("and", Level::Infix, false),
        BinaryOp::Equal => ("eq", Level::Infix, false),
        BinaryOp::NotEqual => ("ne", Level::Infix, false),
        BinaryOp::Less => ("lt", Level::Infix, false),
        BinaryOp::LessEqual => ("le", Level::Infix, false),
        BinaryOp::Greater => ("gt", Level::Infix, false),
        BinaryOp::GreaterEqual => ("ge", Level::Infix, false),
        BinaryOp::Add => ("+", Level::Additive, true),
        BinaryOp::Subtract => ("-", Level::Additive, true),
        BinaryOp::Multiply => ("*", Level::Multiplicative, true),
        BinaryOp::Divide => ("/", Level::Multiplicative, true),
        BinaryOp::Remainder => ("%", Level::Multiplicative, true),
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
        ComprehensionKind::Sum => "sum",
        ComprehensionKind::Count => "count",
    }
}

/// An integer as a Kotlin literal. Kotlin types one too large for `Int` as a
/// `Long`, but it has no literal of `Long.MIN_VALUE`.
pub(super) fn int_literal(value: i64) -> String {
    if value == i64::MIN {
        "Long.MIN_VALUE".to_owned()
    } else {
        value.to_string()
    }
}

/// A float as a Kotlin literal that reads back as the same value.
fn float_literal(value: f64) -> String {
    format!("{value:?}")
}

/// What a textual or numeric operand of `eq` and `ne` is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Open,
    Numeric,
    Textual,
}

fn side(value: &Value) -> Side {
    match value.class {
        Class::Str => Side::Textual,
        Class::Int | Class::Float => Side::Numeric,
        _ => Side::Open,
    }
}

/// Operands shorter than this gain nothing from a local value of their own.
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
            ExprOp::Int(value) => Value::literal(int_literal(*value), Class::Int),
            ExprOp::Float(value) => Value::literal(float_literal(*value), Class::Float),
            ExprOp::Str(text) => Value::string(text),
            ExprOp::Host(host) => {
                self.imports.insert("Gecode".to_owned());
                let text = match host {
                    Host::IntMax => "Gecode.intMax",
                    Host::IntMin => "Gecode.intMin",
                };
                Value::new(text.to_owned(), Class::Int)
            }
            ExprOp::Name(name) => self.expression_handle(*name, state),
            ExprOp::Var(variable) => self.variable(*variable, state),
            ExprOp::Each(record) => Value::name(self.record_names[record.0].clone(), Class::Record),
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
                    let right = values[1].integral();
                    values[0]
                        .integral_receiver()
                        .method("pow", &[right], Class::Int)
                })
            }
            ExprOp::Index => {
                let index = state.pop();
                let base = state.pop();
                self.combine(vec![base, index], |values| {
                    let (base, index) = (values[0].expr(), values[1].integral());
                    let class = match base.class {
                        Class::Record | Class::RecordItem | Class::RecordList => Class::RecordItem,
                        Class::Handle | Class::Other => Class::Dyn,
                        class => class,
                    };
                    base.method("at", &[index], class)
                })
            }
            ExprOp::Member(field) => {
                let base = state.pop();
                let field = self.expression_handle(*field, state);
                self.combine(vec![base, field], |values| {
                    values[0].expr().member(&values[1])
                })
            }
            ExprOp::ListMember(record) => {
                let base = state.pop();
                let inner = Value::name(self.record_names[record.0].clone(), Class::Record);
                self.combine(vec![base, inner], |values| {
                    values[0].method("records", &[values[1].clone()], Class::RecordList)
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

    /// The handle of `name` inside an expression, which a function for a
    /// comprehension's body takes if it is a local value.
    fn expression_handle(&mut self, name: NameId, state: &mut Evaluation) -> Value {
        if let Some(capture) = self.local_capture(name) {
            state.capture(0, &capture);
        }
        self.handle(name)
    }

    fn variable(&mut self, variable: VariableId, state: &mut Evaluation) -> Value {
        let position = state
            .bindings
            .iter()
            .rposition(|binding| binding.variable == variable)
            .expect("a checked variable is bound by a comprehension");
        let binding = &mut state.bindings[position];
        binding.used = true;
        let (identifier, class) = (binding.identifier.clone(), binding.class);
        // Each comprehension inside the binding one needs the variable from
        // outside if its body becomes a function.
        let capture = Capture {
            identifier: identifier.clone(),
            kotlin_type: class.kotlin_type().to_owned(),
        };
        state.capture(position + 1, &capture);
        Value::name(identifier, class)
    }

    fn unary(&mut self, op: UnaryOp, operand: Value) -> Value {
        self.combine(vec![operand], |values| match op {
            UnaryOp::Minus => {
                let operand = values[0].numeric().expr();
                let class = match operand.class {
                    Class::Handle | Class::Other => Class::Dyn,
                    class => class,
                };
                Value::prefix("-", &operand, class)
            }
            UnaryOp::Not => Value::prefix("!", &values[0].integral_receiver(), Class::Int),
        })
    }

    fn binary(&mut self, op: BinaryOp, left: Value, right: Value) -> Value {
        let (operator, level, associative) = operator(op);
        self.combine(vec![left, right], |values| {
            let (left, right) = (values[0].clone(), values[1].clone());
            let (left, right) = match op {
                BinaryOp::Equal | BinaryOp::NotEqual => {
                    let left = left.expr();
                    match (side(&left), side(&right)) {
                        (Side::Numeric, Side::Textual) | (Side::Textual, Side::Numeric) => {
                            (left.dynamic(), right)
                        }
                        _ => (left, right),
                    }
                }
                BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual => (left.numeric().expr(), right.numeric()),
                _ => (left.integral_receiver(), right.integral()),
            };
            Value::binary(&left, operator, &right, level, associative)
        })
    }

    fn function_call(&mut self, function: Function, arguments: Vec<Value>) -> Value {
        let name = function_name(function);
        self.combine(arguments, |values| {
            let arguments: Vec<Value> = match function {
                Function::Size | Function::Count => vec![values[0].domain()],
                _ => values.iter().map(Value::expr).collect(),
            };
            Value::call(name, arguments, Class::Int)
        })
    }

    fn open_comprehension(&mut self, variable: VariableId, domain: Value, state: &mut Evaluation) {
        let spelling = self.program.variables[variable.0].clone();
        let class = match domain.class {
            Class::Record | Class::RecordList => Class::RecordItem,
            Class::Int | Class::Float | Class::Str | Class::RecordItem => domain.class,
            Class::Handle | Class::Dyn | Class::Other => Class::Dyn,
        };
        let name = Value::string(&spelling);
        let domain = self.combine(vec![domain, name], |values| {
            let over = Value::call("over", vec![values[0].domain()], Class::Other);
            over.method("named", &[values[1].clone()], Class::Other)
        });
        // `iris check` refuses a variable that hides one around it, so no
        // lambda's parameter hides another.
        let identifier = self.variable_names[variable.0].clone();
        state.comprehensions.push(OpenComprehension {
            domain,
            outer: std::mem::take(&mut self.lines),
            outer_column: self.column,
            captures: Vec::new(),
        });
        state.bindings.push(Binding {
            variable,
            identifier,
            class,
            used: false,
        });
        self.column = BODY_COLUMN;
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
        // A variable named `it` is the lambda's implicit parameter, which
        // detekt wants unnamed.
        let parameter = match binding.identifier.as_str() {
            "it" if binding.used => String::new(),
            identifier if binding.used => format!("{identifier} -> "),
            _ => "_ -> ".to_owned(),
        };
        let name = comprehension_name(kind);
        let mut body = body.expr();
        let call = |domain: &Value, lambda: &str| {
            Value::new(
                format!("{name}({}) {{ {parameter}{lambda} }}", domain.text),
                Class::Int,
            )
        };
        let budget = Self::budget_at(open.outer_column);
        let mut domain = open.domain;
        if self.lines.is_empty() {
            let inline = call(&domain, &body.text);
            // With the domain in a local value, which only a name can replace.
            let shorter = inline.text.len() - domain.text.len() + "e0000".len();
            let hoistable = !domain.is_fixed() && domain.text.len() >= SHORTEST_HOIST;
            if inline.text.len() <= budget || (hoistable && shorter <= budget) {
                self.lines = open.outer;
                self.column = open.outer_column;
                if inline.text.len() <= budget {
                    return inline;
                }
                self.hoist(&mut domain);
                return call(&domain, &body.text);
            }
        }
        // The body goes to a function of its own, which sees only what it is
        // given.
        if !fits(self.column, &format!("return {}", body.text)) {
            self.hoist(&mut body);
        }
        let mut lines = std::mem::replace(&mut self.lines, open.outer);
        lines.push(Line {
            column: self.column,
            text: format!("return {}", body.text),
        });
        self.column = open.outer_column;
        let function = self.next_body();
        let mut parameters = Vec::new();
        if binding.used {
            parameters.push(Capture {
                identifier: binding.identifier.clone(),
                kotlin_type: binding.class.kotlin_type().to_owned(),
            });
        }
        parameters.extend(open.captures.iter().cloned());
        let arguments: Vec<&str> = parameters
            .iter()
            .map(|parameter| parameter.identifier.as_str())
            .collect();
        let lambda = format!("{function}({})", arguments.join(", "));
        self.imports.insert("Expr".to_owned());
        let head = format!("private fun {function}");
        self.function_declaration(&head, &parameters, ": Expr {", lines);
        let value = call(&domain, &lambda);
        if value.text.len() > budget && !domain.is_fixed() && domain.text.len() >= SHORTEST_HOIST {
            self.hoist(&mut domain);
            return call(&domain, &lambda);
        }
        value
    }

    /// Build a value from `operands`, moving the widest ones into local
    /// values while the result is wider than an expression's budget.
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
            if !self.hoist_widest(&mut operands) {
                return value;
            }
        }
    }

    /// Move the widest of `operands` into a local value, or a string into a
    /// constant. Returns whether one moved.
    fn hoist_widest(&mut self, operands: &mut [Value]) -> bool {
        let widest = operands
            .iter_mut()
            .filter(|operand| operand.text.len() >= SHORTEST_HOIST && !operand.is_fixed())
            .max_by_key(|operand| operand.text.len());
        match widest {
            Some(widest) => {
                self.hoist(widest);
                true
            }
            None => false,
        }
    }
}
