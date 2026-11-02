use crate::{
    Error, ErrorCode, Failure, Spec, Value,
    input::{Input, Position, Token},
    numeric,
    value::{Frame, LeafStorage},
};
use iris_sema::{DefinitionId, DefinitionKind, ReferenceId, RepeatId, Resolution, SemanticModel};
use iris_syntax::{DecodedString, SyntaxKind as K, SyntaxNode, SyntaxToken, TextRange};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

const BASE_BUDGET: u128 = 1 << 20;

#[derive(Debug)]
pub(crate) struct ModelIndex {
    pub params: HashSet<String>,
    definitions: HashMap<TextRange, DefinitionId>,
    pub references: HashMap<TextRange, ReferenceId>,
    pub repeats: HashMap<TextRange, RepeatId>,
    pub checks: HashMap<TextRange, usize>,
    pub expressions: HashMap<TextRange, usize>,
    pub comprehensions: HashMap<TextRange, usize>,
    pub structural: HashMap<String, iris_sema::Denotation>,
    patterns: HashMap<TextRange, pattern::Pattern>,
    pub pattern_literals: HashMap<TextRange, String>,
    /// Every ordinary string token is decoded at load time. Read-time users
    /// borrow it or copy it through the cancellation control.
    pub string_literals: HashMap<TextRange, String>,
    sections: HashMap<TextRange, sections::Sections>,
    alphabets: HashMap<TextRange, Arc<Alphabet>>,
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
        let patterns = root
            .descendants()
            .filter(|n| n.kind() == K::Pattern)
            .map(|n| (n.text_range(), pattern::Pattern::new(&n, &kinds)))
            .collect();
        let pattern_literals: HashMap<_, _> = root
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
        let string_literals = root
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|t| t.kind() == K::String && !pattern_literals.contains_key(&t.text_range()))
            .map(|t| (t.text_range(), DecodedString::new(t.text()).text))
            .collect();
        let sections = root
            .descendants()
            .filter(|n| n.kind() == K::Sections)
            .map(|n| (n.text_range(), sections::Sections::new(n, &string_literals)))
            .collect();
        let alphabets = root
            .descendants()
            .filter(|n| n.kind() == K::Alphabet)
            .map(|n| {
                (
                    n.text_range(),
                    Arc::new(Alphabet::new(&n, &string_literals)),
                )
            })
            .collect();
        let mut params: HashSet<_> = model
            .definitions()
            .iter()
            .filter(|d| d.kind == DefinitionKind::Param)
            .map(|d| d.name.clone())
            .collect();
        if root.children().any(|n| n.kind() == K::Instances) {
            params.insert("instance".into());
        }
        Self {
            params,
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
            string_literals,
            alphabets,
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
#[cfg(test)]
mod budget_tests;
pub(crate) mod checks;
mod instances;
mod params;
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
    Alphabet(Arc<Alphabet>),
}
#[derive(Debug)]
struct Alphabet {
    symbols: Vec<u8>,
    blank: Vec<u8>,
    strip: Vec<u8>,
    base: i64,
}
impl Alphabet {
    fn new(node: &SyntaxNode, strings: &HashMap<TextRange, String>) -> Alphabet {
        let symbols = tokens(node)
            .into_iter()
            .find(|t| t.kind() == K::String)
            .expect("alphabet symbols");
        let mut alphabet = Alphabet {
            symbols: strings[&symbols.text_range()].as_bytes().to_vec(),
            blank: vec![],
            strip: vec![],
            base: 1,
        };
        for option in node.children().filter(|n| n.kind() == K::AlphabetOption) {
            let parts = tokens(&option);
            match parts[0].text() {
                "blank" => alphabet.blank = strings[&parts[1].text_range()].as_bytes().to_vec(),
                "strip" => alphabet.strip = strings[&parts[1].text_range()].as_bytes().to_vec(),
                "base" => {
                    let spelling: String = parts[1..].iter().map(|t| t.text()).collect();
                    alphabet.base = numeric::parse_integer(spelling.as_bytes())
                        .expect("validated base literal");
                }
                _ => unreachable!("alphabet option"),
            }
        }
        alphabet
    }
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
    params: crate::Params,
    instance: Option<instances::Instance>,
    catalog: Option<instances::Catalog>,
    selected: bool,
    idle_remaining: u128,
    // Evaluators borrow the reader's values while charging this operation.
    expression_remaining: std::cell::Cell<u128>,
    #[cfg(test)]
    pub(crate) charges: Vec<Charge>,
    #[cfg(test)]
    pub(crate) capture_charges: bool,
    #[cfg(test)]
    pub(crate) work: std::cell::Cell<[usize; 6]>,
}
impl<'s, 'i> Interpreter<'s, 'i> {
    pub fn new(spec: &'s Spec, input: Input<'i>) -> Result<Self, Failure> {
        // usize input lengths fit in u64 on supported Rust targets.
        let expression_limit = BASE_BUDGET + 16 * input.bytes.len() as u128;
        let mut source_lines = vec![0];
        for (chunk_index, chunk) in spec.source.chunks(input.control.chunk_size()).enumerate() {
            input.control.check()?;
            let start = if input.control.is_active() {
                chunk_index * crate::cancellation::SCAN_CHUNK
            } else {
                0
            };
            source_lines.extend(
                chunk
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| **b == b'\n')
                    .map(|(i, _)| start + i + 1),
            );
        }
        Ok(Self {
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
            params: crate::Params::new(),
            instance: None,
            catalog: None,
            selected: false,
            idle_remaining: BASE_BUDGET,
            expression_remaining: std::cell::Cell::new(expression_limit),
            source_lines,
        })
    }
    pub fn with_params(mut self, params: &crate::Params) -> Result<Self, Failure> {
        if !self.input.control.is_active() {
            self.params = params.clone();
        } else {
            for (name, value) in params.pairs() {
                self.input.control.check()?;
                self.params.set(
                    self.input.control.copy_text(name)?,
                    self.input.control.copy_text(value)?,
                );
            }
        }
        Ok(self)
    }
    pub fn read(mut self) -> Result<crate::Data, Failure> {
        self.execute(self.spec.parsed.syntax())?;
        self.finish_input()?;
        self.input.control.check()?;
        Ok(self
            .frames
            .pop()
            .expect("root frame exists throughout a read")
            .into_data())
    }
    pub(crate) fn execute(&mut self, root: SyntaxNode) -> Result<(), Failure> {
        let mut work = vec![Work::Sequence(root.first_child())];
        while let Some(next) = work.pop() {
            self.input.control.check()?;
            self.work(WorkKind::Dispatch, 1);
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
            K::Setting => self.execute_setting(node),
            K::Declaration => self.execute_declaration(node),
            K::TypeDefinition => {
                let descriptor = self.evaluate_domain(&child(node, K::Type))?;
                let id = self.spec.index.definitions[&node.text_range()];
                let previous = self.types.insert(id, descriptor);
                if let Some(instance) = &mut self.instance {
                    instance.types.push((id, previous));
                }
                Ok(())
            }
            K::Let => {
                let token = tokens(node)
                    .into_iter()
                    .filter(|t| t.kind() == K::Ident)
                    .nth(1)
                    .expect("valid let has a name");
                let name = self.input.control.copy_text(token.text())?;
                let value = self.eval(&expression(node))?;
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .set_controlled(name, value, &self.input.control)?;
                Ok(())
            }
            K::List => self.execute_list(node),
            K::Each => Ok(()),
            K::Index => self.execute_index(node),
            K::Line | K::Find => self.execute_pattern(node),
            K::Skip => self.execute_skip(node),
            K::Expect => self.execute_expect(node),
            K::Check => self.execute_check(node),
            K::Param => self.execute_param(node),
            K::Instances => self.execute_instances(node),
            _ => unreachable!("valid root/block contains statements"),
        }
    }
    fn execute_setting(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        self.input.settings_changed();
        let own_tokens = tokens(node);
        let strings: Vec<_> = own_tokens
            .iter()
            .filter(|t| t.kind() == K::String)
            .map(|t| {
                self.input
                    .control
                    .copy_text(&self.spec.index.string_literals[&t.text_range()])
                    .map(String::into_bytes)
            })
            .collect::<Result<_, _>>()?;
        let settings = &mut self.input.settings;
        match own_tokens[0].text() {
            "comment" => settings.comments.extend(strings),
            "stop" => settings.stops.extend(strings),
            "tokens" => settings.add_tokens(&strings[0], &self.input.control)?,
            "separators" => settings.add_separators(&strings[0], &self.input.control)?,
            "ignore" => settings.ignore_trailing = true,
            _ => unreachable!("valid setting"),
        }
        Ok(())
    }
    fn execute_declaration(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let name = self.input.control.copy_text(tokens(node)[0].text())?;
        self.context = Some(self.input.control.copy_text(&name)?);
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
        self.publish(name, value)?;
        Ok(())
    }
    fn publish(&mut self, name: String, value: Value) -> Result<(), Failure> {
        if name != "_" {
            self.frames
                .last_mut()
                .expect("active frame")
                .set_controlled(name, value, &self.input.control)?;
        }
        self.context = None;
        Ok(())
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
                    self.error_text(
                        ErrorCode::TypeUndefined,
                        &["the type '", name.text(), "' is not defined yet"],
                    )
                })?
            }
            K::Range => self.evaluate_range(&contents)?,
            K::Alphabet => {
                EvaluatedType::Alphabet(self.spec.index.alphabets[&contents.text_range()].clone())
            }
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
    fn convert_scalar(&self, domain: &EvaluatedType, token: &Token<'_>) -> Result<Value, Failure> {
        let value = match domain {
            EvaluatedType::Int | EvaluatedType::Range { .. } => {
                numeric::parse_integer_controlled(token.bytes, &self.input.control)?.map(Value::int)
            }
            EvaluatedType::Float => {
                numeric::parse_float_controlled(token.bytes, &self.input.control)?.map(Value::float)
            }
            EvaluatedType::Num => {
                match numeric::parse_integer_controlled(token.bytes, &self.input.control)? {
                    Some(n) => Some(Value::int(n)),
                    None => numeric::parse_float_controlled(token.bytes, &self.input.control)?
                        .map(Value::float),
                }
            }
            EvaluatedType::Word | EvaluatedType::Rest => {
                let text = self
                    .input
                    .control
                    .copy_utf8(token.bytes)?
                    .map_err(|offset| {
                        self.error_at(
                            ErrorCode::Utf8,
                            "string value is not valid UTF-8",
                            Position {
                                offset: token.start.offset + offset,
                                column: token.start.column + offset,
                                ..token.start
                            },
                        )
                    })?;
                Some(Value::string(text))
            }
            EvaluatedType::Alphabet(_) => unreachable!("alphabet uses byte mapping"),
        };
        match value {
            Some(value) => Ok(value),
            None => Err(self.expected_value(domain, token)?.into()),
        }
    }
    fn validate_range(
        &self,
        domain: &EvaluatedType,
        value: &Value,
        token: &Token<'_>,
    ) -> Result<(), Failure> {
        if let EvaluatedType::Range { lower, upper } = domain {
            let integer = value
                .root()
                .as_int()
                .expect("range conversion produced an int");
            if lower.is_some_and(|lo| integer < lo) || upper.is_some_and(|hi| integer > hi) {
                return Err(self.expected_value(domain, token)?.into());
            }
        }
        Ok(())
    }
    fn read_scalar(&mut self, domain: &EvaluatedType) -> Result<Value, Failure> {
        let token = if matches!(domain, EvaluatedType::Rest) {
            self.input.read_rest()?
        } else {
            self.input.next_token()?
        };
        let token = token.ok_or_else(|| {
            self.error(
                ErrorCode::ExpectedValue,
                "expected a value, found end of input",
            )
        })?;
        self.work(WorkKind::Leaf, token.bytes.len());
        let value = self.convert_scalar(domain, &token)?;
        self.validate_range(domain, &value, &token)?;
        Ok(value)
    }
    #[cfg(test)]
    pub(crate) fn expected_int(
        &self,
        token: &Token<'_>,
    ) -> Result<Error, crate::cancellation::Cancelled> {
        self.expected_value(&EvaluatedType::Int, token)
    }
    fn expected_value(
        &self,
        domain: &EvaluatedType,
        token: &Token<'_>,
    ) -> Result<Error, crate::cancellation::Cancelled> {
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
        let mut message = format!("expected {description}, found '");
        self.input.control.push_text(&mut message, token.bytes)?;
        message.push('\'');
        let mut error = self.language_error_at(ErrorCode::ExpectedValue, message, token.start)?;
        error.set_input_end(token.end.offset);
        Ok(error)
    }
    pub(crate) fn matches_token(
        &self,
        alternatives: &[String],
        bytes: &[u8],
    ) -> Result<bool, Failure> {
        for alternative in alternatives {
            self.input.control.check()?;
            if self
                .input
                .control
                .equal_bytes(alternative.as_bytes(), bytes)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn lookup(&self, name: &str) -> Option<&Value> {
        self.frames
            .iter()
            .rev()
            .find_map(|frame| frame.lookup(name))
    }

    pub fn error(&self, code: ErrorCode, message: impl Into<String>) -> Failure {
        self.error_at(code, message, self.input.position())
    }
    pub(crate) fn error_text(&self, code: ErrorCode, parts: &[&str]) -> Failure {
        match self.input.control.text_message(parts) {
            Ok(message) => self.error(code, message),
            Err(cancelled) => cancelled.into(),
        }
    }
    pub(crate) fn error_at(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        position: Position,
    ) -> Failure {
        match self.language_error_at(code, message, position) {
            Ok(error) => error.into(),
            Err(cancelled) => cancelled.into(),
        }
    }
    fn language_error_at(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        position: Position,
    ) -> Result<Error, crate::cancellation::Cancelled> {
        let context = self.execution_context()?;
        // Move the checked context into the error rather than copying it again.
        let mut error = Error::input(
            code,
            message,
            &self.input,
            position,
            self.statement
                .map(|(line, range)| (self.spec.parsed.origin(), line, range)),
            None,
        );
        error.details.context = context;
        Ok(error)
    }
    pub(crate) fn execution_context(
        &self,
    ) -> Result<Option<String>, crate::cancellation::Cancelled> {
        let control = &self.input.control;
        let mut context = String::new();
        if let Some(instance) = &self.instance {
            context = format!("instance [{}]", instance.ordinal);
        }
        for item in &self.contexts {
            control.check()?;
            let part = match item {
                ExecutionContext::Repeat(index) => self.repeats[*index].context(),
                ExecutionContext::Section(range) => Some(control.text_message(&[
                    "section \"",
                    &self.spec.index.patterns[range].source,
                    "\"",
                ])?),
            };
            if let Some(part) = part {
                if !context.is_empty() {
                    context.push_str(" > ");
                }
                control.push_text(&mut context, part.as_bytes())?;
            }
        }
        if let Some(name) = &self.context {
            if !context.is_empty() {
                context.push_str(" > ");
            }
            control.push_text(&mut context, name.as_bytes())?;
        }
        Ok((!context.is_empty()).then_some(context))
    }
    fn charge_idle(&mut self, units: u128) -> Result<(), Failure> {
        self.work(WorkKind::Probe, 1);
        #[cfg(test)]
        self.observe_charge(false, units)?;
        self.idle_remaining = self.idle_remaining.checked_sub(units).ok_or_else(|| {
            self.error(
                ErrorCode::Budget,
                "too many repetitions or empty rows without reading input",
            )
        })?;
        Ok(())
    }
    pub(crate) fn charge_expression(&self, units: u128) -> Result<(), Failure> {
        self.work(WorkKind::Probe, 1);
        let remaining = self
            .expression_remaining
            .get()
            .checked_sub(units)
            .ok_or_else(|| {
                self.error(
                    ErrorCode::Budget,
                    "the expressions do too much work for the size of the input",
                )
            })?;
        self.expression_remaining.set(remaining);
        Ok(())
    }
    #[cfg(test)]
    fn observe_charge(
        &mut self,
        expression: bool,
        units: u128,
    ) -> Result<(), crate::cancellation::Cancelled> {
        if !self.capture_charges {
            return Ok(());
        }
        self.charges.push(Charge {
            expression,
            units,
            statement_line: self.statement.map(|s| s.0),
            context: self.execution_context()?,
        });
        Ok(())
    }
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

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use crate::cancellation::{SCAN_CHUNK, tests::cancel_at};

    #[test]
    fn cancellation_prepared_alphabets_are_shared_across_records() {
        for (symbols, base) in [
            ("a".repeat(16 * SCAN_CHUNK), "1".into()),
            ("a".into(), "0".repeat(16 * SCAN_CHUNK)),
        ] {
            let source = format!("repeat 4 {{x[1][1]:alphabet \"{symbols}\" base {base}}}");
            let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
            let node = spec
                .parsed
                .syntax()
                .descendants()
                .find(|n| n.kind() == K::Type)
                .unwrap();
            let mut reader = Interpreter::new(&spec, Input::new(b"a a a a", "input")).unwrap();
            let EvaluatedType::Alphabet(first) = reader.evaluate_domain(&node).unwrap() else {
                panic!("alphabet")
            };
            let EvaluatedType::Alphabet(second) = reader.evaluate_domain(&node).unwrap() else {
                panic!("alphabet")
            };
            let prepared = &spec.index.alphabets[&child(&node, K::Alphabet).text_range()];
            assert!(Arc::ptr_eq(&first, prepared));
            assert!(
                Arc::ptr_eq(&first, &second),
                "record execution must share the prepared alphabet, including its parsed base"
            );
            // With construction outside the read, both long forms stop at a
            // short dispatch checkpoint inside the repeat.
            cancel_at(30, |control| {
                reader.input.control = control;
                reader.read()
            });
        }
    }
    #[test]
    fn cancellation_inside_prepared_repeat_and_setting_literals() {
        let long = "a".repeat(16 * SCAN_CHUNK);
        for source in [
            format!("repeat until \"{long}\" {{x:int}}"),
            format!("comment \"{long}\""),
        ] {
            let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
            let node = spec.parsed.syntax().first_child().unwrap();
            let mut reader = Interpreter::new(&spec, Input::new(b"", "input")).unwrap();
            cancel_at(3, |control| {
                reader.input.control = control;
                if node.kind() == K::Repeat {
                    reader.begin_repeat(&node)
                } else {
                    reader.execute_setting(&node)
                }
            });
        }
    }
    #[test]
    fn cancellation_inside_repeat_terminator_diagnostic() {
        let source = format!("repeat until \"{}\" {{x:int}}", "a".repeat(16 * SCAN_CHUNK));
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let node = spec.parsed.syntax().first_child().unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"", "input")).unwrap();
        // Prepare before observing, so only the EOF diagnostic can reach the gate.
        reader.begin_repeat(&node).unwrap();
        cancel_at(20, |control| {
            reader.input.control = control;
            reader.begin_iteration()
        });
    }
    #[test]
    fn cancellation_expect_uses_the_prepared_literal() {
        let mut spec = Spec::parse(b"expect \"a\"", "spec").unwrap();
        // Give the prepared value distinct content: cancellation in its quote
        // then proves that execution uses the index rather than decoding again.
        *spec.index.string_literals.values_mut().next().unwrap() = "a".repeat(16 * SCAN_CHUNK);
        let node = spec.parsed.syntax().first_child().unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"", "input")).unwrap();
        cancel_at(20, |control| {
            reader.input.control = control;
            reader.execute_expect(&node)
        });
    }
    #[test]
    fn cancellation_inside_section_execution_context() {
        let source = format!("sections {{\"{}\" {{x:int}}}}", "a".repeat(16 * SCAN_CHUNK));
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let range = *spec.index.patterns.keys().next().unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"", "input")).unwrap();
        reader.contexts.push(ExecutionContext::Section(range));
        cancel_at(3, |control| {
            reader.input.control = control;
            Err::<(), _>(reader.error(ErrorCode::ExpectedValue, "expected a value"))
        });
    }
}
