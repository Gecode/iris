use gecode_iris_syntax::{SyntaxNode, TextRange, TextSize};

macro_rules! ids {
    ($($name:ident),*) => { $(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub usize);
    )* };
}
ids!(DefinitionId, ReferenceId, ScopeId, RepeatId);

#[derive(Debug, Clone, PartialEq, Eq)]
/// Facts returned by analysis use nonempty, deduplicated `Known` sets.
pub enum Possibilities<T> {
    Unknown,
    Known(Vec<T>),
}
impl<T: Clone + PartialEq> Possibilities<T> {
    pub fn one(value: T) -> Self {
        Self::Known(vec![value])
    }
    pub fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Known(a), Self::Known(b)) => {
                let mut values = a.clone();
                for value in b {
                    if !values.contains(value) {
                        values.push(value.clone());
                    }
                }
                Self::Known(values)
            }
            _ => Self::Unknown,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    Int,
    Float,
    String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueFacts {
    pub value_types: Possibilities<ValueType>,
    pub depths: Possibilities<usize>,
}
impl ValueFacts {
    pub fn unknown() -> Self {
        Self {
            value_types: Possibilities::Unknown,
            depths: Possibilities::Unknown,
        }
    }
    pub fn known(value_type: ValueType, depth: usize) -> Self {
        Self {
            value_types: Possibilities::one(value_type),
            depths: Possibilities::one(depth),
        }
    }
    pub fn join(&self, other: &Self) -> Self {
        Self {
            value_types: self.value_types.join(&other.value_types),
            depths: self.depths.join(&other.depths),
        }
    }
    pub(crate) fn shifted(&self, extra: isize) -> Self {
        let depths = match &self.depths {
            Possibilities::Known(depths) => {
                let values: Option<Vec<_>> =
                    depths.iter().map(|d| d.checked_add_signed(extra)).collect();
                values.map_or(Possibilities::Unknown, Possibilities::Known)
            }
            Possibilities::Unknown => Possibilities::Unknown,
        };
        Self {
            value_types: self.value_types.clone(),
            depths,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefinitionKind {
    Read,
    Capture,
    Let,
    Param,
    Store,
    NamedType,
    RecordList,
    EachRecord,
    Binding,
}
#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub kind: DefinitionKind,
    pub name_range: TextRange,
    pub node: SyntaxNode,
    pub scope: ScopeId,
    pub repeats: Vec<RepeatId>,
    pub facts: ValueFacts,
    pub references: Vec<ReferenceId>,
}
impl Definition {
    pub(crate) fn is_ordinary(&self) -> bool {
        self.name != "_"
            && matches!(
                self.kind,
                DefinitionKind::Read
                    | DefinitionKind::Capture
                    | DefinitionKind::Let
                    | DefinitionKind::Param
                    | DefinitionKind::Store
            )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denotation {
    Value,
    Record(RepeatId),
    RecordList(RepeatId),
}
#[derive(Debug, Clone)]
pub enum Resolution {
    Value {
        definitions: Vec<DefinitionId>,
        possibly_unavailable: bool,
        projection: Vec<RepeatId>,
    },
    Bound(DefinitionId),
    NamedType(DefinitionId),
    Structural {
        definition: DefinitionId,
        denotation: Denotation,
    },
    Field {
        definitions: Vec<DefinitionId>,
        record: RepeatId,
    },
    HostConstant(i64),
}
impl Resolution {
    pub fn definitions(&self) -> &[DefinitionId] {
        match self {
            Self::Value { definitions, .. } | Self::Field { definitions, .. } => definitions,
            Self::Bound(id) | Self::NamedType(id) | Self::Structural { definition: id, .. } => {
                std::slice::from_ref(id)
            }
            Self::HostConstant(_) => &[],
        }
    }
}
#[derive(Debug, Clone)]
pub struct Reference {
    pub name: String,
    pub name_range: TextRange,
    pub resolution: Resolution,
    pub facts: ValueFacts,
}
#[derive(Debug, Clone)]
pub struct Scope {
    pub node: SyntaxNode,
    pub parent: Option<ScopeId>,
    pub type_scope: ScopeId,
    pub value_frame: ScopeId,
    pub repeats: Vec<RepeatId>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingKind {
    Int,
    Float,
    Num,
    Word,
    Rest,
    Range,
    Alphabet,
}
impl ReadingKind {
    pub(crate) fn facts(self, depth: usize) -> ValueFacts {
        match self {
            Self::Float => ValueFacts::known(ValueType::Float, depth),
            Self::Word | Self::Rest => ValueFacts::known(ValueType::String, depth),
            Self::Num => ValueFacts {
                value_types: Possibilities::Known(vec![ValueType::Int, ValueType::Float]),
                depths: Possibilities::one(depth),
            },
            _ => ValueFacts::known(ValueType::Int, depth),
        }
    }
    pub(crate) fn numeric(self) -> bool {
        matches!(self, Self::Int | Self::Float | Self::Num | Self::Range)
    }
}
#[derive(Debug, Clone)]
pub struct NamedType {
    pub definition: DefinitionId,
    pub kind: ReadingKind,
    pub source: SyntaxNode,
}
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub definitions: Vec<DefinitionId>,
    pub fallback: ValueFacts,
}
#[derive(Debug, Clone)]
pub struct RepeatInfo {
    pub node: SyntaxNode,
    pub parent: Option<RepeatId>,
    pub chain: Vec<RepeatId>,
    pub body_scope: Option<ScopeId>,
    pub list_definition: Option<DefinitionId>,
    pub each_definition: Option<DefinitionId>,
    pub fields: Vec<FieldInfo>,
}
#[derive(Debug, Clone)]
pub struct ExpressionInfo {
    pub node: SyntaxNode,
    pub facts: ValueFacts,
    pub denotation: Denotation,
}
#[derive(Debug, Clone)]
pub struct LiftDependency {
    pub label: String,
    pub repeats: Vec<RepeatId>,
}
#[derive(Debug, Clone)]
pub struct CheckInfo {
    pub node: SyntaxNode,
    pub dependencies: Vec<LiftDependency>,
    pub deepest: Vec<RepeatId>,
}
#[derive(Debug, Clone)]
pub struct ComprehensionInfo {
    pub node: SyntaxNode,
    pub binding: DefinitionId,
    pub domain: SyntaxNode,
    pub record_elements: Option<RepeatId>,
}
/// An entry that a successful read can leave in its data. Catalog data is not
/// described here: it has its own entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputEntry {
    pub name: String,
    /// Every definition of the name that can reach the data, in source order.
    /// More than one means that the name is redefined.
    pub definitions: Vec<DefinitionId>,
    /// The depths the entry can have, and the value types of its leaves. An
    /// entry without leaves can have any value type in its data.
    pub facts: ValueFacts,
    /// Whether a successful read can leave no such entry. `false` is a proof.
    /// `true` can also mean only that the analysis found no proof.
    pub possibly_absent: bool,
}
#[derive(Debug, Default, Clone)]
pub struct SemanticModel {
    pub(crate) definitions: Vec<Definition>,
    pub(crate) references: Vec<Reference>,
    pub(crate) scopes: Vec<Scope>,
    pub(crate) repeats: Vec<RepeatInfo>,
    pub(crate) named_types: Vec<NamedType>,
    pub(crate) expressions: Vec<ExpressionInfo>,
    pub(crate) checks: Vec<CheckInfo>,
    pub(crate) comprehensions: Vec<ComprehensionInfo>,
    pub(crate) output_entries: Vec<OutputEntry>,
}
impl SemanticModel {
    pub fn definitions(&self) -> &[Definition] {
        &self.definitions
    }
    pub fn references(&self) -> &[Reference] {
        &self.references
    }
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }
    pub fn repeats(&self) -> &[RepeatInfo] {
        &self.repeats
    }
    pub fn named_types(&self) -> &[NamedType] {
        &self.named_types
    }
    pub fn expressions(&self) -> &[ExpressionInfo] {
        &self.expressions
    }
    pub fn checks(&self) -> &[CheckInfo] {
        &self.checks
    }
    pub fn comprehensions(&self) -> &[ComprehensionInfo] {
        &self.comprehensions
    }
    /// The entries of a successful read, by first definition. Empty when the
    /// spec is invalid.
    pub fn output_entries(&self) -> &[OutputEntry] {
        &self.output_entries
    }
    pub fn definition(&self, id: DefinitionId) -> &Definition {
        &self.definitions[id.0]
    }
    pub fn reference(&self, id: ReferenceId) -> &Reference {
        &self.references[id.0]
    }
    pub fn definition_at(&self, offset: TextSize) -> Option<DefinitionId> {
        self.definitions
            .iter()
            .position(|d| d.name_range.contains(offset))
            .map(DefinitionId)
    }
    pub fn reference_at(&self, offset: TextSize) -> Option<ReferenceId> {
        self.references
            .iter()
            .position(|r| r.name_range.contains(offset))
            .map(ReferenceId)
    }
    pub fn scope_at(&self, offset: TextSize) -> Option<ScopeId> {
        self.scopes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.node.text_range().contains_inclusive(offset))
            .min_by_key(|(_, s)| s.node.text_range().len())
            .map(|(i, _)| ScopeId(i))
    }
    pub fn expression(&self, node: &SyntaxNode) -> Option<&ExpressionInfo> {
        self.expressions.iter().find(|e| e.node == *node)
    }
    pub fn facts_for_definition(&self, id: DefinitionId) -> &ValueFacts {
        &self.definition(id).facts
    }
    pub(crate) fn add_reference(&mut self, reference: Reference) -> ReferenceId {
        let id = ReferenceId(self.references.len());
        for target in reference.resolution.definitions() {
            self.definitions[target.0].references.push(id);
        }
        self.references.push(reference);
        id
    }
}
