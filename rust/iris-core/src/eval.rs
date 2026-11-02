//! One continuation machine for ordinary values, records, and check witnesses.
use crate::{
    ErrorCode, Failure, Value, ValueRef, numeric,
    reader::{Interpreter, WorkKind, expression, tokens},
    value::LeafStorage,
};
use iris_sema::{DefinitionId, RepeatId};
use iris_syntax::{SyntaxKind as K, SyntaxNode};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    rc::Rc,
};
mod records;
use crate::reader::repeat::RecordId;
pub(crate) use records::ListSelection;

#[derive(Clone)]
enum Root<'a> {
    Borrowed(&'a Value),
    Temporary(Rc<Value>),
}
#[derive(Clone)]
pub(crate) struct View<'a> {
    root: Root<'a>,
    level: usize,
    row: usize,
    record_parent: Option<RecordId>,
}
impl<'a> View<'a> {
    fn borrowed(value: &'a Value) -> Self {
        Self {
            root: Root::Borrowed(value),
            level: 0,
            row: 0,
            record_parent: None,
        }
    }
    fn owned(value: Value) -> Self {
        Self {
            root: Root::Temporary(Rc::new(value)),
            level: 0,
            row: 0,
            record_parent: None,
        }
    }
    fn value(&self) -> &Value {
        match &self.root {
            Root::Borrowed(v) => v,
            Root::Temporary(v) => v,
        }
    }
    fn key(&self) -> usize {
        self.value() as *const Value as usize
    }
    fn as_ref(&self) -> ValueRef<'_> {
        ValueRef {
            value: self.value(),
            level: self.level,
            row: self.row,
        }
    }
    pub(crate) fn len(&self) -> Option<usize> {
        self.as_ref().children().ok().map(|r| r.len())
    }
    pub(crate) fn child(&self, index: usize) -> Option<Self> {
        let range = self.as_ref().children().ok()?;
        (index < range.len()).then(|| Self {
            root: self.root.clone(),
            level: self.level + 1,
            row: range.start + index,
            record_parent: self.record_parent,
        })
    }
    fn truth(&self) -> bool {
        if let Some(length) = self.len() {
            return length != 0;
        }
        let scalar = self.as_ref();
        match scalar.value_type() {
            "int" => scalar.as_int().expect("int") != 0,
            "float" => scalar.as_float().expect("float") != 0.0,
            _ => !scalar.as_str().expect("string").is_empty(),
        }
    }
}
#[derive(Clone)]
pub(crate) enum Kind<'a> {
    Value(View<'a>),
    Record(RecordId),
    Records(ListSelection),
}
impl Kind<'_> {
    fn view(&self) -> &View<'_> {
        let Self::Value(value) = self else {
            unreachable!("statically a value")
        };
        value
    }
}
pub(crate) struct ResultValue<'a> {
    pub kind: Kind<'a>,
    pub witness: Option<usize>,
}
impl ResultValue<'_> {
    fn integer(value: i64) -> Self {
        Self {
            kind: Kind::Value(View::owned(Value::int(value))),
            witness: None,
        }
    }
    pub(crate) fn truth(&self) -> bool {
        self.kind.view().truth()
    }
}
pub(crate) struct Witness {
    pub index: usize,
    pub domain: SyntaxNode,
    pub record: Option<RepeatId>,
    pub position: Option<crate::input::Position>,
    pub inner: Option<usize>,
}

pub(crate) struct Evaluator<'a, 's, 'i> {
    pub reader: &'a Interpreter<'s, 'i>,
    names: HashMap<String, Option<View<'a>>>,
    bindings: HashMap<DefinitionId, Kind<'a>>,
    fields: HashMap<(usize, RecordId), View<'a>>,
    spans: HashMap<(usize, usize, usize), usize>,
    pub selected: HashMap<RepeatId, RecordId>,
    pub dependency_slots: HashMap<String, usize>,
    pub dependencies: Vec<Option<Kind<'a>>>,
    pub witnesses: Vec<Witness>,
    #[cfg(test)]
    pub charges: Vec<crate::reader::Charge>,
}
impl Interpreter<'_, '_> {
    pub fn eval(&mut self, node: &SyntaxNode) -> Result<Value, Failure> {
        let mut evaluator = Evaluator::new(self);
        let result = evaluator.evaluate(node).and_then(|result| {
            let view = result.kind.view();
            view.as_ref()
                .copy_with(&self.input.control, |count| {
                    self.work(WorkKind::Copy, count)
                })
                .map_err(Into::into)
        });
        #[cfg(test)]
        {
            let charges = std::mem::take(&mut evaluator.charges);
            drop(evaluator);
            self.charges.extend(charges);
        }
        result
    }
}

struct Aggregate {
    function: String,
    next: Option<SyntaxNode>,
    extreme: Option<i64>,
    sum: i64,
    seen: HashSet<i64>,
    distinct: bool,
}
struct Comprehension<'a> {
    function: String,
    node: SyntaxNode,
    body: SyntaxNode,
    domain: Kind<'a>,
    binding: DefinitionId,
    previous: Option<Kind<'a>>,
    index: usize,
    length: usize,
    result: i64,
    mark: usize,
}
enum Continue<'a> {
    Node(SyntaxNode),
    Unary(String),
    Left {
        operator: String,
        right: SyntaxNode,
    },
    Binary {
        operator: String,
        left: ResultValue<'a>,
    },
    Base(SyntaxNode),
    Index(Kind<'a>),
    Field(SyntaxNode),
    Size(String),
    Aggregate(Aggregate),
    Domain(SyntaxNode),
    Body(Comprehension<'a>),
}
impl<'a, 's, 'i> Evaluator<'a, 's, 'i> {
    pub fn new(reader: &'a Interpreter<'s, 'i>) -> Self {
        Self {
            reader,
            names: HashMap::new(),
            bindings: HashMap::new(),
            fields: HashMap::new(),
            spans: HashMap::new(),
            selected: HashMap::new(),
            dependency_slots: HashMap::new(),
            dependencies: vec![],
            witnesses: vec![],
            #[cfg(test)]
            charges: vec![],
        }
    }
    pub(crate) fn charge(&mut self, units: u128) -> Result<(), Failure> {
        #[cfg(test)]
        if self.reader.capture_charges {
            self.charges.push(crate::reader::Charge {
                expression: true,
                units,
                statement_line: self.reader.statement.map(|s| s.0),
                context: self.reader.execution_context()?,
            });
        }
        self.reader.charge_expression(units)
    }
    pub(crate) fn leaf_span(&mut self, view: &View<'_>) -> Result<std::ops::Range<usize>, Failure> {
        Ok(self.boundary(view, view.row)?..self.boundary(view, view.row + 1)?)
    }
    fn boundary(&mut self, view: &View<'_>, row: usize) -> Result<usize, Failure> {
        let mut level = view.level;
        let mut row = row;
        let root = view.key();
        let mut pending = Vec::new();
        while level < view.value().depth() {
            self.reader.input.control.check()?;
            self.reader.work(WorkKind::Metadata, 1);
            if let Some(&leaf) = self.spans.get(&(root, level, row)) {
                row = leaf;
                break;
            }
            pending.push((root, level, row));
            row = view.value().levels[level].offsets[row];
            level += 1;
        }
        for key in pending {
            self.reader.input.control.check()?;
            self.spans.insert(key, row);
        }
        Ok(row)
    }
    pub fn evaluate(&mut self, node: &SyntaxNode) -> Result<ResultValue<'a>, Failure> {
        let result = self.evaluate_inner(node);
        if result.is_err() {
            self.bindings.clear();
            self.witnesses.clear();
        }
        result
    }
    fn evaluate_inner(&mut self, node: &SyntaxNode) -> Result<ResultValue<'a>, Failure> {
        let mut work = vec![Continue::Node(node.clone())];
        let mut results = Vec::<ResultValue<'a>>::new();
        while let Some(next) = work.pop() {
            self.reader.input.control.check()?;
            match next {
                Continue::Node(node) => {
                    self.reader.work(WorkKind::Dispatch, 1);
                    match node.kind() {
                        K::Literal => {
                            let token = &tokens(&node)[0];
                            let control = &self.reader.input.control;
                            let value = match token.kind() {
                                K::Integer => Value::int(
                                    numeric::parse_integer_controlled(
                                        token.text().as_bytes(),
                                        control,
                                    )?
                                    .expect("validated literal"),
                                ),
                                K::Float => Value::float(
                                    numeric::parse_float_controlled(
                                        token.text().as_bytes(),
                                        control,
                                    )?
                                    .expect("validated literal"),
                                ),
                                K::String => {
                                    let index = &self.reader.spec.index;
                                    let range = token.text_range();
                                    Value::string(
                                        match index
                                            .pattern_literals
                                            .get(&range)
                                            .or_else(|| index.string_literals.get(&range))
                                        {
                                            Some(text) => control.copy_text(text)?,
                                            None => {
                                                unreachable!("string literal indexed at spec load")
                                            }
                                        },
                                    )
                                }
                                _ => unreachable!("literal"),
                            };
                            results.push(ResultValue {
                                kind: Kind::Value(View::owned(value)),
                                witness: None,
                            });
                        }
                        K::Name => results.push(ResultValue {
                            kind: self.name(&node)?,
                            witness: None,
                        }),
                        K::HostConstant => {
                            let token = &tokens(&node)[0];
                            let id = self.reader.spec.index.references[&token.text_range()];
                            let iris_sema::Resolution::HostConstant(value) =
                                self.reader.spec.analysis.model.reference(id).resolution
                            else {
                                unreachable!("constant")
                            };
                            results.push(ResultValue::integer(value));
                        }
                        K::Parenthesized => work.push(Continue::Node(expression(&node))),
                        K::Unary => {
                            work.push(Continue::Unary(tokens(&node)[0].text().into()));
                            work.push(Continue::Node(expression(&node)));
                        }
                        K::Binary | K::Power => {
                            let mut children = node.children();
                            let left = children.next().expect("left");
                            let right = children.next().expect("right");
                            work.push(Continue::Left {
                                operator: tokens(&node)[0].text().into(),
                                right,
                            });
                            work.push(Continue::Node(left));
                        }
                        K::IndexExpression => {
                            work.push(Continue::Base(node.clone()));
                            work.push(Continue::Node(expression(&node)));
                        }
                        K::Field => {
                            work.push(Continue::Field(node.clone()));
                            work.push(Continue::Node(expression(&node)));
                        }
                        K::Call => self.start_call(&node, &mut work, &mut results)?,
                        K::Comprehension => {
                            work.push(Continue::Domain(node.clone()));
                            work.push(Continue::Node(expression(&node)));
                        }
                        _ => unreachable!("expression kind"),
                    }
                }
                Continue::Unary(operator) => {
                    let value = results.pop().expect("unary operand");
                    let view = value.kind.view();
                    let result = if operator == "!" {
                        Value::int(i64::from(!view.truth()))
                    } else if let Ok(n) = view.as_ref().as_int() {
                        Value::int(n.checked_neg().ok_or_else(|| {
                            self.reader
                                .error(ErrorCode::IntegerOverflow, "integer overflow in '-'")
                        })?)
                    } else if let Ok(n) = view.as_ref().as_float() {
                        Value::float(-n)
                    } else {
                        return Err(self
                            .reader
                            .error(ErrorCode::IntegerType, "'-' needs integers"));
                    };
                    results.push(ResultValue {
                        kind: Kind::Value(View::owned(result)),
                        witness: None,
                    });
                }
                Continue::Left { operator, right } => {
                    let left = results.pop().expect("left");
                    if (operator == "&&" && !left.truth()) || (operator == "||" && left.truth()) {
                        let truth = left.truth();
                        results.push(ResultValue {
                            kind: Kind::Value(View::owned(Value::int(i64::from(truth)))),
                            witness: if truth { None } else { left.witness },
                        });
                    } else {
                        work.push(Continue::Binary { operator, left });
                        work.push(Continue::Node(right));
                    }
                }
                Continue::Binary { operator, left } => {
                    let right = results.pop().expect("right");
                    if matches!(operator.as_str(), "&&" | "||") {
                        let truth = right.truth();
                        results.push(ResultValue {
                            kind: Kind::Value(View::owned(Value::int(i64::from(truth)))),
                            witness: if truth { None } else { right.witness },
                        });
                    } else {
                        let value =
                            if matches!(operator.as_str(), "==" | "!=" | "<" | ">" | "<=" | ">=") {
                                self.compare(
                                    &operator,
                                    left.kind.view().as_ref(),
                                    right.kind.view().as_ref(),
                                )?
                            } else {
                                self.integer_arithmetic(
                                    &operator,
                                    left.kind.view().as_ref(),
                                    right.kind.view().as_ref(),
                                )?
                            };
                        results.push(ResultValue {
                            kind: Kind::Value(View::owned(value)),
                            witness: None,
                        });
                    }
                }
                Continue::Base(node) => {
                    let base = results.pop().expect("index base").kind;
                    self.length(&base, ErrorCode::IndexScalar, "cannot index a scalar")?;
                    work.push(Continue::Index(base));
                    work.push(Continue::Node(node.children().nth(1).expect("index")));
                }
                Continue::Index(base) => {
                    let index = results
                        .pop()
                        .expect("index operand")
                        .kind
                        .view()
                        .as_ref()
                        .as_int()
                        .map_err(|_| {
                            self.reader
                                .error(ErrorCode::IndexType, "array index must be an integer")
                        })?;
                    let length =
                        self.length(&base, ErrorCode::IndexScalar, "cannot index a scalar")?;
                    if index < 0 || index as u64 >= length as u64 {
                        return Err(self
                            .reader
                            .error(ErrorCode::IndexRange, "array index out of range"));
                    }
                    let kind = self.element(&base, index as usize)?;
                    results.push(ResultValue {
                        kind,
                        witness: None,
                    });
                }
                Continue::Field(node) => {
                    let Kind::Record(record) = results.pop().expect("field base").kind else {
                        unreachable!("resolved record")
                    };
                    let kind = self.field(&node, record)?;
                    results.push(ResultValue {
                        kind,
                        witness: None,
                    });
                }
                Continue::Size(name) => {
                    let value = results.pop().expect("size argument");
                    let length = self.length(
                        &value.kind,
                        ErrorCode::FunctionType,
                        &format!("'{name}' needs an array"),
                    )?;
                    results.push(ResultValue::integer(length as i64));
                }
                Continue::Aggregate(mut state) => {
                    let value = results.pop().expect("aggregate argument");
                    self.add_argument(&mut state, value.kind.view())?;
                    if let Some(argument) = state.next.take() {
                        state.next = argument.next_sibling();
                        work.push(Continue::Aggregate(state));
                        work.push(Continue::Node(argument));
                    } else {
                        let result = match state.function.as_str() {
                            "min" | "max" => state.extreme,
                            "sum" => Some(state.sum),
                            "distinct" => Some(i64::from(state.distinct)),
                            _ => unreachable!("aggregate"),
                        }
                        .ok_or_else(|| {
                            self.reader.error(
                                ErrorCode::FunctionEmpty,
                                format!("'{}' needs at least one value", state.function),
                            )
                        })?;
                        results.push(ResultValue::integer(result));
                    }
                }
                Continue::Domain(node) => {
                    let domain = results.pop().expect("domain").kind;
                    let length = self.length(
                        &domain,
                        ErrorCode::FunctionType,
                        "comprehension needs an array",
                    )?;
                    let info = &self.reader.spec.analysis.model.comprehensions()
                        [self.reader.spec.index.comprehensions[&node.text_range()]];
                    let function = tokens(&node)[0].text().to_owned();
                    let body = node.children().nth(1).expect("body");
                    let binding = info.binding;
                    let previous = self.bindings.remove(&binding);
                    let state = Comprehension {
                        result: i64::from(function == "all"),
                        function,
                        node,
                        body,
                        domain,
                        binding,
                        previous,
                        index: 0,
                        length,
                        mark: self.witnesses.len(),
                    };
                    self.next_body(state, &mut work, &mut results)?;
                }
                Continue::Body(mut state) => {
                    let value = results.pop().expect("body result");
                    let truth = value.truth();
                    if state.function == "all" && !truth {
                        let witness = self.explain(&state, value.witness);
                        self.restore_binding(state.binding, state.previous);
                        results.push(ResultValue {
                            kind: Kind::Value(View::owned(Value::int(0))),
                            witness: Some(witness),
                        });
                    } else if state.function == "any" && truth {
                        self.restore_binding(state.binding, state.previous);
                        self.witnesses.truncate(state.mark);
                        results.push(ResultValue::integer(1));
                    } else {
                        match state.function.as_str() {
                            "sum" => {
                                let integer =
                                    value.kind.view().as_ref().as_int().map_err(|_| {
                                        self.reader
                                            .error(ErrorCode::IntegerType, "'sum' needs integers")
                                    })?;
                                state.result =
                                    state.result.checked_add(integer).ok_or_else(|| {
                                        self.reader.error(
                                            ErrorCode::IntegerOverflow,
                                            "integer overflow in 'sum'",
                                        )
                                    })?;
                            }
                            "count" => state.result += i64::from(truth),
                            _ => {}
                        }
                        self.witnesses.truncate(state.mark);
                        state.index += 1;
                        self.next_body(state, &mut work, &mut results)?;
                    }
                }
            }
        }
        Ok(results.pop().expect("one expression result"))
    }
    fn start_call(
        &mut self,
        node: &SyntaxNode,
        work: &mut Vec<Continue<'a>>,
        results: &mut Vec<ResultValue<'a>>,
    ) -> Result<(), Failure> {
        let name = self
            .reader
            .input
            .control
            .copy_text(tokens(node)[0].text())?;
        if name == "defined" {
            let mut argument = expression(node);
            while argument.kind() == K::Parenthesized {
                self.reader.input.control.check()?;
                argument = expression(&argument);
            }
            let available = self.is_defined(&argument)?;
            results.push(ResultValue::integer(i64::from(available)));
            return Ok(());
        }
        let argument = expression(node);
        if matches!(name.as_str(), "size" | "count") {
            work.push(Continue::Size(name));
        } else {
            work.push(Continue::Aggregate(Aggregate {
                function: name,
                next: argument.next_sibling(),
                extreme: None,
                sum: 0,
                seen: HashSet::new(),
                distinct: true,
            }));
        }
        work.push(Continue::Node(argument));
        Ok(())
    }
    fn length(&self, kind: &Kind<'_>, code: ErrorCode, message: &str) -> Result<usize, Failure> {
        match kind {
            Kind::Records(list) => Ok(self.list_len(*list)),
            Kind::Value(value) => value.len().ok_or_else(|| self.reader.error(code, message)),
            Kind::Record(_) => unreachable!("array required statically"),
        }
    }
    pub(crate) fn element(&mut self, domain: &Kind<'a>, index: usize) -> Result<Kind<'a>, Failure> {
        match domain {
            Kind::Value(value) => {
                let selected = value.child(index).expect("valid element");
                let leaves = self.leaf_span(&selected)?.len();
                self.charge(1 + leaves as u128)?;
                Ok(Kind::Value(selected))
            }
            Kind::Records(list) => {
                let record = self.list_record(*list, index);
                self.charge(1)?;
                Ok(Kind::Record(record))
            }
            Kind::Record(_) => unreachable!("domain"),
        }
    }
    fn restore_binding(&mut self, id: DefinitionId, previous: Option<Kind<'a>>) {
        if let Some(value) = previous {
            self.bindings.insert(id, value);
        } else {
            self.bindings.remove(&id);
        }
    }
    fn next_body(
        &mut self,
        state: Comprehension<'a>,
        work: &mut Vec<Continue<'a>>,
        results: &mut Vec<ResultValue<'a>>,
    ) -> Result<(), Failure> {
        if state.index == state.length {
            self.restore_binding(state.binding, state.previous);
            self.witnesses.truncate(state.mark);
            results.push(ResultValue::integer(state.result));
        } else {
            let selected = self.element(&state.domain, state.index)?;
            self.bindings.insert(state.binding, selected);
            let body = state.body.clone();
            work.push(Continue::Body(state));
            work.push(Continue::Node(body));
        }
        Ok(())
    }
    fn explain(&mut self, state: &Comprehension<'a>, inner: Option<usize>) -> usize {
        let info = &self.reader.spec.analysis.model.comprehensions()
            [self.reader.spec.index.comprehensions[&state.node.text_range()]];
        let record = info.record_elements;
        let position = record.and_then(|repeat| {
            let list = match &state.domain {
                Kind::Records(list) => Some(*list),
                Kind::Value(view) => self.list(
                    repeat,
                    view.record_parent.or_else(|| self.parent_for(repeat)),
                ),
                Kind::Record(_) => None,
            }?;
            (state.index < self.list_len(list))
                .then(|| self.record_start(self.list_record(list, state.index)))
        });
        self.reader.work(WorkKind::Link, 1);
        let id = self.witnesses.len();
        self.witnesses.push(Witness {
            index: state.index,
            domain: info.domain.clone(),
            record,
            position,
            inner,
        });
        id
    }
    fn add_argument(&mut self, state: &mut Aggregate, view: &View<'_>) -> Result<(), Failure> {
        let span = self.leaf_span(view)?;
        if !span.is_empty() && view.value().value_type() != "int" {
            return Err(self.reader.error(
                ErrorCode::FunctionType,
                format!("'{}' has an invalid argument type", state.function),
            ));
        }
        self.charge(span.len() as u128)?;
        if let LeafStorage::Int(integers) = &view.value().leaves {
            for (index, &integer) in integers[span].iter().enumerate() {
                if index.is_multiple_of(crate::cancellation::VALUE_CHUNK) {
                    self.reader.input.control.check()?;
                }
                self.reader.work(WorkKind::Leaf, 1);
                match state.function.as_str() {
                    "min" => {
                        state.extreme = Some(state.extreme.map_or(integer, |n| n.min(integer)))
                    }
                    "max" => {
                        state.extreme = Some(state.extreme.map_or(integer, |n| n.max(integer)))
                    }
                    "sum" => {
                        state.sum = state.sum.checked_add(integer).ok_or_else(|| {
                            self.reader
                                .error(ErrorCode::IntegerOverflow, "integer overflow in 'sum'")
                        })?
                    }
                    "distinct" => state.distinct &= state.seen.insert(integer),
                    _ => unreachable!("aggregate"),
                }
            }
        }
        Ok(())
    }
    fn integer_arithmetic(
        &self,
        operator: &str,
        left: ValueRef<'_>,
        right: ValueRef<'_>,
    ) -> Result<Value, Failure> {
        let a = left.as_int().map_err(|_| {
            self.reader.error(
                ErrorCode::IntegerType,
                format!("'{operator}' needs integers"),
            )
        })?;
        let b = right.as_int().map_err(|_| {
            self.reader.error(
                ErrorCode::IntegerType,
                format!("'{operator}' needs integers"),
            )
        })?;
        if matches!(operator, "/" | "%") && b == 0 {
            return Err(self
                .reader
                .error(ErrorCode::DivisionZero, "division by zero"));
        }
        if operator == "^" && b < 0 {
            return Err(self
                .reader
                .error(ErrorCode::Exponent, "negative exponent in '^'"));
        }
        let result = match operator {
            "+" => a.checked_add(b),
            "-" => a.checked_sub(b),
            "*" => a.checked_mul(b),
            "/" => a.checked_div(b),
            "%" => a.checked_rem(b),
            "^" => numeric::integer_power(a, b),
            _ => unreachable!("arithmetic operator"),
        };
        result.map(Value::int).ok_or_else(|| {
            self.reader.error(
                ErrorCode::IntegerOverflow,
                format!("integer overflow in '{operator}'"),
            )
        })
    }
    fn compare(
        &self,
        operator: &str,
        left: ValueRef<'_>,
        right: ValueRef<'_>,
    ) -> Result<Value, Failure> {
        if left.depth() > 0 || right.depth() > 0 {
            return Err(self.reader.error(
                ErrorCode::ComparisonType,
                format!("'{operator}' compares single values, not arrays"),
            ));
        }
        let a = left;
        let b = right;
        let order = match (a.as_int(), b.as_int()) {
            (Ok(a), Ok(b)) => Some(a.cmp(&b)),
            _ if a.value_type() != "string" && b.value_type() != "string" => {
                let a = a
                    .as_float()
                    .unwrap_or_else(|_| a.as_int().expect("numeric scalar") as f64);
                let b = b
                    .as_float()
                    .unwrap_or_else(|_| b.as_int().expect("numeric scalar") as f64);
                a.partial_cmp(&b)
            }
            _ if matches!(operator, "==" | "!=") => {
                let equal = match a.as_str().ok().zip(b.as_str().ok()) {
                    Some((a, b)) => self
                        .reader
                        .input
                        .control
                        .equal_bytes(a.as_bytes(), b.as_bytes())?,
                    None => false,
                };
                if equal { Some(Ordering::Equal) } else { None }
            }
            _ => {
                return Err(self.reader.error(
                    ErrorCode::ComparisonType,
                    format!("'{operator}' compares numbers"),
                ));
            }
        };
        let result = match operator {
            "==" => order == Some(Ordering::Equal),
            "!=" => order != Some(Ordering::Equal),
            "<" => order == Some(Ordering::Less),
            ">" => order == Some(Ordering::Greater),
            "<=" => matches!(order, Some(Ordering::Less | Ordering::Equal)),
            ">=" => matches!(order, Some(Ordering::Greater | Ordering::Equal)),
            _ => unreachable!("comparison operator"),
        };
        Ok(Value::int(i64::from(result)))
    }
}

#[cfg(test)]
mod tests;
