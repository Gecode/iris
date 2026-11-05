//! A reader spec as a language-independent list of builder calls.
//!
//! [`Program::events`] is flat and in text order: a block's opener, its
//! statements, then [`Event::Close`]. Expressions are postfix slices of
//! [`Program::exprs`]. So a backend needs loops and a stack, and no recursion.

/// An index into [`Program::names`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NameId(pub usize);

/// An index into [`Program::records`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordId(pub usize);

/// An index into [`Program::variables`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableId(pub usize);

/// How the builder gets the handle of a value name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameKind {
    /// Some statement defines the name.
    Forward,
    /// Nothing defines the name, which `defined` and references may still use.
    Undefined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub kind: NameKind,
}

/// The names of a repeat that something refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordNames {
    pub each: String,
    pub list: String,
}

/// A reader spec ready for a backend.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Program {
    /// Value names, in the order that lowering meets them.
    pub names: Vec<Name>,
    /// Repeats that something refers to, in text order.
    pub records: Vec<RecordNames>,
    /// The spellings of comprehension variables.
    pub variables: Vec<String>,
    pub events: Vec<Event>,
    pub exprs: Vec<ExprOp>,
    /// The deepest nesting of blocks, arm lists and comprehensions.
    pub closure_depth: usize,
}

impl Program {
    /// The postfix operations of `expr`.
    pub fn ops(&self, expr: Expr) -> &[ExprOp] {
        &self.exprs[expr.start..expr.end]
    }
}

/// A postfix slice of [`Program::exprs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expr {
    pub start: usize,
    pub end: usize,
}

/// What a definition defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Name(NameId),
    /// `_`, which reads a value without storing it.
    Discard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Format(String),
    Comment(Vec<String>),
    Stop(Vec<String>),
    Tokens(String),
    Separators(String),
    IgnoreTrailing,
    Read {
        target: Target,
        declared: Declared,
    },
    List {
        target: Target,
        declared: Declared,
        end: ListEnd,
    },
    Let {
        target: NameId,
        value: Expr,
    },
    Param {
        target: NameId,
        default: Option<Expr>,
    },
    DefineType {
        name: String,
        ty: Type,
        /// Whether a later type refers to this definition.
        used: bool,
    },
    Check {
        condition: Expr,
        message: Option<String>,
    },
    Expect(Vec<String>),
    Line(Vec<Piece>),
    Find(Vec<Piece>),
    SkipLine,
    SkipLines(Expr),
    Index(Expr),
    /// Opens a block.
    Repeat {
        count: RepeatCount,
        names: RepeatNames,
    },
    /// Opens the block of arms.
    Sections(Vec<SectionClause>),
    /// Opens a block when `block` is set.
    Arm {
        pattern: Vec<Piece>,
        optional: bool,
        last: bool,
        block: bool,
    },
    /// Opens a block.
    If(Expr),
    /// Opens a block, after the [`Event::Close`] of the branch before it.
    ElseIf(Expr),
    /// `else if:type`, which ends the `if` without a block.
    ElseRead {
        target: Target,
        declared: Declared,
    },
    /// Opens a block, after the [`Event::Close`] of the branch before it.
    Else,
    /// Opens a block.
    Instances,
    Close,
}

impl Event {
    /// Whether a later [`Event::Close`] ends a block this event starts.
    pub fn opens_block(&self) -> bool {
        match self {
            Self::Repeat { .. }
            | Self::Sections(_)
            | Self::If(_)
            | Self::ElseIf(_)
            | Self::Else
            | Self::Instances => true,
            Self::Arm { block, .. } => *block,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListEnd {
    Eol,
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepeatCount {
    Times(Expr),
    UntilEof,
    Until {
        terminators: Vec<String>,
        store: Option<NameId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepeatNames {
    /// Something refers to this repeat, through a handle made in advance.
    Forward(RecordId),
    /// Nothing refers to this repeat, so its names are only text.
    InPlace {
        each: Option<String>,
        list: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionClause {
    Until {
        terminators: Vec<String>,
        store: Option<NameId>,
    },
    SkipOther,
}

/// A reading type with the options of its declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    pub ty: Type,
    pub dimensions: Vec<Expr>,
    pub layout: Option<Expr>,
    pub offset: Option<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseType {
    Int,
    Float,
    Num,
    Word,
    Rest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlphabetOption {
    Blank(String),
    Strip(String),
    Base(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Base(BaseType),
    Alphabet {
        symbols: String,
        options: Vec<AlphabetOption>,
    },
    Named(String),
    Range {
        lower: Option<Expr>,
        upper: Option<Expr>,
        /// `..<`, whose upper bound is not in the range.
        exclusive: bool,
    },
}

/// One part of a line pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    /// `{x}` or `{x:type}`.
    Bind {
        name: NameId,
        ty: Option<Type>,
    },
    /// `{$x}`.
    Reference(NameId),
    /// `{:type}`.
    Validate(Type),
    /// `{}`.
    Blank,
    /// `{_}`.
    Discard,
    /// `{_:type}`.
    DiscardAs(Type),
    /// `...`.
    RestOfLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Host {
    IntMax,
    IntMin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Minus,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Function {
    Defined,
    Size,
    Count,
    Min,
    Max,
    Sum,
    Distinct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComprehensionKind {
    All,
    Any,
    Sum,
    Count,
}

/// One step of a postfix expression.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprOp {
    Int(i64),
    Float(f64),
    Str(String),
    Host(Host),
    Name(NameId),
    Var(VariableId),
    /// Each record of a repeat.
    Each(RecordId),
    /// The list of a repeat's records.
    List(RecordId),
    Unary(UnaryOp),
    Binary(BinaryOp),
    Power,
    Index,
    Member(NameId),
    /// The nested list `inner` of the record before it.
    ListMember(RecordId),
    Call {
        function: Function,
        arguments: usize,
    },
    /// Follows a comprehension's domain and starts its body.
    ComprehensionBody(VariableId),
    Comprehension(ComprehensionKind),
}
