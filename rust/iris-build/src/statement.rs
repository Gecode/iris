//! The statement stream: one flat list in text order, with blocks marked by
//! openers and [`Stmt::Close`], so that every pass is a loop over it.
use iris_core::handle::Str;
use std::collections::{HashMap, HashSet};

use crate::error::{Issue, IssueKind};
use crate::expr::{Name, Op, RecordRef, ValueTag, ValueType};
use crate::field::{BuilderId, Field, Shape, intern};
use crate::pattern::{CaptureSpec, Pattern, PatternPart, Piece, piece_name};
use crate::types::{Declared, ListTerminator, TypeRef, TypeSpec};

/// What a definition defines: `name` is `None` for the discard `_`. `levels`
/// counts the definition's own array levels, or is `None` when they are open.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Definition {
    pub name: Option<&'static str>,
    pub value: ValueTag,
    pub levels: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) enum RepeatCount {
    Times(Vec<Op>),
    UntilEof,
    Until {
        terminators: Vec<String>,
        store: Option<Definition>,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum SectionClause {
    Until {
        terminators: Vec<String>,
        store: Option<Definition>,
    },
    SkipOther,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ArmOptions {
    pub optional: bool,
    pub last: bool,
    pub body: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Format(String),
    Comment(Vec<String>),
    Stop(Vec<String>),
    Tokens(String),
    Separators(String),
    IgnoreTrailing,
    Expect(Vec<String>),
    Param {
        definition: Definition,
        default: Option<Vec<Op>>,
    },
    Declaration {
        definition: Definition,
        declared: Declared,
    },
    List {
        definition: Definition,
        declared: Declared,
        end: ListTerminator,
    },
    TypeDefinition {
        name: &'static str,
        declared: Declared,
    },
    Line(Pattern),
    Find(Pattern),
    SkipLine,
    SkipLines(Vec<Op>),
    Index(Vec<Op>),
    Let {
        definition: Definition,
        value: Vec<Op>,
    },
    Check {
        condition: Vec<Op>,
        message: Option<String>,
    },
    Repeat {
        count: RepeatCount,
        record: usize,
    },
    Sections(Vec<SectionClause>),
    Arm {
        pattern: Pattern,
        options: ArmOptions,
    },
    If(Vec<Op>),
    ElseIf(Vec<Op>),
    Else,
    /// `else if:type`, a declaration named `if` as the else branch.
    ElseDeclaration {
        definition: Definition,
        declared: Declared,
    },
    Instances,
    Close,
}

/// Something a statement mentions.
pub(crate) enum Mention<'a> {
    Ops(&'a [Op]),
    Name(Name),
    Type(TypeRef),
}

pub(crate) fn mention_declared(declared: &Declared, visit: &mut impl FnMut(Mention<'_>)) {
    declared
        .dimensions
        .iter()
        .for_each(|dimension| visit(Mention::Ops(dimension)));
    match &declared.ty {
        TypeSpec::Named(named) => visit(Mention::Type(*named)),
        TypeSpec::Range { lower, upper, .. } => [lower, upper]
            .into_iter()
            .flatten()
            .for_each(|bound| visit(Mention::Ops(bound))),
        TypeSpec::Base(_) | TypeSpec::Alphabet { .. } => {}
    }
    [&declared.layout, &declared.offset]
        .into_iter()
        .flatten()
        .for_each(|option| visit(Mention::Ops(option)));
}

fn mention_pattern(pattern: &Pattern, visit: &mut impl FnMut(Mention<'_>)) {
    for piece in &pattern.pieces {
        if let Some((text, owner)) = piece_name(piece) {
            visit(Mention::Name(Name { text, owner }));
        }
        if let Piece::Bind { capture, .. } | Piece::Validate(capture) | Piece::DiscardAs(capture) =
            piece
            && let CaptureSpec::Typed(declared) = capture
        {
            mention_declared(declared, visit);
        }
    }
}

impl Stmt {
    /// Visit every expression and name in this statement.
    pub(crate) fn mentions(&self, visit: &mut impl FnMut(Mention<'_>)) {
        match self {
            Self::Param {
                default: Some(default),
                ..
            } => visit(Mention::Ops(default)),
            Self::Declaration { declared, .. }
            | Self::List { declared, .. }
            | Self::TypeDefinition { declared, .. }
            | Self::ElseDeclaration { declared, .. } => {
                mention_declared(declared, visit);
                if let Self::List {
                    end: ListTerminator::Sentinel(sentinel),
                    ..
                } = self
                {
                    visit(Mention::Ops(sentinel));
                }
            }
            Self::Line(pattern) | Self::Find(pattern) | Self::Arm { pattern, .. } => {
                mention_pattern(pattern, visit);
            }
            Self::SkipLines(ops)
            | Self::Index(ops)
            | Self::If(ops)
            | Self::ElseIf(ops)
            | Self::Let { value: ops, .. }
            | Self::Check { condition: ops, .. }
            | Self::Repeat {
                count: RepeatCount::Times(ops),
                ..
            } => visit(Mention::Ops(ops)),
            _ => {}
        }
    }

    /// Whether this statement starts a block that [`Stmt::Close`] ends.
    pub(crate) fn opens_block(&self) -> bool {
        match self {
            Self::Repeat { .. }
            | Self::Sections(_)
            | Self::If(_)
            | Self::ElseIf(_)
            | Self::Else
            | Self::Instances => true,
            Self::Arm { options, .. } => options.body,
            _ => false,
        }
    }

    /// Whether this statement continues the line of the preceding `}`.
    pub(crate) fn continues_if(&self) -> bool {
        matches!(
            self,
            Self::ElseIf(_) | Self::Else | Self::ElseDeclaration { .. }
        )
    }
}

/// What the builder knows about a name.
#[derive(Debug, Clone)]
pub(crate) enum NameState {
    Defined,
    Forward(Shape),
    /// From `undefined`; nothing may define it.
    Undefined,
    Capture {
        spec: CaptureSpec,
        shape: Shape,
    },
}

#[derive(Debug, Default)]
pub(crate) struct RecordInfo {
    pub each: Option<&'static str>,
    pub list: Option<&'static str>,
    /// Made by `forward_record`, awaiting `record_as`.
    pub forward: bool,
    pub bound: bool,
    /// The forward record this repeat was bound to.
    pub link: Option<usize>,
}

/// The builder's state, shared by its scopes and guards.
#[derive(Debug)]
pub(crate) struct State {
    pub id: BuilderId,
    pub stmts: Vec<Stmt>,
    /// The ids of the open blocks, innermost last.
    pub open: Vec<usize>,
    next_block: usize,
    pub names: HashMap<&'static str, NameState>,
    /// Forward handles and captures in creation order, for reporting.
    pub declared: Vec<&'static str>,
    pub types: HashSet<&'static str>,
    pub records: Vec<RecordInfo>,
    pub issue: Option<Issue>,
}

pub(crate) const ROOT_BLOCK: usize = 0;

impl State {
    pub fn new() -> Self {
        Self {
            id: BuilderId::next(),
            stmts: Vec::new(),
            open: vec![ROOT_BLOCK],
            next_block: ROOT_BLOCK + 1,
            names: HashMap::new(),
            declared: Vec::new(),
            types: HashSet::new(),
            records: Vec::new(),
            issue: None,
        }
    }

    /// Keep the first issue; the checks after it may only be its consequences.
    pub fn report(&mut self, kind: IssueKind, message: impl Into<String>) {
        self.issue.get_or_insert(Issue::new(kind, message));
    }

    pub fn open_block(&mut self) -> usize {
        let block = self.next_block;
        self.next_block += 1;
        self.open.push(block);
        block
    }

    pub fn close_block(&mut self) {
        self.open.pop();
        self.stmts.push(Stmt::Close);
    }

    pub fn check_innermost(&mut self, block: usize) {
        if self.open.last() != Some(&block) {
            self.report(
                IssueKind::Scope,
                "a statement went to a scope that is not the innermost open block; \
                 use the scope of the closure it is in",
            );
        }
    }

    pub fn is_last(&self, index: usize) -> bool {
        index + 1 == self.stmts.len()
    }

    /// Check that a guard's statement at `index` is still the last one.
    pub fn check_last(&mut self, index: usize) {
        if !self.is_last(index) {
            self.report(
                IssueKind::Scope,
                "a guard was used after later statements; \
                 finish a statement before starting the next one",
            );
        }
    }

    /// Remove the `}` at `index`, so that a block can take its body.
    pub fn reopen(&mut self, index: usize) {
        self.check_last(index);
        if self.is_last(index) {
            self.stmts.pop();
        }
    }

    pub fn check_owner(&mut self, owner: BuilderId, what: &str, name: &str) {
        if owner != self.id {
            self.report(
                IssueKind::Provenance,
                format!("{what} `{name}` does not come from this builder"),
            );
        }
    }

    pub fn check_record(&mut self, reference: RecordRef) {
        if reference.owner != self.id {
            self.report(
                IssueKind::Provenance,
                "a record handle does not come from this builder",
            );
        }
    }

    fn check_ops(&mut self, ops: &[Op]) {
        for op in ops {
            match op {
                Op::Name(name) | Op::Member(name) => {
                    self.check_owner(name.owner, "the handle", name.text);
                }
                Op::Record(reference) | Op::RecordList(reference) | Op::ListMember(reference) => {
                    self.check_record(*reference);
                }
                _ => {}
            }
        }
    }

    fn check_mention(&mut self, mention: Mention<'_>) {
        match mention {
            Mention::Ops(ops) => self.check_ops(ops),
            Mention::Name(name) => self.check_owner(name.owner, "the handle", name.text),
            Mention::Type(named) => {
                self.check_owner(named.name.owner, "the type", named.name.text);
            }
        }
    }

    /// Check the handles of a capture's type, which a pattern prints later.
    pub fn check_capture(&mut self, capture: &CaptureSpec) {
        if let CaptureSpec::Typed(declared) = capture {
            mention_declared(declared, &mut |mention| self.check_mention(mention));
        }
    }

    /// Check the handles of `stmt` and append it to the block `block`.
    pub fn push(&mut self, block: usize, stmt: Stmt) -> usize {
        self.check_innermost(block);
        stmt.mentions(&mut |mention| self.check_mention(mention));
        self.stmts.push(stmt);
        self.stmts.len() - 1
    }

    /// Follow a repeat's record to the forward record it was bound to.
    pub fn record_id(&self, id: usize) -> usize {
        self.records
            .get(id)
            .and_then(|record| record.link)
            .unwrap_or(id)
    }

    pub fn new_record(&mut self) -> usize {
        self.records.push(RecordInfo::default());
        self.records.len() - 1
    }

    /// Check the name of a definition, and intern it. A declaration, list or
    /// capture named `_` stores nothing, and `let`, `param` and `type` reject
    /// `_`, so no definition gives a handle for it.
    pub fn definition_name(&mut self, name: &str) -> &'static str {
        if name == "_" {
            self.report(
                IssueKind::Unprintable,
                "`_` names nothing here: use `discard()` to read without storing, \
                 or `forward(\"_\")` with `store` for the entry named `_`",
            );
        }
        self.spelled_name(name)
    }

    /// Check a name that may be `_`, and intern it: a repeat's `each` or `as`
    /// name, a forward handle for `store`, or a name left undefined.
    pub fn spelled_name(&mut self, name: &str) -> &'static str {
        if name.starts_with("gecode.") {
            self.report(
                IssueKind::Unprintable,
                format!("a definition cannot be named `{name}`: the prefix `gecode.` is reserved"),
            );
        } else if !is_identifier(name) {
            self.report(
                IssueKind::Unprintable,
                format!("`{name}` is not an Iris name"),
            );
        }
        intern(name)
    }

    /// Resolve a definition's target.
    pub fn define(
        &mut self,
        target: TargetName,
        value: ValueTag,
        levels: Option<usize>,
    ) -> Definition {
        let name = match target {
            TargetName::Discard => None,
            TargetName::New(name) => {
                let name = self.definition_name(&name);
                if self.names.contains_key(name) {
                    self.report(
                        IssueKind::Redefinition,
                        format!(
                            "`{name}` is already defined; to redefine it, pass its handle \
                             as the target, or use `set`"
                        ),
                    );
                }
                self.names.insert(name, NameState::Defined);
                Some(name)
            }
            TargetName::Existing(name) => {
                self.check_owner(name.owner, "the handle", name.text);
                Some(name.text)
            }
        };
        Definition {
            name,
            value,
            levels,
        }
    }

    /// Declare a checked name for a forward handle, a capture, or `undefined`.
    pub fn declare(&mut self, name: &'static str, state: NameState) -> &'static str {
        if self.names.contains_key(name) {
            self.report(
                IssueKind::Redefinition,
                format!("`{name}` is already declared"),
            );
        }
        self.names.insert(name, state);
        self.declared.push(name);
        name
    }
}

pub(crate) fn is_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// What a definition's target names.
#[derive(Debug)]
pub enum TargetName {
    Discard,
    New(String),
    Existing(Name),
}

/// The target of a defining statement: a new name, an existing handle, or
/// [`discard`].
///
/// A new name gives a typed handle, and `_` is not one. An existing handle
/// redefines its name and gives a [`DynField`](crate::DynField), since the value
/// type or depth may have changed. [`discard`] gives nothing.
pub trait Target {
    type Out<V, D, O>;
    #[doc(hidden)]
    fn into_target(self) -> TargetName;
    #[doc(hidden)]
    fn handle<V, D, O>(name: Option<&'static str>, owner: BuilderId) -> Self::Out<V, D, O>;
}

fn named<V, D, O>(name: Option<&'static str>, owner: BuilderId) -> Field<V, D, O> {
    Field::new(name.expect("a named target has a name"), owner)
}

impl Target for &str {
    type Out<V, D, O> = Field<V, D, O>;
    fn into_target(self) -> TargetName {
        TargetName::New(self.to_owned())
    }
    fn handle<V, D, O>(name: Option<&'static str>, owner: BuilderId) -> Field<V, D, O> {
        named(name, owner)
    }
}

impl Target for String {
    type Out<V, D, O> = Field<V, D, O>;
    fn into_target(self) -> TargetName {
        TargetName::New(self)
    }
    fn handle<V, D, O>(name: Option<&'static str>, owner: BuilderId) -> Field<V, D, O> {
        named(name, owner)
    }
}

impl<A, B, C> Target for Field<A, B, C> {
    type Out<V, D, O> = crate::DynField;
    fn into_target(self) -> TargetName {
        TargetName::Existing(Name {
            text: self.name,
            owner: self.owner,
        })
    }
    fn handle<V, D, O>(name: Option<&'static str>, owner: BuilderId) -> crate::DynField {
        named(name, owner)
    }
}

/// The discard `_`: a target, and `{_}` in a pattern.
#[derive(Debug, Clone, Copy)]
pub struct Discard;

/// `_`, reading a value without storing it. A `store` does not discard, so it
/// takes a handle from `forward("_")` instead.
pub fn discard() -> Discard {
    Discard
}

impl Target for Discard {
    type Out<V, D, O> = ();
    fn into_target(self) -> TargetName {
        TargetName::Discard
    }
    fn handle<V, D, O>(_: Option<&'static str>, _: BuilderId) {}
}

impl PatternPart for Discard {
    fn into_piece(self) -> Piece {
        Piece::Discard
    }
}

/// A list of strings: one string, or an array, slice or `Vec` of them.
pub trait Strings {
    #[doc(hidden)]
    fn into_strings(self) -> Vec<String>;
}

impl Strings for &str {
    fn into_strings(self) -> Vec<String> {
        vec![self.to_owned()]
    }
}
impl Strings for String {
    fn into_strings(self) -> Vec<String> {
        vec![self]
    }
}
impl<S: AsRef<str>, const N: usize> Strings for [S; N] {
    fn into_strings(self) -> Vec<String> {
        self.iter().map(|text| text.as_ref().to_owned()).collect()
    }
}
impl<S: AsRef<str>> Strings for &[S] {
    fn into_strings(self) -> Vec<String> {
        self.iter().map(|text| text.as_ref().to_owned()).collect()
    }
}
impl<S: AsRef<str>> Strings for Vec<S> {
    fn into_strings(self) -> Vec<String> {
        self.iter().map(|text| text.as_ref().to_owned()).collect()
    }
}

/// The handle of a repeat's records. One handle serves projection
/// (`job.get(machine)`, printed `job.machine`) and comprehensions
/// (`sum_over(job, ..)`, printed `j in jobs`).
#[derive(Debug, Clone, Copy)]
pub struct Record {
    pub(crate) id: usize,
    pub(crate) owner: BuilderId,
}

impl Record {
    pub(crate) fn reference(self) -> RecordRef {
        RecordRef {
            id: self.id,
            owner: self.owner,
        }
    }
}

/// The definition a terminator's `store` makes.
pub(crate) fn store_definition(state: &mut State, target: TargetName) -> Definition {
    if matches!(target, TargetName::Discard) {
        state.report(
            IssueKind::Unprintable,
            "`store _` keeps an entry named `_`, so `discard()` cannot spell it: \
             pass a handle from `forward(\"_\")`",
        );
    }
    state.define(target, Str::TAG, Some(0))
}
