use crate::{
    Error, ErrorCode, Failure, Spec, UnsupportedFeature, Value,
    input::{Input, Position, Token},
    numeric,
    value::{Frame, LeafStorage},
};
use iris_sema::{DefinitionId, DefinitionKind, ReferenceId, Resolution, SemanticModel};
use iris_syntax::{DecodedString, SyntaxKind as K, SyntaxNode, SyntaxToken, TextRange};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct ModelIndex {
    definitions: HashMap<TextRange, DefinitionId>,
    pub references: HashMap<TextRange, ReferenceId>,
}
impl ModelIndex {
    pub fn new(model: &SemanticModel) -> Self {
        Self {
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
mod reads;

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
    pub bindings: Vec<(String, Value)>,
    statement: Option<(usize, TextRange)>,
    context: Option<String>,
}
impl<'s, 'i> Interpreter<'s, 'i> {
    pub fn new(spec: &'s Spec, input: Input<'i>) -> Self {
        Self {
            spec,
            input,
            frames: vec![Frame::default()],
            types: HashMap::new(),
            bindings: vec![],
            statement: None,
            context: None,
        }
    }
    pub fn read(mut self) -> Result<crate::Data, Failure> {
        self.execute_block(&self.spec.parsed.syntax())?;
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
    fn execute_block(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        for statement in node.children() {
            self.execute_statement(&statement)?;
        }
        Ok(())
    }
    fn execute_statement(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let offset = usize::from(node.text_range().start());
        let line = self.spec.source[..offset]
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
            + 1;
        self.statement = Some((line, node.text_range()));
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
            K::If => self.execute_if(node),
            K::Block => self.execute_block(node),
            K::List => self.execute_list(node),
            K::Repeat | K::Each | K::Index => Err(unsupported(
                node.text_range(),
                "repeat and index execution",
                "rust-005",
            )),
            K::Line | K::Find | K::Skip | K::Expect | K::Sections => Err(unsupported(
                node.text_range(),
                "line and section execution",
                "rust-006",
            )),
            K::Check => Err(unsupported(
                node.text_range(),
                "check execution",
                "rust-007",
            )),
            K::Param | K::Instances => Err(unsupported(
                node.text_range(),
                "parameters and instances",
                "rust-008",
            )),
            _ => unreachable!("valid root/block contains statements"),
        }
    }
    fn execute_if(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let condition = self.eval(&expression(node))?;
        let selected = if truth(&condition) { 0 } else { 1 };
        if let Some(body) = node
            .children()
            .filter(|n| !is_expression(n.kind()))
            .nth(selected)
        {
            self.execute_statement(&body)?;
        }
        Ok(())
    }
    fn execute_setting(&mut self, node: &SyntaxNode) {
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
                        self.input.position_at(token.start.offset + e.valid_up_to()),
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
        self.bindings
            .iter()
            .rev()
            .find(|(bound, _)| bound == name)
            .map(|(_, value)| value)
            .or_else(|| {
                self.frames
                    .iter()
                    .rev()
                    .find_map(|frame| frame.lookup(name))
            })
    }
    pub fn error(&self, code: ErrorCode, message: impl Into<String>) -> Error {
        self.error_at(code, message, self.input.position())
    }
    fn error_at(&self, code: ErrorCode, message: impl Into<String>, position: Position) -> Error {
        Error::input(
            code,
            message,
            &self.input,
            position,
            self.statement
                .map(|(line, range)| (self.spec.parsed.origin(), line, range)),
            self.context.as_deref(),
        )
    }
    fn charge_idle(&mut self, _units: u128) -> Result<(), Error> {
        Ok(())
    }
    pub fn charge_expression(&mut self, _units: u128) -> Result<(), Error> {
        Ok(())
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
