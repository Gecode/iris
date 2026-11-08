//! TypeScript expressions for postfix Iris expressions.
//!
//! Every operator is a method of the left operand or a function, and the
//! classes of `gecode-iris/build` decide which methods a value has and what
//! each one takes, so every value on the stack carries its [`Class`], and an
//! operand that a method refuses goes through `.dynamic()`. A number or a
//! string has no methods, so one on the left goes through `lit`; elsewhere a
//! literal is an operand as it is.
//!
//! oxfmt breaks a member chain with more than two calls into one call per
//! line, even when it fits, if an argument of one of its calls is not simple
//! in prettier's sense, so every value also carries what that rule asks of
//! it, and [`Renderer::combine`] moves an operand into a temporary where the
//! rule would break a chain.
use super::layout::call_lines;
use super::{Line, Renderer, TAB, fits, quoted};
use crate::program::{
    BinaryOp, ComprehensionKind, Expr, ExprOp, Function, Host, RecordId, UnaryOp, VariableId,
};

/// What the TypeScript type of a value allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Class {
    /// A `DynField` from `forward`, which is a `Lookup<"dyn">`.
    Handle,
    /// An `Expr<"dyn">`.
    Dyn,
    /// An `Expr<"int">`, or a number or bigint.
    Int,
    /// An `Expr<"float">`.
    Float,
    /// An `Expr<"string">`, or a string.
    Str,
    /// A `Record`, which has no `.at` and no `.dynamic()`.
    Record,
    /// A `RecordExpr`, which is no domain.
    RecordItem,
    /// A `RecordList`, which has no `.get`.
    RecordList,
    /// Something only other builder calls take, such as a reading type.
    Other,
}

impl Class {
    /// The TypeScript type of a comprehension variable over a domain whose
    /// items are of this class.
    pub(super) fn variable_type(self) -> &'static str {
        match self {
            Self::RecordItem | Self::Record | Self::RecordList => "RecordExpr",
            Self::Int => "Lookup<\"int\">",
            Self::Float => "Lookup<\"float\">",
            Self::Str => "Lookup<\"string\">",
            Self::Handle | Self::Dyn | Self::Other => "Lookup<\"dyn\">",
        }
    }
}

/// How a value may change when it is too wide for its line.
#[derive(Debug, Clone)]
pub(super) enum Form {
    /// An identifier, a member such as `gecode.intMax`, or an arrow
    /// function, which no layout splits and no temporary shortens.
    Name,
    /// A string literal, which a string constant can shorten.
    Str,
    /// Anything else, which a temporary can replace.
    Other,
}

/// The parts of a call of a function, an array or an object.
#[derive(Debug, Clone)]
pub(super) enum Parts {
    /// The function and the text of each argument.
    Call(String, Vec<String>),
    /// The text of each element.
    Array(Vec<String>),
    /// The text of each property.
    Object(Vec<String>),
}

/// The deepest prettier's `isSimpleCallArgument` looks: an identifier or a
/// literal is simple down to this depth of call arguments.
const SIMPLE_DEPTH: i32 = 1;
/// The depth of a value that is never simple.
const NEVER_SIMPLE: i32 = -1000;
/// The most arguments that a simple call may have.
const SIMPLE_ARGUMENTS: usize = 2;
/// The most calls that a member chain keeps on one line whatever their
/// arguments.
const FLAT_CALLS: usize = 2;

/// A rendered value.
#[derive(Debug, Clone)]
pub(super) struct Value {
    pub(super) text: String,
    pub(super) class: Class,
    /// Whether the value is a `Lookup`, which has `.at` and `.get`.
    pub(super) lookup: bool,
    /// A number, bigint or string literal, or a temporary or constant that
    /// holds one, which needs `lit` to be an `Expr`.
    pub(super) literal: bool,
    pub(super) form: Form,
    /// Whether the value is an object or an array, which oxfmt keeps on the
    /// line of a call that it is the last argument of.
    pub(super) hugs: bool,
    /// What oxfmt puts on lines of their own when the value is too wide.
    pub(super) parts: Option<Parts>,
    /// The calls of the member chain that the value ends, counting a call at
    /// its head.
    calls: usize,
    /// Whether every argument of those calls is simple.
    simple_chain: bool,
    /// The deepest call argument at which the value is still simple, or
    /// below zero if it is never simple.
    simple: i32,
}

impl Value {
    fn with(text: String, class: Class, form: Form, simple: i32) -> Self {
        Self {
            text,
            class,
            lookup: false,
            literal: false,
            form,
            hugs: false,
            parts: None,
            calls: 0,
            simple_chain: true,
            simple,
        }
    }

    /// An identifier or a member of one.
    pub(super) fn name(text: String, class: Class) -> Self {
        Self::with(text, class, Form::Name, SIMPLE_DEPTH)
    }

    /// The handle `text`, which is a lookup.
    pub(super) fn handle(text: String) -> Self {
        Self {
            lookup: true,
            ..Self::name(text, Class::Handle)
        }
    }

    /// A string literal.
    pub(super) fn string(text: &str) -> Self {
        Self {
            literal: true,
            ..Self::with(quoted(text), Class::Str, Form::Str, SIMPLE_DEPTH)
        }
    }

    /// A number or bigint literal.
    pub(super) fn number(text: String) -> Self {
        Self {
            literal: true,
            ..Self::with(text, Class::Int, Form::Other, SIMPLE_DEPTH)
        }
    }

    /// Text that prettier never takes for a simple argument, such as an
    /// object, a tagged template or an arrow function.
    pub(super) fn opaque(text: String, class: Class, form: Form) -> Self {
        Self::with(text, class, form, NEVER_SIMPLE)
    }

    /// An array literal of `elements`.
    pub(super) fn array(elements: &[Value]) -> Self {
        let texts: Vec<&str> = elements.iter().map(|value| value.text.as_str()).collect();
        let simple = elements
            .iter()
            .map(|element| element.simple - 1)
            .fold(SIMPLE_DEPTH, i32::min);
        Self {
            hugs: true,
            parts: Some(Parts::Array(
                texts.iter().map(|text| (*text).to_owned()).collect(),
            )),
            ..Self::with(
                format!("[{}]", texts.join(", ")),
                Class::Other,
                Form::Other,
                simple,
            )
        }
    }

    /// The object literal with `properties`.
    pub(super) fn object(properties: Vec<String>) -> Self {
        let text = format!("{{ {} }}", properties.join(", "));
        Self {
            hugs: true,
            parts: Some(Parts::Object(properties)),
            ..Self::opaque(text, Class::Other, Form::Other)
        }
    }

    /// The constant `name` that holds this value.
    pub(super) fn renamed(&self, name: String) -> Self {
        Self {
            lookup: self.lookup,
            literal: self.literal,
            ..Self::name(name, self.class)
        }
    }

    /// How simple a call with `arguments` is, given its callee's.
    fn call_simplicity(callee: i32, arguments: &[Value]) -> i32 {
        if arguments.len() > SIMPLE_ARGUMENTS {
            return NEVER_SIMPLE;
        }
        arguments
            .iter()
            .map(|argument| argument.simple - 1)
            .fold(callee, i32::min)
    }

    fn arguments_are_simple(arguments: &[Value]) -> bool {
        arguments.iter().all(|argument| argument.simple >= 0)
    }

    /// `callee(arguments)`.
    pub(super) fn call(callee: &str, arguments: Vec<Value>, class: Class) -> Self {
        let texts: Vec<&str> = arguments.iter().map(|value| value.text.as_str()).collect();
        let parts = texts.iter().map(|text| (*text).to_owned()).collect();
        Self {
            parts: Some(Parts::Call(callee.to_owned(), parts)),
            calls: 1,
            simple_chain: Self::arguments_are_simple(&arguments),
            ..Self::with(
                format!("{callee}({})", texts.join(", ")),
                class,
                Form::Other,
                Self::call_simplicity(SIMPLE_DEPTH, &arguments),
            )
        }
    }

    /// `receiver.method(arguments)`.
    pub(super) fn method(&self, method: &str, arguments: &[Value], class: Class) -> Self {
        let receiver = self.receiver();
        let texts: Vec<&str> = arguments.iter().map(|value| value.text.as_str()).collect();
        Self {
            calls: receiver.calls + 1,
            simple_chain: receiver.simple_chain && Self::arguments_are_simple(arguments),
            ..Self::with(
                format!("{}.{method}({})", receiver.text, texts.join(", ")),
                class,
                Form::Other,
                Self::call_simplicity(receiver.simple, arguments),
            )
        }
    }

    /// `receiver.method(arguments)`, which is a lookup.
    fn lookup_method(&self, method: &str, arguments: &[Value], class: Class) -> Self {
        Self {
            lookup: true,
            ..self.method(method, arguments, class)
        }
    }

    /// Whether oxfmt would break the member chain that the value ends.
    pub(super) fn breaks_chain(&self) -> bool {
        self.calls > FLAT_CALLS && !self.simple_chain
    }

    /// Whether no temporary can make the value shorter.
    pub(super) fn is_fixed(&self) -> bool {
        matches!(self.form, Form::Name)
    }

    /// Whether the value is no simple call argument.
    fn is_complex(&self) -> bool {
        self.simple < 0 || (self.calls > 0 && !self.simple_chain)
    }

    /// The value as the receiver of a method: a literal goes through `lit`.
    fn receiver(&self) -> Self {
        if self.literal {
            Self::call("lit", vec![self.clone()], self.class)
        } else {
            self.clone()
        }
    }

    /// This value where an `Expr` is needed.
    pub(super) fn expr(&self) -> Self {
        self.receiver()
    }

    /// This value with no static type, which is a lookup.
    pub(super) fn dynamic(&self) -> Self {
        self.lookup_method("dynamic", &[], Class::Dyn)
    }

    fn is_integral(&self) -> bool {
        matches!(self.class, Class::Handle | Class::Dyn | Class::Int)
    }

    /// This value where an `IntOperand` is needed.
    pub(super) fn integral(&self) -> Self {
        if self.is_integral() || !self.can_be_dynamic() {
            self.clone()
        } else {
            self.dynamic()
        }
    }

    /// This value as the receiver of a method on `Expr<Integral>`.
    fn integral_receiver(&self) -> Self {
        self.integral().expr()
    }

    /// This value where a `NumericOperand` is needed.
    fn numeric(&self) -> Self {
        if self.class == Class::Str {
            self.dynamic()
        } else {
            self.clone()
        }
    }

    /// This value as a range bound: a number, a bigint, or an `Expr` that is
    /// neither a float nor a string.
    pub(super) fn bound(&self) -> Self {
        match self.class {
            Class::Float | Class::Str => self.dynamic(),
            _ => self.clone(),
        }
    }

    /// This value as a domain or as the argument of `defined`.
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

    /// Whether `.at` and `.get` apply as they are.
    fn has_members(&self) -> bool {
        self.lookup || matches!(self.class, Class::Record | Class::RecordItem)
    }

    /// This value where `.at` and `.get` are needed.
    fn with_members(&self) -> Self {
        if self.has_members() || !self.can_be_dynamic() {
            self.clone()
        } else {
            self.dynamic()
        }
    }
}

/// A constant or a variable that a function of the module takes from the
/// code that calls it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Capture {
    pub(super) identifier: String,
    pub(super) ts_type: String,
}

impl Capture {
    pub(super) fn new(identifier: &str, ts_type: &str) -> Self {
        Self {
            identifier: identifier.to_owned(),
            ts_type: ts_type.to_owned(),
        }
    }
}

/// An open comprehension: its domain is rendered, its body is being rendered.
struct OpenComprehension {
    domain: Value,
    /// The statements before the comprehension, set aside for the body's own.
    outer: Vec<Line>,
    outer_column: usize,
    /// Whether the body's statements go to a function of their own.
    outlined: bool,
    /// The variables, handles and records from outside that the body uses,
    /// which a function for the body takes.
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

fn method_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Or => "or",
        BinaryOp::And => "and",
        BinaryOp::Equal => "eq",
        BinaryOp::NotEqual => "ne",
        BinaryOp::Less => "lt",
        BinaryOp::LessEqual => "le",
        BinaryOp::Greater => "gt",
        BinaryOp::GreaterEqual => "ge",
        BinaryOp::Add => "plus",
        BinaryOp::Subtract => "minus",
        BinaryOp::Multiply => "times",
        // Functions, below.
        BinaryOp::Divide => "div",
        BinaryOp::Remainder => "rem",
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

/// The largest integer that a `number` holds exactly.
const SAFE_INTEGER: i64 = (1 << 53) - 1;

/// An integer as a TypeScript literal: a `number` if it is safe, which `lit`
/// requires, and a `bigint` otherwise.
pub(super) fn int_literal(value: i64) -> String {
    if (-SAFE_INTEGER..=SAFE_INTEGER).contains(&value) {
        value.to_string()
    } else {
        format!("{value}n")
    }
}

/// A float as a TypeScript number literal that reads back as the same value,
/// in the spelling that oxfmt keeps.
fn float_literal(value: f64) -> String {
    format!("{value:?}")
}

/// Whether oxlint's `oxc/approx-constant` takes the literal of `value` for
/// one of the `Math` constants. The rule reads a literal as Rust prints its
/// value, without a sign, and refuses one of five characters or more that is
/// the start of a constant or the constant rounded to as many decimals; see
/// <https://github.com/oxc-project/oxc/blob/main/crates/oxc_linter/src/rules/oxc/approx_constant.rs>.
fn approximates_math_constant(value: f64) -> bool {
    use std::f64::consts as c;
    const SHORTEST: usize = 5;
    const MATH_CONSTANTS: [f64; 8] = [
        c::E,
        c::LN_10,
        c::LN_2,
        c::LOG2_E,
        c::LOG10_E,
        c::PI,
        c::FRAC_1_SQRT_2,
        c::SQRT_2,
    ];
    let text = value.abs().to_string();
    text.len() >= SHORTEST
        && MATH_CONSTANTS.iter().any(|constant| {
            constant.to_string().starts_with(&text)
                || format!("{constant:.*}", text.len() - 2) == text
        })
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
            ExprOp::Int(value) => Value::number(int_literal(*value)),
            ExprOp::Float(value) => {
                self.approx_constant |= approximates_math_constant(*value);
                let literal = Value::name(float_literal(*value), Class::Float);
                Value::call("floatLit", vec![literal], Class::Float)
            }
            ExprOp::Str(text) => Value::string(text),
            ExprOp::Host(host) => {
                let text = match host {
                    Host::IntMax => "gecode.intMax",
                    Host::IntMin => "gecode.intMin",
                };
                Value::name(text.to_owned(), Class::Int)
            }
            ExprOp::Name(name) => {
                let handle = self.handle(*name);
                state.capture(0, &Capture::new(&handle.text, "DynField"));
                handle
            }
            ExprOp::Var(variable) => self.variable(*variable, state),
            ExprOp::Each(record) => self.captured_record(*record, state),
            ExprOp::List(record) => {
                let record = self.captured_record(*record, state);
                Value::name(format!("{}.list", record.text), Class::RecordList)
            }
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
                    let arguments = vec![values[0].integral(), values[1].integral()];
                    Value::call("pow", arguments, Class::Int)
                })
            }
            ExprOp::Index => {
                let index = state.pop();
                let base = state.pop();
                self.combine(vec![base, index], |values| {
                    let index = values[1].integral();
                    let base = values[0].with_members();
                    match base.class {
                        Class::Record | Class::RecordItem | Class::RecordList => {
                            base.method("at", &[index], Class::RecordItem)
                        }
                        Class::Handle | Class::Other => {
                            base.lookup_method("at", &[index], Class::Dyn)
                        }
                        class => base.lookup_method("at", &[index], class),
                    }
                })
            }
            ExprOp::Member(field) => {
                let base = state.pop();
                let field = self.handle(*field);
                state.capture(0, &Capture::new(&field.text, "DynField"));
                self.combine(vec![base, field], |values| {
                    let base = values[0].with_members();
                    base.lookup_method("get", &[values[1].clone()], Class::Dyn)
                })
            }
            ExprOp::ListMember(record) => {
                let base = state.pop();
                let inner = self.captured_record(*record, state);
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

    /// The record of `record`, which every open comprehension takes.
    fn captured_record(&mut self, record: RecordId, state: &mut Evaluation) -> Value {
        let value = self.record(record);
        state.capture(0, &Capture::new(&value.text, "Record"));
        value
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
        state.capture(
            position + 1,
            &Capture::new(&identifier, class.variable_type()),
        );
        let value = Value::name(identifier, class);
        Value {
            lookup: class != Class::RecordItem,
            ..value
        }
    }

    fn unary(&mut self, op: UnaryOp, operand: Value) -> Value {
        self.combine(vec![operand], |values| match op {
            UnaryOp::Minus => {
                let operand = values[0].numeric().expr();
                let class = match operand.class {
                    Class::Handle | Class::Other => Class::Dyn,
                    class => class,
                };
                operand.method("neg", &[], class)
            }
            UnaryOp::Not => values[0].integral_receiver().method("not", &[], Class::Int),
        })
    }

    fn binary(&mut self, op: BinaryOp, left: Value, right: Value) -> Value {
        let method = method_name(op);
        self.combine(vec![left, right], |values| {
            let (left, right) = (values[0].clone(), values[1].clone());
            match op {
                BinaryOp::Divide | BinaryOp::Remainder => {
                    let arguments = vec![left.integral(), right.integral()];
                    return Value::call(method, arguments, Class::Int);
                }
                _ => {}
            }
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
            left.method(method, &[right], Class::Int)
        })
    }

    fn function_call(&mut self, function: Function, arguments: Vec<Value>) -> Value {
        let name = function_name(function);
        self.combine(arguments, |values| {
            let arguments: Vec<Value> = match function {
                Function::Size | Function::Count | Function::Defined => {
                    vec![values[0].domain()]
                }
                _ => values.to_vec(),
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
            Value::call(
                "over",
                vec![values[0].domain(), values[1].clone()],
                Class::Other,
            )
        });
        // `iris check` refuses a variable that hides one around it, so no
        // arrow function's parameter hides another.
        let identifier = self.variable_names[variable.0].clone();
        let outer_column = self.column;
        let outlined = outer_column + TAB > super::DEEPEST_BODY;
        state.comprehensions.push(OpenComprehension {
            domain,
            outer: std::mem::take(&mut self.lines),
            outer_column,
            outlined,
            captures: Vec::new(),
        });
        state.bindings.push(Binding {
            variable,
            identifier,
            class,
            used: false,
        });
        self.column = if outlined {
            super::BODY_COLUMN
        } else {
            outer_column + TAB
        };
    }

    /// Close a comprehension, which always goes into a temporary of its own:
    /// oxfmt breaks every argument of a call that has a call with an arrow
    /// function among its arguments.
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
        let parameter = if binding.used {
            format!("({})", binding.identifier)
        } else {
            "()".to_owned()
        };
        let name = comprehension_name(kind);
        let body_lines = std::mem::replace(&mut self.lines, open.outer);
        let body_column = self.column;
        self.column = open.outer_column;
        let mut domain = open.domain;
        let temporary = self.temporary();
        let head = format!("const {temporary} = ");
        let statement =
            |domain: &Value, arrow: &str| format!("{head}{name}({}, {arrow});", domain.text);
        if body_lines.is_empty() {
            let concise = format!("{parameter} => {}", body.text);
            if fits(self.column, &statement(&domain, &concise)) {
                self.line(statement(&domain, &concise));
                return Value::name(temporary, Class::Int);
            }
        }
        if open.outlined {
            // The body goes to a function of its own, which sees only what
            // it is given.
            let function = self.next_body();
            let mut parameters = Vec::new();
            if binding.used {
                parameters.push(Capture {
                    identifier: binding.identifier.clone(),
                    ts_type: binding.class.variable_type().to_owned(),
                });
            }
            parameters.extend(open.captures.iter().cloned());
            let mut lines = body_lines;
            let mut body = body.expr();
            let saved = std::mem::replace(&mut self.column, body_column);
            if !fits(body_column, &format!("return {};", body.text)) {
                let outer = std::mem::replace(&mut self.lines, lines);
                self.hoist(&mut body);
                lines = std::mem::replace(&mut self.lines, outer);
            }
            self.column = saved;
            lines.push(Line::new(body_column, format!("return {};", body.text)));
            self.type_imports.insert("Expr");
            self.function_declaration(&function, &parameters, ": Expr", lines);
            let arguments: Vec<String> = parameters
                .iter()
                .map(|parameter| parameter.identifier.clone())
                .collect();
            let arrow = format!("{parameter} => {function}({})", arguments.join(", "));
            if !fits(self.column, &statement(&domain, &arrow)) && !domain.is_fixed() {
                self.hoist(&mut domain);
            }
            if fits(self.column, &statement(&domain, &arrow)) {
                self.line(statement(&domain, &arrow));
                return Value::name(temporary, Class::Int);
            }
            // Too many variables for one line: the arrow function returns
            // the call, with its arguments on lines of their own.
            self.line(format!("{head}{name}({}, {parameter} => {{", domain.text));
            let column = self.column + TAB;
            let call =
                call_lines("return ", &function, &arguments, ";", column).unwrap_or_else(|| {
                    let text = format!("return {function}({});", arguments.join(", "));
                    vec![Line::new(column, text)]
                });
            self.lines.extend(call);
            self.line("});".to_owned());
            return Value::name(temporary, Class::Int);
        }
        // A body with statements, or too wide for a concise arrow function.
        let mut lines = body_lines;
        let mut body = body;
        if !fits(body_column, &format!("return {};", body.text)) && !body.is_fixed() {
            let outer = std::mem::replace(&mut self.lines, lines);
            self.column = body_column;
            self.hoist(&mut body);
            self.column = open.outer_column;
            lines = std::mem::replace(&mut self.lines, outer);
        }
        lines.push(Line::new(body_column, format!("return {};", body.text)));
        let opener = |domain: &Value| format!("{head}{name}({}, {parameter} => {{", domain.text);
        if !fits(self.column, &opener(&domain)) && !domain.is_fixed() {
            self.hoist(&mut domain);
        }
        self.line(opener(&domain));
        self.lines.extend(lines);
        self.line("});".to_owned());
        Value::name(temporary, Class::Int)
    }

    /// Build a value from `operands`, moving operands into temporaries while
    /// the result is wider than an expression's budget or oxfmt would break
    /// its member chain.
    pub(super) fn combine(
        &mut self,
        mut operands: Vec<Value>,
        build: impl Fn(&[Value]) -> Value,
    ) -> Value {
        let width = self.budget();
        loop {
            let value = build(&operands);
            if value.breaks_chain() && self.hoist_complex(&mut operands) {
                continue;
            }
            if value.text.len() <= width || !self.hoist_widest(&mut operands) {
                return value;
            }
        }
    }

    /// Move an operand that keeps a member chain from one line into a
    /// temporary: one that is no simple argument, or else the widest one
    /// that is not a name. Returns whether one moved.
    fn hoist_complex(&mut self, operands: &mut [Value]) -> bool {
        let complex = operands.iter().position(Value::is_complex).or_else(|| {
            (0..operands.len())
                .filter(|&index| !operands[index].is_fixed())
                .max_by_key(|&index| operands[index].text.len())
        });
        match complex {
            Some(index) => {
                // A literal goes in as an `Expr`, so that its temporary
                // needs no `lit` of its own.
                let mut value = operands[index].expr();
                self.hoist(&mut value);
                operands[index] = value;
                true
            }
            None => false,
        }
    }

    /// Move the widest of `operands` into a temporary, or a string into a
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
