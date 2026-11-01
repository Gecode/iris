use crate::{
    Error, ErrorCode, Failure, Spec, UnsupportedFeature, Value,
    input::{Input, Position, Token},
    numeric,
    value::{Frame, LeafStorage},
};
use iris_sema::{DefinitionId, DefinitionKind, ReferenceId, RepeatId, Resolution, SemanticModel};
use iris_syntax::{DecodedString, SyntaxKind as K, SyntaxNode, SyntaxToken, TextRange};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct ModelIndex {
    definitions: HashMap<TextRange, DefinitionId>,
    pub references: HashMap<TextRange, ReferenceId>,
    pub repeats: HashMap<TextRange, RepeatId>,
    pub checks: HashMap<TextRange, usize>,
    pub expressions: HashMap<TextRange, usize>,
    pub comprehensions: HashMap<TextRange, usize>,
    pub structural: HashMap<String, iris_sema::Denotation>,
    patterns: HashMap<TextRange, pattern::Pattern>,
    pub pattern_literals: HashMap<TextRange, String>,
    sections: HashMap<TextRange, sections::Sections>,
}
impl ModelIndex {
    pub fn new(model: &SemanticModel, root: SyntaxNode) -> Self {
        let named: HashMap<_, _> = model
            .named_types()
            .iter()
            .map(|t| (t.definition, t.kind))
            .collect();
        let kinds = model
            .references()
            .iter()
            .filter_map(|r| match r.resolution {
                Resolution::NamedType(id) => Some((r.name_range, named[&id])),
                _ => None,
            })
            .collect();
        let sections = root
            .descendants()
            .filter(|n| n.kind() == K::Sections)
            .map(|n| (n.text_range(), sections::Sections::new(n)))
            .collect();
        let patterns = root
            .descendants()
            .filter(|n| n.kind() == K::Pattern)
            .map(|n| (n.text_range(), pattern::Pattern::new(&n, &kinds)))
            .collect();
        let pattern_literals = root
            .descendants()
            .filter(|n| n.kind() == K::Pattern)
            .flat_map(|n| {
                n.descendants()
                    .filter(|n| n.kind() == K::Literal)
                    .collect::<Vec<_>>()
            })
            .flat_map(|n| tokens(&n))
            .filter(|t| t.kind() == K::String)
            .map(|t| {
                let inner = DecodedString::new(&format!("\"{}\"", t.text())).text;
                (t.text_range(), DecodedString::new(&inner).text)
            })
            .collect();
        Self {
            checks: model
                .checks()
                .iter()
                .enumerate()
                .map(|(i, c)| (c.node.text_range(), i))
                .collect(),
            expressions: model
                .expressions()
                .iter()
                .enumerate()
                .map(|(i, e)| (e.node.text_range(), i))
                .collect(),
            comprehensions: model
                .comprehensions()
                .iter()
                .enumerate()
                .map(|(i, c)| (c.node.text_range(), i))
                .collect(),
            structural: model
                .repeats()
                .iter()
                .enumerate()
                .flat_map(|(i, r)| {
                    [
                        (
                            r.list_definition,
                            iris_sema::Denotation::RecordList(RepeatId(i)),
                        ),
                        (
                            r.each_definition,
                            iris_sema::Denotation::Record(RepeatId(i)),
                        ),
                    ]
                    .into_iter()
                    .filter_map(|(id, denotation)| {
                        id.map(|id| (model.definition(id).name.clone(), denotation))
                    })
                })
                .collect(),
            patterns,
            sections,
            pattern_literals,
            repeats: model
                .repeats()
                .iter()
                .enumerate()
                .map(|(id, repeat)| (repeat.node.text_range(), RepeatId(id)))
                .collect(),
            definitions: model
                .definitions()
                .iter()
                .enumerate()
                .filter(|(_, d)| d.kind == DefinitionKind::NamedType)
                .map(|(id, d)| (d.node.text_range(), DefinitionId(id)))
                .collect(),
            references: model
                .references()
                .iter()
                .enumerate()
                .map(|(id, reference)| (reference.name_range, ReferenceId(id)))
                .collect(),
        }
    }
}
pub(crate) mod checks;
mod pattern;
mod reads;
pub(crate) mod repeat;
mod sections;
use repeat::{RecordLists, RepeatState};
use sections::SectionsState;

enum Work {
    Statement(SyntaxNode),
    Sequence(Option<SyntaxNode>),
    RepeatNext,
    AfterBody,
    SectionsNext,
    AfterSectionBody,
}

enum ExecutionContext {
    Repeat(usize),
    Section(TextRange),
}

#[derive(Clone)]
enum EvaluatedType {
    Int,
    Float,
    Num,
    Word,
    Rest,
    Range {
        lower: Option<i64>,
        upper: Option<i64>,
    },
    Alphabet(Alphabet),
}
#[derive(Clone)]
struct Alphabet {
    symbols: Vec<u8>,
    blank: Vec<u8>,
    strip: Vec<u8>,
    base: i64,
}
impl EvaluatedType {
    fn empty_value(&self) -> Value {
        Value::empty(match self {
            Self::Float => "float",
            Self::Word | Self::Rest => "string",
            _ => "int",
        })
    }
}
pub(crate) struct Interpreter<'s, 'i> {
    pub spec: &'s Spec,
    pub input: Input<'i>,
    pub frames: Vec<Frame>,
    types: HashMap<DefinitionId, EvaluatedType>,
    pub(crate) statement: Option<(usize, TextRange)>,
    pub(crate) context: Option<String>,
    pub repeats: Vec<RepeatState>,
    pub records: RecordLists,
    sections: Vec<SectionsState>,
    contexts: Vec<ExecutionContext>,
    source_lines: Vec<usize>,
    #[cfg(test)]
    pub(crate) charges: Vec<Charge>,
    #[cfg(test)]
    pub(crate) capture_charges: bool,
    #[cfg(test)]
    pub(crate) work: std::cell::Cell<[usize; 6]>,
}
impl<'s, 'i> Interpreter<'s, 'i> {
    pub fn new(spec: &'s Spec, input: Input<'i>) -> Self {
        Self {
            spec,
            input,
            frames: vec![Frame::default()],
            types: HashMap::new(),
            statement: None,
            context: None,
            repeats: vec![],
            records: RecordLists::default(),
            sections: vec![],
            contexts: vec![],
            #[cfg(test)]
            charges: vec![],
            #[cfg(test)]
            capture_charges: true,
            #[cfg(test)]
            work: std::cell::Cell::new([0; 6]),
            source_lines: std::iter::once(0)
                .chain(
                    spec.source
                        .iter()
                        .enumerate()
                        .filter(|(_, byte)| **byte == b'\n')
                        .map(|(i, _)| i + 1),
                )
                .collect(),
        }
    }
    pub fn read(mut self) -> Result<crate::Data, Failure> {
        self.execute(self.spec.parsed.syntax())?;
        self.statement = None;
        self.context = None;
        if !self.input.settings.ignore_trailing {
            self.input.skip_to_data();
            if self.input.position().offset < self.input.bytes.len() {
                return Err(self
                    .error(
                        ErrorCode::TrailingInput,
                        "unexpected input after the end of the spec",
                    )
                    .into());
            }
        }
        Ok(self
            .frames
            .pop()
            .expect("root frame exists throughout a read")
            .into_data())
    }
    pub(crate) fn execute(&mut self, root: SyntaxNode) -> Result<(), Failure> {
        let mut work = vec![Work::Sequence(root.first_child())];
        while let Some(next) = work.pop() {
            match next {
                Work::Sequence(Some(node)) => {
                    work.push(Work::Sequence(node.next_sibling()));
                    work.push(Work::Statement(node));
                }
                Work::Sequence(None) => {}
                Work::RepeatNext => {
                    if self.begin_iteration()? {
                        work.push(Work::AfterBody);
                        work.push(Work::Statement(
                            self.repeats.last().expect("active repeat").body.clone(),
                        ));
                    } else {
                        self.finish_repeat()?;
                    }
                }
                Work::AfterBody => {
                    self.collect_iteration()?;
                    work.push(Work::RepeatNext);
                }
                Work::SectionsNext => self.next_section(&mut work)?,
                Work::AfterSectionBody => self.after_section_body(&mut work)?,
                Work::Statement(node) => {
                    // A block is not a statement of its own: diagnostics keep
                    // the enclosing statement until a child statement runs.
                    if node.kind() != K::Block {
                        self.select_statement(&node);
                    }
                    match node.kind() {
                        K::Block => work.push(Work::Sequence(node.first_child())),
                        K::If => {
                            let condition = self.eval(&expression(&node))?;
                            let selected = if truth(&condition) { 0 } else { 1 };
                            if let Some(body) = node
                                .children()
                                .filter(|n| !is_expression(n.kind()))
                                .nth(selected)
                            {
                                work.push(Work::Statement(body));
                            }
                        }
                        K::Sections => {
                            self.begin_sections(&node);
                            work.push(Work::SectionsNext);
                        }
                        K::Repeat => {
                            self.begin_repeat(&node)?;
                            work.push(Work::RepeatNext);
                        }
                        _ => self.execute_statement(&node)?,
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn select_statement(&mut self, node: &SyntaxNode) {
        let offset = usize::from(node.text_range().start());
        let line = self.source_lines.partition_point(|&start| start <= offset);
        self.statement = Some((line, node.text_range()));
    }
    fn execute_statement(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        match node.kind() {
            K::Format => Ok(()),
            K::Setting => {
                self.execute_setting(node);
                Ok(())
            }
            K::Declaration => self.execute_declaration(node),
            K::TypeDefinition => {
                let descriptor = self.evaluate_domain(&child(node, K::Type))?;
                let id = self.spec.index.definitions[&node.text_range()];
                self.types.insert(id, descriptor);
                Ok(())
            }
            K::Let => {
                let name = tokens(node)
                    .into_iter()
                    .filter(|t| t.kind() == K::Ident)
                    .nth(1)
                    .expect("valid let has a name")
                    .text()
                    .to_owned();
                let value = self.eval(&expression(node))?;
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .set(name, value);
                Ok(())
            }
            K::List => self.execute_list(node),
            K::Each => Ok(()),
            K::Index => self.execute_index(node),
            K::Line | K::Find => self.execute_pattern(node),
            K::Skip => self.execute_skip(node),
            K::Expect => self.execute_expect(node),
            K::Check => self.execute_check(node),
            K::Param | K::Instances => Err(unsupported(
                node.text_range(),
                "parameters and instances",
                "rust-008",
            )),
            _ => unreachable!("valid root/block contains statements"),
        }
    }
    fn execute_setting(&mut self, node: &SyntaxNode) {
        self.input.settings_changed();
        let own_tokens = tokens(node);
        let strings: Vec<_> = own_tokens
            .iter()
            .filter(|t| t.kind() == K::String)
            .map(|t| DecodedString::new(t.text()).text.into_bytes())
            .collect();
        let settings = &mut self.input.settings;
        match own_tokens[0].text() {
            "comment" => settings.comments.extend(strings),
            "stop" => settings.stops.extend(strings),
            "tokens" => settings.add_tokens(&strings[0]),
            "separators" => settings.add_separators(&strings[0]),
            "ignore" => settings.ignore_trailing = true,
            _ => unreachable!("valid setting"),
        }
    }
    fn execute_declaration(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let name = tokens(node)[0].text().to_owned();
        self.context = Some(name.clone());
        let offset = self.evaluate_offset(node)?;
        let domain = self.evaluate_domain(&child(node, K::Type))?;
        let dimensions = self.evaluate_dimensions(node)?;
        let value = match &domain {
            EvaluatedType::Alphabet(alphabet) => self.read_alphabet(alphabet, &dimensions)?,
            _ => {
                if let Some(layout) = node.children().find(|n| n.kind() == K::Layout) {
                    self.read_layout(&domain, &dimensions, &layout, offset)?
                } else {
                    self.read_array(&domain, &dimensions, offset)?
                }
            }
        };
        self.publish(name, value);
        Ok(())
    }
    fn publish(&mut self, name: String, value: Value) {
        if name != "_" {
            self.frames
                .last_mut()
                .expect("active frame")
                .set(name, value);
        }
        self.context = None;
    }
    fn evaluate_domain(&mut self, node: &SyntaxNode) -> Result<EvaluatedType, Failure> {
        let contents = node.children().next().expect("valid type has contents");
        Ok(match contents.kind() {
            K::BaseType => match tokens(&contents)[0].text() {
                "int" => EvaluatedType::Int,
                "float" => EvaluatedType::Float,
                "num" => EvaluatedType::Num,
                "word" => EvaluatedType::Word,
                "rest" => EvaluatedType::Rest,
                _ => unreachable!("base type"),
            },
            K::TypeName => {
                let name = tokens(&contents)
                    .into_iter()
                    .find(|t| t.kind() == K::Ident)
                    .expect("named type has identifier");
                let reference = self.spec.index.references[&name.text_range()];
                let Resolution::NamedType(id) =
                    self.spec.analysis.model.reference(reference).resolution
                else {
                    unreachable!("valid type use resolves to a named type");
                };
                self.types.get(&id).cloned().ok_or_else(|| {
                    self.error(
                        ErrorCode::TypeUndefined,
                        format!("the type '{}' is not defined yet", name.text()),
                    )
                })?
            }
            K::Range => self.evaluate_range(&contents)?,
            K::Alphabet => EvaluatedType::Alphabet(Self::alphabet_options(&contents)),
            _ => unreachable!("valid reading type"),
        })
    }
    fn evaluate_range(&mut self, contents: &SyntaxNode) -> Result<EvaluatedType, Failure> {
        let mut lower = None;
        let mut upper = None;
        let operator = tokens(contents)
            .into_iter()
            .find(|t| matches!(t.text(), ".." | "..<"))
            .expect("range operator");
        for bound in contents.children().filter(|n| is_expression(n.kind())) {
            let value = self.eval(&bound)?;
            let integer = value.root().as_int().map_err(|_| {
                self.error(
                    ErrorCode::RangeBounds,
                    "the bounds of a range must be integers",
                )
            })?;
            if bound.text_range().start() < operator.text_range().start() {
                lower = Some(integer);
            } else {
                upper = Some(integer);
            }
        }
        if operator.text() == "..<" {
            upper = Some(
                upper
                    .expect("exclusive upper bound exists")
                    .checked_sub(1)
                    .ok_or_else(|| {
                        self.error(
                            ErrorCode::IntegerOverflow,
                            "integer overflow in the upper bound of a range",
                        )
                    })?,
            );
        }
        Ok(EvaluatedType::Range { lower, upper })
    }
    fn convert_scalar(&self, domain: &EvaluatedType, token: &Token<'_>) -> Result<Value, Error> {
        let value = match domain {
            EvaluatedType::Int | EvaluatedType::Range { .. } => {
                numeric::parse_integer(token.bytes).map(Value::int)
            }
            EvaluatedType::Float => numeric::parse_float(token.bytes).map(Value::float),
            EvaluatedType::Num => numeric::parse_integer(token.bytes)
                .map(Value::int)
                .or_else(|| numeric::parse_float(token.bytes).map(Value::float)),
            EvaluatedType::Word | EvaluatedType::Rest => {
                let text = std::str::from_utf8(token.bytes).map_err(|e| {
                    self.error_at(
                        ErrorCode::Utf8,
                        "string value is not valid UTF-8",
                        Position {
                            offset: token.start.offset + e.valid_up_to(),
                            column: token.start.column + e.valid_up_to(),
                            ..token.start
                        },
                    )
                })?;
                Some(Value::string(text.to_owned()))
            }
            EvaluatedType::Alphabet(_) => unreachable!("alphabet uses byte mapping"),
        };
        value.ok_or_else(|| self.expected_value(domain, token))
    }
    fn validate_range(
        &self,
        domain: &EvaluatedType,
        value: &Value,
        token: &Token<'_>,
    ) -> Result<(), Error> {
        if let EvaluatedType::Range { lower, upper } = domain {
            let integer = value
                .root()
                .as_int()
                .expect("range conversion produced an int");
            if lower.is_some_and(|lo| integer < lo) || upper.is_some_and(|hi| integer > hi) {
                return Err(self.expected_value(domain, token));
            }
        }
        Ok(())
    }
    fn read_scalar(&mut self, domain: &EvaluatedType) -> Result<Value, Failure> {
        let token = if matches!(domain, EvaluatedType::Rest) {
            self.input.read_rest()
        } else {
            self.input.next_token()
        };
        let token = token.ok_or_else(|| {
            self.error(
                ErrorCode::ExpectedValue,
                "expected a value, found end of input",
            )
        })?;
        let value = self.convert_scalar(domain, &token)?;
        self.validate_range(domain, &value, &token)?;
        Ok(value)
    }
    fn expected_value(&self, domain: &EvaluatedType, token: &Token<'_>) -> Error {
        let description = match domain {
            EvaluatedType::Int => "int".into(),
            EvaluatedType::Float => "float".into(),
            EvaluatedType::Num => "a number".into(),
            EvaluatedType::Range { lower, upper } => format!(
                "an int in {}..{}",
                lower.map_or(String::new(), |n| n.to_string()),
                upper.map_or(String::new(), |n| n.to_string())
            ),
            _ => "a value".into(),
        };
        let mut error = self.error_at(
            ErrorCode::ExpectedValue,
            format!(
                "expected {description}, found '{}'",
                String::from_utf8_lossy(token.bytes)
            ),
            token.start,
        );
        error.set_input_end(token.end.offset);
        error
    }
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| frame.lookup(name))
    }

    pub fn error(&self, code: ErrorCode, message: impl Into<String>) -> Error {
        self.error_at(code, message, self.input.position())
    }
    pub(crate) fn error_at(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        position: Position,
    ) -> Error {
        let context = self.execution_context();
        Error::input(
            code,
            message,
            &self.input,
            position,
            self.statement
                .map(|(line, range)| (self.spec.parsed.origin(), line, range)),
            context.as_deref(),
        )
    }
    pub(crate) fn execution_context(&self) -> Option<String> {
        let mut contexts: Vec<_> = self
            .contexts
            .iter()
            .filter_map(|context| match context {
                ExecutionContext::Repeat(index) => self.repeats[*index].context(),
                ExecutionContext::Section(range) => Some(format!(
                    "section \"{}\"",
                    self.spec.index.patterns[range].source
                )),
            })
            .collect();
        contexts.extend(self.context.iter().cloned());
        (!contexts.is_empty()).then(|| contexts.join(" > "))
    }
    fn charge_idle(&mut self, _units: u128) -> Result<(), Error> {
        #[cfg(test)]
        self.observe_charge(false, _units);
        Ok(())
    }
    #[cfg(test)]
    fn observe_charge(&mut self, expression: bool, units: u128) {
        if !self.capture_charges {
            return;
        }
        self.charges.push(Charge {
            expression,
            units,
            statement_line: self.statement.map(|s| s.0),
            context: self.execution_context(),
        });
    }
}
pub(crate) fn unsupported(range: TextRange, feature: &'static str, task: &'static str) -> Failure {
    Failure::Unsupported(UnsupportedFeature {
        feature,
        task,
        range,
    })
}
pub(crate) fn tokens(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !matches!(t.kind(), K::Whitespace | K::Comment))
        .collect()
}
pub(crate) fn child(node: &SyntaxNode, kind: K) -> SyntaxNode {
    node.children()
        .find(|n| n.kind() == kind)
        .expect("valid syntax has required child")
}
pub(crate) fn expression(node: &SyntaxNode) -> SyntaxNode {
    node.children()
        .find(|n| is_expression(n.kind()))
        .expect("valid syntax has expression")
}
fn is_expression(kind: K) -> bool {
    matches!(
        kind,
        K::Literal
            | K::Name
            | K::HostConstant
            | K::Parenthesized
            | K::Unary
            | K::Binary
            | K::Power
            | K::Call
            | K::Comprehension
            | K::IndexExpression
            | K::Field
    )
}
pub(crate) fn truth(value: &Value) -> bool {
    let scalar = value.root();
    if value.depth() > 0 {
        return !scalar.children().expect("array").is_empty();
    }
    match scalar.value_type() {
        "int" => scalar.as_int().expect("scalar int") != 0,
        "float" => scalar.as_float().expect("scalar float") != 0.0,
        _ => !scalar.as_str().expect("scalar string").is_empty(),
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Charge {
    pub(crate) expression: bool,
    pub(crate) units: u128,
    pub(crate) statement_line: Option<usize>,
    pub(crate) context: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) enum WorkKind {
    Dispatch,
    Metadata,
    Probe,
    Copy,
    Link,
    Leaf,
}
impl Interpreter<'_, '_> {
    #[inline]
    pub(crate) fn work(&self, _kind: WorkKind, _count: usize) {
        #[cfg(test)]
        {
            let mut counts = self.work.get();
            counts[_kind as usize] += _count;
            self.work.set(counts);
        }
    }
}
