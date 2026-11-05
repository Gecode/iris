//! Scopes, the guards of block statements, and the builder.
use iris_core::Spec;
use iris_core::handle::{
    D0, D1, D2, D3, D4, D5, D6, D7, D8, Depth, Dyn, DynDepth, Maybe, Presence, Required,
};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::check::check_bindings;
use crate::error::{BuildError, IssueKind};
use crate::expr::{
    LetDepth, Name, Operand, SameDepth, ValueTag, ValueType, let_levels, scalar_levels,
};
use crate::field::{BuilderId, DynField, Field, Forward, Shape};
use crate::pattern::{CaptureType, Pattern};
use crate::print::print;
use crate::statement::{
    ArmOptions, Definition, NameState, ROOT_BLOCK, Record, RepeatCount, SectionClause, State, Stmt,
    Strings, Target, store_definition,
};
use crate::types::{IntoDeclared, ListReading, NamedType};

type Shared = Rc<RefCell<State>>;

/// The origin reported for the text a builder emits.
const ORIGIN: &str = "builder.iris";

/// What reading `R` into `T` gives in a scope at depth `D` with presence `O`.
type ReadOut<T, R, D, O> = <T as Target>::Out<
    <R as IntoDeclared>::Value,
    <<R as IntoDeclared>::Dims as Depth>::Plus<D>,
    O,
>;

/// The depths inside at least one repeat.
pub trait Nested: Depth {}
macro_rules! nested_depths {
    ($($depth:ty),*) => {$(impl Nested for $depth {})*};
}
nested_depths!(D1, D2, D3, D4, D5, D6, D7, D8, DynDepth);

/// An open block. `D` counts the repeats around it, and `O` says whether its
/// statements always execute.
///
/// A scope accepts statements only while it is the innermost open block; a
/// statement added to an outer scope fails `build()`.
#[derive(Debug)]
pub struct Scope<D = D0, O = Required> {
    state: Shared,
    block: usize,
    /// The record of the nearest repeat.
    record: Option<usize>,
    marker: PhantomData<fn() -> (D, O)>,
}

fn enter<D, O, R>(
    state: &Shared,
    record: Option<usize>,
    f: impl FnOnce(&mut Scope<D, O>) -> R,
) -> R {
    let block = state.borrow_mut().open_block();
    let mut scope = Scope {
        state: Rc::clone(state),
        block,
        record,
        marker: PhantomData,
    };
    let result = f(&mut scope);
    state.borrow_mut().close_block();
    result
}

impl<D: Depth, O: Presence> Scope<D, O> {
    fn push(&mut self, stmt: Stmt) -> usize {
        self.state.borrow_mut().push(self.block, stmt)
    }

    /// Resolve `target`, build its statement, and return its handle.
    fn define<T: Target, V, HD, HO>(
        &mut self,
        target: T,
        value: ValueTag,
        levels: Option<usize>,
        statement: impl FnOnce(Definition) -> Stmt,
    ) -> T::Out<V, HD, HO> {
        let mut state = self.state.borrow_mut();
        let definition = state.define(target.into_target(), value, levels);
        state.push(self.block, statement(definition));
        T::handle(definition.name, state.id)
    }

    /// `x: type`, reading a value into `target`.
    pub fn read<T: Target, R: IntoDeclared>(&mut self, target: T, ty: R) -> ReadOut<T, R, D, O> {
        let declared = ty.into_declared();
        let levels = Some(declared.dimensions.len());
        self.define(target, R::Value::TAG, levels, |definition| {
            Stmt::Declaration {
                definition,
                declared,
            }
        })
    }

    /// `list x: type until end`.
    pub fn list<T: Target, V: ValueType>(
        &mut self,
        target: T,
        reading: ListReading<V>,
    ) -> T::Out<V, D::Next, O> {
        let ListReading { declared, end, .. } = reading;
        self.define(target, V::TAG, Some(1), |definition| Stmt::List {
            definition,
            declared,
            end,
        })
    }

    /// `let x = value`.
    pub fn let_<T: Target, E: Operand>(
        &mut self,
        target: T,
        value: E,
    ) -> T::Out<E::Value, LetDepth<E, D>, O> {
        let value = value.into_expr();
        let levels = let_levels::<E, D>(&value);
        let value = value.ops;
        self.define(target, E::Value::TAG, levels, |definition| Stmt::Let {
            definition,
            value,
        })
    }

    /// `let x = value` through a handle, keeping its type. Rust needs the scope
    /// here, so principle 2's `handle.set(expr)` is `s.set(handle, expr)`.
    ///
    /// The value must have the handle's value type and, where its type says,
    /// its depth. To change either, use [`let_`](Self::let_) with the handle as
    /// the target, which returns a [`DynField`].
    pub fn set<V: ValueType, HD: Depth, HO, E>(&mut self, handle: Field<V, HD, HO>, value: E)
    where
        E: Operand<Value = V>,
        E::DepthIn<D>: SameDepth<HD>,
    {
        let value = value.into_expr();
        let levels = let_levels::<E, D>(&value);
        let value = value.ops;
        self.define::<_, (), (), ()>(handle, V::TAG, levels, |definition| Stmt::Let {
            definition,
            value,
        });
    }

    /// `param p`. The caller decides its type.
    pub fn param<T: Target>(&mut self, target: T) -> T::Out<Dyn, DynDepth, O> {
        self.define(target, Dyn::TAG, None, |definition| Stmt::Param {
            definition,
            default: None,
        })
    }

    /// `param p = default`.
    pub fn param_default<T: Target, E: Operand>(
        &mut self,
        target: T,
        default: E,
    ) -> T::Out<E::Value, LetDepth<E, D>, O> {
        let levels = scalar_levels::<E>();
        let default = Some(default.into_expr().ops);
        self.define(target, E::Value::TAG, levels, |definition| Stmt::Param {
            definition,
            default,
        })
    }

    /// `check condition`. Add a message with [`Check::message`].
    pub fn check<E: Operand>(&mut self, condition: E) -> Check {
        let condition = condition.into_expr().ops;
        let stmt = self.push(Stmt::Check {
            condition,
            message: None,
        });
        Check {
            state: Rc::clone(&self.state),
            stmt,
        }
    }

    /// `expect "text"...`.
    pub fn expect(&mut self, words: impl Strings) {
        self.push(Stmt::Expect(words.into_strings()));
    }

    /// `line "pattern"`.
    pub fn line(&mut self, pattern: impl Into<Pattern>) {
        self.push(Stmt::Line(pattern.into()));
    }

    /// `find "pattern"`.
    pub fn find(&mut self, pattern: impl Into<Pattern>) {
        self.push(Stmt::Find(pattern.into()));
    }

    /// `skip line`.
    pub fn skip_line(&mut self) {
        self.push(Stmt::SkipLine);
    }

    /// `skip count lines`.
    pub fn skip_lines<E: Operand>(&mut self, count: E) {
        self.push(Stmt::SkipLines(count.into_expr().ops));
    }

    /// `index position`.
    pub fn index<E: Operand>(&mut self, position: E) {
        self.push(Stmt::Index(position.into_expr().ops));
    }

    /// `type name = type`, usable wherever a type is.
    pub fn define_type<R: IntoDeclared<Dims = D0>>(
        &mut self,
        name: &str,
        ty: R,
    ) -> NamedType<R::Value> {
        let mut state = self.state.borrow_mut();
        let name = state.definition_name(name);
        state.types.insert(name);
        let declared = ty.into_declared();
        let declaration = state.push(self.block, Stmt::TypeDefinition { name, declared });
        let name = Name {
            text: name,
            owner: state.id,
        };
        NamedType::new(name, declaration)
    }

    fn open_repeat(&mut self, count: RepeatCount) -> (usize, usize) {
        let mut state = self.state.borrow_mut();
        let record = state.new_record();
        (
            state.push(self.block, Stmt::Repeat { count, record }),
            record,
        )
    }

    /// `repeat count { ... }`.
    pub fn repeat<E: Operand, R>(
        &mut self,
        count: E,
        f: impl FnOnce(&mut Scope<D::Next, O>) -> R,
    ) -> R {
        let (_, record) = self.open_repeat(RepeatCount::Times(count.into_expr().ops));
        enter(&self.state, Some(record), f)
    }

    fn repeat_guard(&mut self, count: RepeatCount) -> Repeat<D, O> {
        let (stmt, record) = self.open_repeat(count);
        self.state.borrow_mut().stmts.push(Stmt::Close);
        Repeat {
            state: Rc::clone(&self.state),
            close: stmt + 1,
            record,
            marker: PhantomData,
        }
    }

    /// `repeat until eof`. Give it a block with [`Repeat::body`].
    pub fn repeat_until_eof(&mut self) -> Repeat<D, O> {
        self.repeat_guard(RepeatCount::UntilEof)
    }

    /// `repeat until "text"...`. Give it a block with [`RepeatUntil::body`].
    pub fn repeat_until(&mut self, terminators: impl Strings) -> RepeatUntil<D, O> {
        RepeatUntil(self.repeat_guard(RepeatCount::Until {
            terminators: terminators.into_strings(),
            store: None,
        }))
    }

    /// `sections`. Give it arms with [`Sections::body`].
    pub fn sections(&mut self) -> Sections<D, O> {
        let stmt = self.push(Stmt::Sections(Vec::new()));
        self.state.borrow_mut().stmts.push(Stmt::Close);
        Sections {
            state: Rc::clone(&self.state),
            stmt,
            record: self.record,
            marker: PhantomData,
        }
    }

    /// `if condition { ... }`, returning a guard for its `else`.
    pub fn if_<E: Operand, R>(
        &mut self,
        condition: E,
        f: impl FnOnce(&mut Scope<D, Maybe>) -> R,
    ) -> If<D, O, R> {
        self.push(Stmt::If(condition.into_expr().ops));
        let result = enter(&self.state, self.record, f);
        If::after(Rc::clone(&self.state), self.record, self.block, result)
    }

    /// A handle for a pattern to bind, at this scope's depth and presence.
    pub fn capture<T: CaptureType>(&mut self, name: &str, ty: T) -> Field<T::Value, D, O> {
        let (name, owner) = self.declare_capture::<T, O>(name, ty);
        Field::new(name, owner)
    }

    /// [`Scope::capture`] for an optional arm or a conditional block.
    pub fn optional_capture<T: CaptureType>(
        &mut self,
        name: &str,
        ty: T,
    ) -> Field<T::Value, D, Maybe> {
        let (name, owner) = self.declare_capture::<T, Maybe>(name, ty);
        Field::new(name, owner)
    }

    fn declare_capture<T: CaptureType, P: Presence>(
        &mut self,
        name: &str,
        ty: T,
    ) -> (&'static str, BuilderId) {
        let mut state = self.state.borrow_mut();
        let shape = Shape {
            value: T::Value::TAG,
            levels: D::LEVELS,
            required: P::REQUIRED,
        };
        let spec = ty.into_capture();
        state.check_capture(&spec);
        let name = state.definition_name(name);
        (
            state.declare(name, NameState::Capture { spec, shape }),
            state.id,
        )
    }

    /// A handle for a name that a later statement defines.
    ///
    /// The annotation gives its type: `let x: Field<Int, D1> = s.forward("x")`,
    /// or [`DynField`] for one that stays open. `build()` reports a forward
    /// handle that nothing defines. A definition that no earlier binding always
    /// precedes must have the handle's type, and a value whose type does not
    /// show its value type or depth needs a [`DynField`]. After a binding, a
    /// definition of another type is a redefinition: use the handle it returns.
    ///
    /// `forward("_")` is the handle of the entry that `store _` keeps. Only
    /// `store` defines it, and inside a repeat its value lasts for the
    /// iteration, so it reaches the data only from outside every repeat.
    pub fn forward<H: Forward>(&mut self, name: &str) -> H {
        let mut state = self.state.borrow_mut();
        let name = state.spelled_name(name);
        let name = state.declare(name, NameState::Forward(H::shape()));
        H::handle(name, state.id)
    }

    /// A handle for a name deliberately left undefined, as in
    /// `check !defined(name)`. Defining it fails `build()`.
    pub fn undefined(&mut self, name: &str) -> DynField {
        let mut state = self.state.borrow_mut();
        let name = state.spelled_name(name);
        let name = state.declare(name, NameState::Undefined);
        Field::new(name, state.id)
    }

    /// The record of a repeat that comes later in the text, printed with the
    /// names given here. Bind it with [`Scope::record_as`].
    pub fn forward_record(&mut self, each: &str, list: &str) -> Record {
        let mut state = self.state.borrow_mut();
        let (each, list) = (state.spelled_name(each), state.spelled_name(list));
        let id = state.new_record();
        let record = &mut state.records[id];
        record.each = Some(each);
        record.list = Some(list);
        record.forward = true;
        Record {
            id,
            owner: state.id,
        }
    }
}

impl<D: Nested, O: Presence> Scope<D, O> {
    fn record_id(&self) -> usize {
        self.record.expect("a nested scope is inside a repeat")
    }

    /// The record of the nearest repeat.
    pub fn record(&self) -> Record {
        Record {
            id: self.record_id(),
            owner: self.state.borrow().id,
        }
    }

    /// Print this repeat's `each` name as `name`, which may be `_`.
    pub fn each_name(&mut self, name: &str) {
        let mut state = self.state.borrow_mut();
        let name = state.spelled_name(name);
        let id = state.record_id(self.record_id());
        state.records[id].each = Some(name);
    }

    /// Print this repeat's `as` name as `name`, which may be `_`.
    pub fn as_name(&mut self, name: &str) {
        let mut state = self.state.borrow_mut();
        let name = state.spelled_name(name);
        let id = state.record_id(self.record_id());
        state.records[id].list = Some(name);
    }

    /// Bind this repeat to a record from [`Scope::forward_record`].
    pub fn record_as(&mut self, record: Record) {
        let mut state = self.state.borrow_mut();
        state.check_record(record.reference());
        let forward = state.records.get(record.id).filter(|info| info.forward);
        if forward.is_none_or(|info| info.bound) {
            state.report(
                IssueKind::Binding,
                "`record_as` needs a record from `forward_record` that is not bound yet",
            );
            return;
        }
        state.records[record.id].bound = true;
        let own = self.record_id();
        state.records[own].link = Some(record.id);
    }
}

/// The `check` guard.
#[derive(Debug)]
pub struct Check {
    state: Shared,
    stmt: usize,
}

impl Check {
    /// The message of a failed check.
    pub fn message(self, text: impl Into<String>) {
        let mut state = self.state.borrow_mut();
        state.check_last(self.stmt);
        if let Stmt::Check { message, .. } = &mut state.stmts[self.stmt] {
            *message = Some(text.into());
        }
    }
}

/// The guard of a `repeat` with an end condition.
#[derive(Debug)]
pub struct Repeat<D, O> {
    state: Shared,
    close: usize,
    record: usize,
    marker: PhantomData<fn() -> (D, O)>,
}

impl<D: Depth, O> Repeat<D, O> {
    /// The repeat's block.
    pub fn body<R>(self, f: impl FnOnce(&mut Scope<D::Next, O>) -> R) -> R {
        self.state.borrow_mut().reopen(self.close);
        enter(&self.state, Some(self.record), f)
    }
}

/// The guard of a `repeat until "text"`.
#[derive(Debug)]
pub struct RepeatUntil<D, O>(Repeat<D, O>);

impl<D: Depth, O> RepeatUntil<D, O> {
    /// `store x`, keeping the terminator that ended the repeat. To read it,
    /// pass a handle from [`Scope::forward`].
    pub fn store<T: Target>(self, target: T) -> Self {
        {
            let mut state = self.0.state.borrow_mut();
            state.check_last(self.0.close);
            let definition = store_definition(&mut state, target.into_target());
            if let Some(Stmt::Repeat {
                count: RepeatCount::Until { store, .. },
                ..
            }) = state.stmts.get_mut(self.0.close - 1)
            {
                *store = Some(definition);
            }
        }
        self
    }

    /// The repeat's block.
    pub fn body<R>(self, f: impl FnOnce(&mut Scope<D::Next, O>) -> R) -> R {
        self.0.body(f)
    }
}

/// The guard of a `sections` statement.
#[derive(Debug)]
pub struct Sections<D, O> {
    state: Shared,
    stmt: usize,
    record: Option<usize>,
    marker: PhantomData<fn() -> (D, O)>,
}

impl<D, O> Sections<D, O> {
    fn add(self, clause: SectionClause) -> Self {
        {
            let mut state = self.state.borrow_mut();
            state.check_last(self.stmt + 1);
            if let Stmt::Sections(clauses) = &mut state.stmts[self.stmt] {
                clauses.push(clause);
            }
        }
        self
    }

    /// `until "text"...`.
    pub fn until(self, terminators: impl Strings) -> Self {
        self.add(SectionClause::Until {
            terminators: terminators.into_strings(),
            store: None,
        })
    }

    /// `skip other`.
    pub fn skip_other(self) -> Self {
        self.add(SectionClause::SkipOther)
    }

    /// `store x` on the preceding `until`. To read it, pass a handle from
    /// [`Scope::forward`].
    pub fn store<T: Target>(self, target: T) -> Self {
        {
            let mut state = self.state.borrow_mut();
            state.check_last(self.stmt + 1);
            let definition = store_definition(&mut state, target.into_target());
            let clause = match &mut state.stmts[self.stmt] {
                Stmt::Sections(clauses) => clauses.last_mut(),
                _ => None,
            };
            if let Some(SectionClause::Until { store, .. }) = clause {
                *store = Some(definition);
            } else {
                state.report(IssueKind::Unprintable, "`store` needs an `until` before it");
            }
        }
        self
    }

    /// The block of arms.
    pub fn body<R>(self, f: impl FnOnce(&mut Arms<D, O>) -> R) -> R {
        let mut state = self.state.borrow_mut();
        state.reopen(self.stmt + 1);
        let block = state.open_block();
        drop(state);
        let mut arms = Arms {
            state: Rc::clone(&self.state),
            block,
            record: self.record,
            marker: PhantomData,
        };
        let result = f(&mut arms);
        self.state.borrow_mut().close_block();
        result
    }
}

/// The block of a `sections` statement, which holds only arms.
#[derive(Debug)]
pub struct Arms<D = D0, O = Required> {
    state: Shared,
    block: usize,
    record: Option<usize>,
    marker: PhantomData<fn() -> (D, O)>,
}

impl<D, O> Arms<D, O> {
    /// An arm with this pattern.
    pub fn arm(&mut self, pattern: impl Into<Pattern>) -> Arm<D, O> {
        let stmt = self.state.borrow_mut().push(
            self.block,
            Stmt::Arm {
                pattern: pattern.into(),
                options: ArmOptions::default(),
            },
        );
        Arm {
            state: Rc::clone(&self.state),
            stmt,
            record: self.record,
            marker: PhantomData,
        }
    }
}

/// The guard of an arm of `sections`.
#[derive(Debug)]
pub struct Arm<D, O> {
    state: Shared,
    stmt: usize,
    record: Option<usize>,
    marker: PhantomData<fn() -> (D, O)>,
}

impl<D, O> Arm<D, O> {
    fn update(&self, change: impl FnOnce(&mut ArmOptions)) {
        let mut state = self.state.borrow_mut();
        state.check_last(self.stmt);
        if let Stmt::Arm { options, .. } = &mut state.stmts[self.stmt] {
            change(options);
        }
    }

    /// `optional`: the arm need not match, so what it defines may be absent.
    pub fn optional(self) -> Arm<D, Maybe> {
        self.update(|options| options.optional = true);
        Arm {
            state: self.state,
            stmt: self.stmt,
            record: self.record,
            marker: PhantomData,
        }
    }

    /// `last`: sections end after this arm.
    pub fn last(self) -> Self {
        self.update(|options| options.last = true);
        self
    }

    /// The arm's block.
    pub fn body<R>(self, f: impl FnOnce(&mut Scope<D, O>) -> R) -> R {
        self.update(|options| options.body = true);
        enter(&self.state, self.record, f)
    }
}

/// The guard of an `if`, holding what its branches returned.
#[derive(Debug)]
pub struct If<D, O, R> {
    state: Shared,
    close: usize,
    record: Option<usize>,
    block: usize,
    result: R,
    marker: PhantomData<fn() -> (D, O)>,
}

impl<D: Depth, O: Presence, R> If<D, O, R> {
    fn after(state: Shared, record: Option<usize>, block: usize, result: R) -> Self {
        let close = state.borrow().stmts.len() - 1;
        Self {
            state,
            close,
            record,
            block,
            result,
            marker: PhantomData,
        }
    }

    fn continuation(&self, stmt: Stmt) {
        let mut state = self.state.borrow_mut();
        state.check_last(self.close);
        state.push(self.block, stmt);
    }

    /// `else { ... }`.
    pub fn else_<R2>(self, f: impl FnOnce(&mut Scope<D, Maybe>) -> R2) -> (R, R2) {
        self.continuation(Stmt::Else);
        let otherwise = enter(&self.state, self.record, f);
        (self.result, otherwise)
    }

    /// `else if condition { ... }`.
    pub fn else_if<E: Operand, R2>(
        self,
        condition: E,
        f: impl FnOnce(&mut Scope<D, Maybe>) -> R2,
    ) -> If<D, O, (R, R2)> {
        self.continuation(Stmt::ElseIf(condition.into_expr().ops));
        let otherwise = enter(&self.state, self.record, f);
        If::after(
            self.state,
            self.record,
            self.block,
            (self.result, otherwise),
        )
    }

    /// `else if:type`: the else branch is a declaration of the name `if`, which
    /// the target must have.
    pub fn else_read_if<T: Target, Ty: IntoDeclared>(
        self,
        target: T,
        ty: Ty,
    ) -> (R, ReadOut<T, Ty, D, Maybe>) {
        let declared = ty.into_declared();
        let levels = Some(declared.dimensions.len());
        let mut state = self.state.borrow_mut();
        state.check_last(self.close);
        let definition = state.define(target.into_target(), Ty::Value::TAG, levels);
        if definition.name != Some("if") {
            state.report(
                IssueKind::Unprintable,
                "`else_read_if` declares the name `if`, which its target must have",
            );
        }
        state.push(
            self.block,
            Stmt::ElseDeclaration {
                definition,
                declared,
            },
        );
        let handle = T::handle(definition.name, state.id);
        drop(state);
        (self.result, handle)
    }

    /// The `if` has no more branches; returns what they returned.
    pub fn finish(self) -> R {
        self.result
    }
}

/// Builds a reader spec. It is the root [`Scope`], with the settings and
/// statements that only the top level allows.
#[derive(Debug)]
pub struct SpecBuilder {
    scope: Scope,
}

impl Deref for SpecBuilder {
    type Target = Scope;
    fn deref(&self) -> &Scope {
        &self.scope
    }
}

impl DerefMut for SpecBuilder {
    fn deref_mut(&mut self) -> &mut Scope {
        &mut self.scope
    }
}

impl SpecBuilder {
    /// A builder for a spec with the format name `format`.
    pub fn new(format: impl Into<String>) -> Self {
        let mut builder = Self::unnamed();
        builder.format(format);
        builder
    }

    /// A builder for a spec with no format name.
    pub fn unnamed() -> Self {
        Self {
            scope: Scope {
                state: Rc::new(RefCell::new(State::new())),
                block: ROOT_BLOCK,
                record: None,
                marker: PhantomData,
            },
        }
    }

    /// `format name`, a further name of this format.
    pub fn format(&mut self, name: impl Into<String>) {
        self.push(Stmt::Format(name.into()));
    }

    /// `comment "text"...`.
    pub fn comment(&mut self, markers: impl Strings) {
        self.push(Stmt::Comment(markers.into_strings()));
    }

    /// `stop "text"...`.
    pub fn stop(&mut self, markers: impl Strings) {
        self.push(Stmt::Stop(markers.into_strings()));
    }

    /// `tokens "characters"`.
    pub fn tokens(&mut self, characters: impl Into<String>) {
        self.push(Stmt::Tokens(characters.into()));
    }

    /// `separators "characters"`.
    pub fn separators(&mut self, characters: impl Into<String>) {
        self.push(Stmt::Separators(characters.into()));
    }

    /// `ignore trailing`.
    pub fn ignore_trailing(&mut self) {
        self.push(Stmt::IgnoreTrailing);
    }

    /// `instances { ... }`.
    pub fn instances<R>(&mut self, f: impl FnOnce(&mut Scope) -> R) -> R {
        self.push(Stmt::Instances);
        enter(&self.scope.state, None, f)
    }

    /// The text this builder emits, without parsing it.
    pub fn source(&self) -> Result<String, BuildError> {
        let state = self.scope.state.borrow();
        if let Some(issue) = &state.issue {
            return Err(issue.clone().into());
        }
        check_bindings(&state)?;
        Ok(print(&state)?)
    }

    /// Print the text and parse it with [`Spec::parse`]. The spec is an
    /// ordinary one, and [`Spec::source`] gives the text.
    pub fn build(self) -> Result<Spec, BuildError> {
        let text = self.source()?;
        Spec::parse(text.as_bytes(), ORIGIN)
            .map_err(|failure| BuildError::from_failure(failure, text))
    }
}
