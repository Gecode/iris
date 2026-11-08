//! C++ expressions for postfix Iris expressions.
//!
//! The concepts of `iris/build.hpp` decide what each operand may be, so every
//! value on the stack carries its [`Class`], and an operand that an operator
//! refuses goes through `.dynamic()`. An operator needs an operand from the
//! builder on one side, so a C++ literal on the left goes through `lit`.
use super::{Arg, HELPER_BODY_INDENT, Line, MAX_INDENT, Renderer, TAB, fits, quoted};
use crate::program::{
    BinaryOp, ComprehensionKind, Expr, ExprOp, Function, Host, UnaryOp, VariableId,
};

/// What the C++ type of a value allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Class {
    /// A `DynField`, which has `[]` and `.dynamic()` but no `.get`.
    Handle,
    /// An `Expr<Dynamic, _>`.
    Dyn,
    /// An `Expr<std::int64_t>` or an integer literal.
    Int,
    /// An `Expr<double>` or a float literal.
    Float,
    /// An `Expr<std::string>` or a string literal.
    Str,
    /// A host constant, whose `.dynamic()` is an expression.
    Host,
    /// A `Record`, which has no `.dynamic()`.
    Record,
    /// A `RecordExpr`.
    RecordItem,
    /// A `RecordList`.
    RecordList,
    /// Something only other builder calls take, such as a reading type.
    Other,
}

/// C++'s binding strengths, weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Precedence {
    Or,
    And,
    Equality,
    Relational,
    Sum,
    Product,
    Unary,
    Postfix,
}

/// The brackets of a call or a braced list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Bracket {
    Paren,
    Brace,
}

impl Bracket {
    pub(super) fn open(self) -> char {
        match self {
            Self::Paren => '(',
            Self::Brace => '{',
        }
    }

    pub(super) fn close(self) -> char {
        match self {
            Self::Paren => ')',
            Self::Brace => '}',
        }
    }
}

/// How clang-format may split a value that is too wide for its line.
#[derive(Debug, Clone)]
pub(super) enum Form {
    /// A literal or member access, which no layout splits.
    Atom,
    /// A plain identifier, which no layout splits and no temporary shortens.
    Name,
    /// A string literal, which a namespace constant can shorten.
    Str,
    /// `callee(arguments)` or `callee{arguments}`, which splits between its
    /// arguments. A braced list without a callee needs `hoist_as` to go into
    /// a temporary.
    Call {
        callee: String,
        bracket: Bracket,
        arguments: Vec<Arg>,
        hoist_as: Option<&'static str>,
    },
    /// `callee(domain, [&](parameter) { return body; })`.
    Comprehension {
        callee: &'static str,
        domain: Box<Value>,
        parameter: String,
        body: String,
    },
    /// Anything else, which must fit its line.
    Other,
}

/// A rendered value.
#[derive(Debug, Clone)]
pub(super) struct Value {
    pub(super) text: String,
    pub(super) class: Class,
    pub(super) precedence: Precedence,
    /// A C++ literal, or a temporary or constant that holds one: an operand as
    /// it is, and a receiver or left operand through `lit`.
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

    /// A string literal.
    pub(super) fn string(text: &str) -> Self {
        Self {
            literal: true,
            form: Form::Str,
            ..Self::new(quoted(text), Class::Str)
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
        Self::bracketed(callee, Bracket::Paren, arguments, None, class)
    }

    /// `callee(values)`, with positional arguments.
    pub(super) fn call_of(callee: &str, values: &[Value], class: Class) -> Self {
        Self::call(
            callee,
            values.iter().cloned().map(Arg::new).collect(),
            class,
        )
    }

    /// `callee{arguments}`, or a braced list without a callee, which goes into
    /// a temporary as `hoist_as{arguments}`.
    pub(super) fn braced(
        callee: &str,
        arguments: Vec<Arg>,
        hoist_as: Option<&'static str>,
        class: Class,
    ) -> Self {
        Self::bracketed(callee, Bracket::Brace, arguments, hoist_as, class)
    }

    pub(super) fn bracketed(
        callee: &str,
        bracket: Bracket,
        arguments: Vec<Arg>,
        hoist_as: Option<&'static str>,
        class: Class,
    ) -> Self {
        let texts: Vec<String> = arguments.iter().map(Arg::text).collect();
        let text = format!(
            "{callee}{}{}{}",
            bracket.open(),
            texts.join(", "),
            bracket.close()
        );
        Self {
            form: Form::Call {
                callee: callee.to_owned(),
                bracket,
                arguments,
                hoist_as,
            },
            ..Self::new(text, class)
        }
    }

    /// `callee(domain, [&](parameter) { return body; })`.
    pub(super) fn comprehension(
        callee: &'static str,
        domain: Value,
        parameter: String,
        body: String,
    ) -> Self {
        let text = format!(
            "{callee}({}, {})",
            domain.text,
            joined_lambda(&parameter, &format!("return {body};"))
        );
        Self {
            form: Form::Comprehension {
                callee,
                domain: Box::new(domain),
                parameter,
                body,
            },
            ..Self::new(text, Class::Int)
        }
    }

    fn operator_result(text: String, precedence: Precedence, class: Class) -> Self {
        Self {
            precedence,
            ..Self::new(text, class)
        }
    }

    /// Whether no temporary can make the value shorter.
    pub(super) fn is_fixed(&self) -> bool {
        matches!(self.form, Form::Name)
            || matches!(
                self.form,
                Form::Call {
                    callee: ref name,
                    hoist_as: None,
                    ..
                } if name.is_empty()
            )
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

    /// The text as the right operand of a left-associative operator at
    /// `level`, where a literal needs no `lit`.
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

    /// This value where an operand of value type `std::int64_t` or `Dynamic`
    /// is needed.
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

    /// This value as a comprehension's or a function's domain.
    fn domain(&self) -> Self {
        match self.class {
            Class::Handle | Class::Dyn | Class::Record | Class::RecordItem | Class::RecordList => {
                self.clone()
            }
            _ => self.dynamic(),
        }
    }

    /// The receiver for `[]`.
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

    /// Whether `.dynamic()` applies.
    fn can_be_dynamic(&self) -> bool {
        !matches!(
            self.class,
            Class::Record | Class::RecordItem | Class::RecordList
        )
    }
}

/// `[&](parameter) { statement }`, as clang-format joins a short lambda.
pub(super) fn joined_lambda(parameter: &str, statement: &str) -> String {
    format!("[&]({parameter}) {{ {statement} }}")
}

/// The parameter of a comprehension's lambda.
fn parameter(identifier: Option<&str>) -> String {
    match identifier {
        Some(identifier) => format!("const auto &{identifier}"),
        None => "const auto &".to_owned(),
    }
}

/// The C++ type of a comprehension variable of this class.
pub(super) fn variable_type(class: Class) -> &'static str {
    if class == Class::RecordItem {
        "RecordExpr"
    } else {
        "Expr<Dynamic, Open>"
    }
}

/// An open comprehension: its domain is rendered, its body is being rendered.
struct OpenComprehension {
    variable: VariableId,
    domain: Value,
    /// The statements before the comprehension, set aside for the body's own.
    outer: Vec<Line>,
    outer_indent: usize,
    /// Whether a lambda for the body goes to the top of `builder`.
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
        BinaryOp::Or => ("||", Precedence::Or),
        BinaryOp::And => ("&&", Precedence::And),
        BinaryOp::Equal => ("==", Precedence::Equality),
        BinaryOp::NotEqual => ("!=", Precedence::Equality),
        BinaryOp::Less => ("<", Precedence::Relational),
        BinaryOp::LessEqual => ("<=", Precedence::Relational),
        BinaryOp::Greater => (">", Precedence::Relational),
        BinaryOp::GreaterEqual => (">=", Precedence::Relational),
        BinaryOp::Add => ("+", Precedence::Sum),
        BinaryOp::Subtract => ("-", Precedence::Sum),
        BinaryOp::Multiply => ("*", Precedence::Product),
        BinaryOp::Divide => ("/", Precedence::Product),
        BinaryOp::Remainder => ("%", Precedence::Product),
    }
}

fn is_comparison(precedence: Precedence) -> bool {
    matches!(precedence, Precedence::Equality | Precedence::Relational)
}

/// The text of `operand` beside an operator at `level`, with the parentheses
/// that compilers' `-Wparentheses` wants as well as those C++ needs: around a
/// comparison in a comparison, `&&` in `||`, and `!` before a comparison.
fn operand_text(operand: &Value, level: Precedence, left: bool) -> String {
    let warned = !operand.literal
        && ((is_comparison(level) && is_comparison(operand.precedence))
            || (level == Precedence::Or && operand.precedence == Precedence::And)
            || (is_comparison(level)
                && operand.precedence == Precedence::Unary
                && operand.text.starts_with('!')));
    if warned {
        format!("({})", operand.text)
    } else if left {
        operand.bound(level)
    } else {
        operand.right_of(level)
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

/// A float as a C++ literal that reads back as the same value.
fn float_literal(value: f64) -> String {
    format!("{value:?}")
}

/// Whether clang-tidy's `modernize-use-std-numbers` takes the literal of
/// `value` for a constant of `std::numbers`: a literal within its
/// `DiffThreshold`, 0.001 by default, of one. A minus sign is an operator in
/// C++, so the check sees the magnitude. The tolerance is a little wider than
/// the threshold, so that rounding at its edge cannot leave a literal out.
fn near_std_number(value: f64) -> bool {
    use std::f64::consts as c;
    const TOLERANCE: f64 = 0.0011;
    // std::numbers: e, log2e, log10e, pi, inv_pi, inv_sqrtpi, ln2, ln10,
    // sqrt2, sqrt3, inv_sqrt3, egamma and phi.
    const EGAMMA: f64 = 0.577_215_664_901_532_9;
    const PHI: f64 = 1.618_033_988_749_895;
    const SQRT_3: f64 = 1.732_050_807_568_877_2;
    const INV_SQRT_3: f64 = 0.577_350_269_189_625_8;
    const INV_SQRT_PI: f64 = 0.564_189_583_547_756_3;
    [
        c::E,
        c::LOG2_E,
        c::LOG10_E,
        c::PI,
        c::FRAC_1_PI,
        INV_SQRT_PI,
        c::LN_2,
        c::LN_10,
        c::SQRT_2,
        SQRT_3,
        INV_SQRT_3,
        EGAMMA,
        PHI,
    ]
    .iter()
    .any(|constant| (value.abs() - constant).abs() <= TOLERANCE)
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
            ExprOp::Float(value) => {
                self.near_std_number |= near_std_number(*value);
                Value::literal(float_literal(*value), Class::Float)
            }
            ExprOp::Str(text) => self.string_value(text),
            ExprOp::Host(host) => {
                let text = match host {
                    Host::IntMax => "host::int_max",
                    Host::IntMin => "host::int_min",
                };
                Value::atom(text.to_owned(), Class::Host)
            }
            ExprOp::Name(name) => Value::name(self.value_names[name.0].clone(), Class::Handle),
            ExprOp::Var(variable) => self.variable(*variable, state),
            ExprOp::Each(record) => Value::name(self.record_names[record.0].clone(), Class::Record),
            ExprOp::List(record) => Value::atom(
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
                    let operands = [values[0].integral(), values[1].integral()];
                    // `pow` alone may also find the C library's `::pow`.
                    Value::call_of("iris::build::pow", &operands, Class::Int)
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
                    Value::new(format!("{}[{}]", base.receiver(), index.text), class)
                })
            }
            ExprOp::Member(field) => {
                let base = state.pop();
                let field = Value::name(self.value_names[field.0].clone(), Class::Handle);
                self.combine(vec![base, field], |values| {
                    let base = values[0].has_fields();
                    let text = format!("{}.get({})", base.receiver(), values[1].text);
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
        // outside if its body becomes a lambda at the top of `builder`.
        for open in &mut state.comprehensions[position + 1..] {
            if !open.captures.contains(&variable) {
                open.captures.push(variable);
            }
        }
        Value::name(self.variable_names[variable.0].clone(), class)
    }

    fn unary(&mut self, op: UnaryOp, operand: Value) -> Value {
        self.combine(vec![operand], |values| {
            let (symbol, operand, class) = match op {
                UnaryOp::Minus => {
                    let operand = values[0].numeric();
                    let class = match operand.class {
                        Class::Handle => Class::Dyn,
                        Class::Host => Class::Int,
                        class => class,
                    };
                    ("-", operand, class)
                }
                UnaryOp::Not => ("!", values[0].integral(), Class::Int),
            };
            // `- -x` reads as a mistaken `--x`, so a unary operand gets
            // parentheses.
            let text = if !operand.literal && operand.precedence == Precedence::Unary {
                format!("{symbol}({})", operand.text)
            } else {
                format!("{symbol}{}", operand.bound(Precedence::Unary))
            };
            Value::operator_result(text, Precedence::Unary, class)
        })
    }

    fn binary(&mut self, op: BinaryOp, left: Value, right: Value) -> Value {
        let (symbol, precedence) = operator(op);
        self.combine(vec![left, right], |values| {
            let (mut left, mut right) = (values[0].clone(), values[1].clone());
            match op {
                BinaryOp::Equal | BinaryOp::NotEqual => {
                    let mixed = (left.class == Class::Str) != (right.class == Class::Str);
                    let open = |value: &Value| matches!(value.class, Class::Handle | Class::Dyn);
                    if mixed && !open(&left) && !open(&right) {
                        if left.can_be_dynamic() {
                            left = left.dynamic();
                        } else {
                            right = right.dynamic();
                        }
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
            let text = format!(
                "{} {symbol} {}",
                operand_text(&left, precedence, true),
                operand_text(&right, precedence, false)
            );
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
                Value::call_of(name, values, Class::Int)
            }
        })
    }

    fn open_comprehension(&mut self, variable: VariableId, domain: Value, state: &mut Evaluation) {
        let spelling = self.program.variables[variable.0].clone();
        let class = match domain.class {
            Class::Record | Class::RecordList => Class::RecordItem,
            _ => Class::Dyn,
        };
        let spelling = self.string_value(&spelling);
        let domain = self.combine(vec![domain, spelling], |values| {
            let over = Value::call_of("over", &[values[0].domain()], Class::Other);
            let callee = format!("{}.named", over.text);
            Value::call(&callee, vec![Arg::new(values[1].clone())], Class::Other)
        });
        let outlined = self.indent + 1 > MAX_INDENT;
        state.comprehensions.push(OpenComprehension {
            variable,
            domain,
            outer: std::mem::take(&mut self.lines),
            outer_indent: self.indent,
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
        let identifier = binding
            .used
            .then(|| self.variable_names[open.variable.0].clone());
        let name = comprehension_name(kind);
        let mut body = body;
        if !fits(self.column(), &format!("return {};", body.text)) {
            // An expanded lambda's `return` must fit its line.
            self.hoist(&mut body);
        }
        let body_lines = std::mem::replace(&mut self.lines, open.outer);
        let body_indent = std::mem::replace(&mut self.indent, open.outer_indent);
        if !open.outlined {
            let value = Value::comprehension(
                name,
                open.domain,
                parameter(identifier.as_deref()),
                body.text,
            );
            if body_lines.is_empty() {
                return value;
            }
            // The body needs statements: the lambda is laid out in full.
            let temporary = self.temporary();
            self.expanded(&format!("auto {temporary} = "), value, body_lines);
            return Value::name(temporary, Class::Int);
        }
        // The body goes to a lambda at the top of `builder`, which sees only
        // what it is given.
        let function = format!("body_{}", self.next_function());
        let mut parameters = Vec::new();
        let mut arguments = Vec::new();
        if let Some(identifier) = &identifier {
            parameters.push(format!(
                "const {} &{identifier}",
                variable_type(binding.class)
            ));
            arguments.push(identifier.clone());
        }
        for &captured in &open.captures {
            let position = state
                .bindings
                .iter()
                .rposition(|binding| binding.variable == captured)
                .expect("a captured variable is bound outside");
            let captured_identifier = self.variable_names[captured.0].clone();
            let ty = variable_type(state.bindings[position].class);
            parameters.push(format!("const {ty} &{captured_identifier}"));
            arguments.push(captured_identifier);
        }
        let mut lines = body_lines;
        lines.push(Line {
            column: body_indent * TAB,
            text: format!("return {};", body.text),
        });
        self.helper(&function, &parameters, lines);
        if open.captures.is_empty() {
            let call = Value::name(function, Class::Other);
            let text = format!("{name}({}, {})", open.domain.text, call.text);
            return Value {
                form: Form::Call {
                    callee: name.to_owned(),
                    bracket: Bracket::Paren,
                    arguments: vec![Arg::new(open.domain), Arg::new(call)],
                    hoist_as: None,
                },
                ..Value::new(text, Class::Int)
            };
        }
        let call = format!("{function}({})", arguments.join(", "));
        Value::comprehension(name, open.domain, parameter(identifier.as_deref()), call)
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
            .filter(|operand| {
                operand.text.len() >= SHORTEST_HOIST && !operand.is_fixed() && eligible(operand)
            })
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
