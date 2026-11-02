//! Immutable syntax metadata. Runtime values and bounds are never cached here.
use super::*;

/// Name text is hashed only while the spec loads. The temporary text index is
/// discarded; execution retains identities and shared diagnostic/output text.
pub(super) struct Names {
    pub tokens: HashMap<TextRange, Name>,
    pub definitions: Vec<Name>,
    pub repeats: Vec<Vec<Name>>,
    pub dependencies: Vec<Vec<Name>>,
    pub params: Vec<Name>,
}
impl Names {
    pub fn new(root: &SyntaxNode, model: &SemanticModel) -> Self {
        let mut interned = HashMap::<String, Name>::new();
        for text in ["name", "instances", "failed", "instance", "_"] {
            interned.insert(text.into(), Name::new(interned.len(), text));
        }
        let names: HashMap<_, _> = root
            .descendants_with_tokens()
            .filter_map(|element| element.into_token())
            .filter(|token| token.kind() == K::Ident)
            .map(|token| {
                let next = interned.len();
                let name = interned
                    .entry(token.text().into())
                    .or_insert_with(|| Name::new(next, token.text()))
                    .clone();
                (token.text_range(), name)
            })
            .collect();
        let definition_names: Vec<_> = model
            .definitions()
            .iter()
            .map(|definition| interned[&definition.name].clone())
            .collect();
        let repeat_names = model
            .repeats()
            .iter()
            .map(|repeat| {
                repeat
                    .fields
                    .iter()
                    .map(|field| interned[&field.name].clone())
                    .collect()
            })
            .collect();
        let dependency_names = model
            .checks()
            .iter()
            .map(|check| {
                check
                    .dependencies
                    .iter()
                    .map(|dependency| interned[&dependency.label].clone())
                    .collect()
            })
            .collect();
        let mut params: Vec<_> = model
            .definitions()
            .iter()
            .filter(|d| d.kind == DefinitionKind::Param)
            .map(|d| interned[&d.name].clone())
            .collect();
        if root.children().any(|n| n.kind() == K::Instances) {
            params.push(interned["instance"].clone());
        }
        params.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        params.dedup_by_key(|name| name.id);
        Self {
            tokens: names,
            definitions: definition_names,
            repeats: repeat_names,
            dependencies: dependency_names,
            params,
        }
    }
}

#[derive(Debug)]
pub(super) struct List {
    pub name: Name,
    pub domain: Domain,
    pub sentinel: Option<Bound>,
    pub offset: Option<Bound>,
}
#[derive(Debug)]
pub(super) enum Domain {
    Base(EvaluatedType),
    Named {
        definition: DefinitionId,
        name: Name,
    },
    Range {
        lower: Option<Bound>,
        upper: Option<Bound>,
        exclusive: bool,
    },
    Alphabet(TextRange),
}
#[derive(Debug)]
pub(super) struct Integer {
    base: IntegerBase,
    /// Outer-to-inner syntax steps, including parentheses for dispatch accounting.
    steps: Vec<IntegerStep>,
}
#[derive(Debug)]
enum IntegerStep {
    Parenthesized,
    Negate,
}
#[derive(Debug)]
enum IntegerBase {
    Literal(i64),
    Name(Name),
}

#[derive(Debug)]
pub(super) struct Bound {
    pub node: SyntaxNode,
    integer: Option<Integer>,
}
impl Bound {
    fn new(
        node: SyntaxNode,
        names: &HashMap<TextRange, Name>,
        references: &HashMap<TextRange, ReferenceId>,
        model: &SemanticModel,
    ) -> Self {
        let integer = Integer::new(node.clone(), names, references, model);
        Self { node, integer }
    }
}
impl List {
    pub fn new(
        node: &SyntaxNode,
        names: &HashMap<TextRange, Name>,
        references: &HashMap<TextRange, ReferenceId>,
        model: &SemanticModel,
    ) -> Self {
        Self {
            name: names[&tokens(node)[1].text_range()].clone(),
            domain: Domain::new(&child(node, K::Type), names, references, model),
            sentinel: node
                .children()
                .find(|n| is_expression(n.kind()))
                .map(|n| Bound::new(n, names, references, model)),
            offset: node
                .children()
                .find(|n| n.kind() == K::Offset)
                .map(|n| Bound::new(expression(&n), names, references, model)),
        }
    }
}
impl Domain {
    pub fn new(
        node: &SyntaxNode,
        names: &HashMap<TextRange, Name>,
        references: &HashMap<TextRange, ReferenceId>,
        model: &SemanticModel,
    ) -> Self {
        let contents = node.first_child().expect("type contents");
        match contents.kind() {
            K::BaseType => Self::Base(match tokens(&contents)[0].text() {
                "int" => EvaluatedType::Int,
                "float" => EvaluatedType::Float,
                "num" => EvaluatedType::Num,
                "word" => EvaluatedType::Word,
                "rest" => EvaluatedType::Rest,
                _ => unreachable!("base type"),
            }),
            K::TypeName => {
                let token = &tokens(&contents)[0];
                let Resolution::NamedType(definition) =
                    model.reference(references[&token.text_range()]).resolution
                else {
                    unreachable!("named type")
                };
                Self::Named {
                    definition,
                    name: names[&token.text_range()].clone(),
                }
            }
            K::Range => {
                let operator = tokens(&contents)
                    .into_iter()
                    .find(|t| matches!(t.text(), ".." | "..<"))
                    .expect("range operator");
                let mut lower = None;
                let mut upper = None;
                for bound in contents.children().filter(|n| is_expression(n.kind())) {
                    if bound.text_range().start() < operator.text_range().start() {
                        lower = Some(Bound::new(bound, names, references, model));
                    } else {
                        upper = Some(Bound::new(bound, names, references, model));
                    }
                }
                Self::Range {
                    lower,
                    upper,
                    exclusive: operator.text() == "..<",
                }
            }
            K::Alphabet => Self::Alphabet(contents.text_range()),
            _ => unreachable!("reading type"),
        }
    }
}
impl Integer {
    pub fn new(
        mut node: SyntaxNode,
        names: &HashMap<TextRange, Name>,
        references: &HashMap<TextRange, ReferenceId>,
        model: &SemanticModel,
    ) -> Option<Self> {
        let mut steps = Vec::new();
        let base = loop {
            match node.kind() {
                K::Parenthesized => {
                    steps.push(IntegerStep::Parenthesized);
                    node = expression(&node);
                }
                K::Unary if tokens(&node)[0].text() == "-" => {
                    steps.push(IntegerStep::Negate);
                    node = expression(&node);
                }
                K::Literal => {
                    let token = &tokens(&node)[0];
                    if token.kind() != K::Integer {
                        return None;
                    }
                    break IntegerBase::Literal(numeric::parse_integer(token.text().as_bytes())?);
                }
                K::HostConstant => {
                    let token = &tokens(&node)[0];
                    let Resolution::HostConstant(value) =
                        model.reference(references[&token.text_range()]).resolution
                    else {
                        return None;
                    };
                    break IntegerBase::Literal(value);
                }
                K::Name => {
                    let token = &tokens(&node)[0];
                    if !matches!(&model.reference(references[&token.text_range()]).resolution, Resolution::Value { projection, .. } if projection.is_empty())
                    {
                        return None;
                    }
                    break IntegerBase::Name(names[&token.text_range()].clone());
                }
                _ => return None,
            }
        };
        Some(Self { base, steps })
    }
}
impl Interpreter<'_, '_> {
    pub(super) fn eval_integer(&self, bound: &Bound) -> Result<Option<i64>, Failure> {
        let Some(integer) = &bound.integer else {
            return Ok(None);
        };
        for _ in &integer.steps {
            self.input.control.check()?;
            self.work(WorkKind::Dispatch, 1);
        }
        self.input.control.check()?;
        self.work(WorkKind::Dispatch, 1);
        let mut value = match &integer.base {
            IntegerBase::Literal(value) => *value,
            IntegerBase::Name(name) => {
                let mut found = None;
                for frame in self.frames.iter().rev() {
                    self.input.control.check()?;
                    self.work(WorkKind::Probe, 1);
                    if let Some(value) = frame.lookup(name.id) {
                        found = Some(value);
                        break;
                    }
                }
                let value = found.ok_or_else(|| {
                    self.error_text(
                        ErrorCode::NameUndefined,
                        &["'", name, "' is not defined yet"],
                    )
                })?;
                let Ok(value) = value.root().as_int() else {
                    return Ok(None);
                };
                value
            }
        };
        for step in integer.steps.iter().rev() {
            self.input.control.check()?;
            self.work(WorkKind::Dispatch, 1);
            if matches!(step, IntegerStep::Negate) {
                value = value.checked_neg().ok_or_else(|| {
                    self.error(ErrorCode::IntegerOverflow, "integer overflow in '-'")
                })?;
            }
        }
        Ok(Some(value))
    }
    pub(super) fn eval_range_bound(&mut self, bound: &Bound) -> Result<i64, Failure> {
        if let Some(integer) = self.eval_integer(bound)? {
            return Ok(integer);
        }
        self.eval(&bound.node)?.root().as_int().map_err(|_| {
            self.error(
                ErrorCode::RangeBounds,
                "the bounds of a range must be integers",
            )
        })
    }
}
