//! The binding check: one walk over the statements, which knows the depth and
//! presence of each definition, so `.optional()` counts wherever it was called.
use gecode_iris_core::CallSite;
use std::collections::HashMap;

use crate::error::{Issue, IssueKind};
use crate::expr::ValueTag;
use crate::field::Shape;
use crate::pattern::{CaptureSpec, Piece};
use crate::statement::{
    Declaration, Definition, Mention, NameState, RepeatCount, SectionClause, State, Stmt,
    mention_declared,
};
use crate::types::TypeRef;

#[derive(Debug, Clone, Copy)]
struct Frame {
    /// Identifies this block. Open blocks are numbered in nesting order.
    block: usize,
    /// The repeats around this block.
    depth: usize,
    required: bool,
}

#[derive(Debug, Default, Clone, Copy)]
struct Bound {
    count: usize,
    required: bool,
    /// The block of a binding, which always precedes what follows it there.
    block: Option<usize>,
}

/// A repeat's `store`, which runs when the repeat ends.
#[derive(Debug, Clone, Copy)]
struct PendingStore {
    /// The repeat statement, which the store belongs to.
    stmt: usize,
    repeat_block: usize,
    store: Definition,
    frame: Frame,
}

/// A `type` statement, in the block whose end hides it.
#[derive(Debug, Clone, Copy)]
struct TypeDeclaration {
    stmt: usize,
    block: usize,
}

struct Walk<'a> {
    state: &'a State,
    /// The index of the statement being checked.
    current: usize,
    bound: HashMap<&'static str, Bound>,
    frames: Vec<Frame>,
    next_block: usize,
    /// The stores of the open repeats, innermost last.
    stores: Vec<PendingStore>,
    /// The declarations of each type name so far, latest last. Those of closed
    /// blocks are dropped when a use finds them.
    types: HashMap<&'static str, Vec<TypeDeclaration>>,
}

fn fail_at<T>(site: CallSite, message: String) -> Result<T, Issue> {
    Err(Issue::new(IssueKind::Binding, message, site))
}

fn is_open(frames: &[Frame], block: usize) -> bool {
    frames
        .binary_search_by_key(&block, |frame| frame.block)
        .is_ok()
}

/// Whether a definition certainly has the shape a forward handle promises. A
/// value type or depth that is not known agrees only with an open promise.
fn agrees(promised: &Shape, value: ValueTag, levels: Option<usize>) -> bool {
    let value_agrees = match promised.value {
        ValueTag::Dyn => true,
        ValueTag::Num => matches!(value, ValueTag::Int | ValueTag::Float | ValueTag::Num),
        promised => promised == value,
    };
    value_agrees && (promised.levels.is_none() || promised.levels == levels)
}

fn describe(value: ValueTag, levels: Option<usize>, open: &str) -> String {
    match levels {
        Some(levels) => format!("{} at depth {levels}", value.describe()),
        None => format!("{} at {open} depth", value.describe()),
    }
}

impl Walk<'_> {
    fn site(&self) -> CallSite {
        self.state.sites[self.current]
    }

    /// Fail the statement being checked.
    fn fail<T>(&self, message: String) -> Result<T, Issue> {
        fail_at(self.site(), message)
    }

    /// `frame` with the number of a block of its own.
    fn own_block(&mut self, frame: Frame) -> Frame {
        self.next_block += 1;
        Frame {
            block: self.next_block,
            ..frame
        }
    }

    /// Open a block and return its number.
    fn enter(&mut self, frame: Frame) -> usize {
        let frame = self.own_block(frame);
        self.frames.push(frame);
        frame.block
    }

    fn record_binding(&mut self, name: &'static str, frame: Frame) -> usize {
        let precedes = self.binding_precedes(name);
        let bound = self.bound.entry(name).or_default();
        bound.count += 1;
        bound.required |= frame.required;
        if !precedes {
            bound.block = Some(frame.block);
        }
        bound.count
    }

    /// Whether a binding of `name` always executes before the current statement.
    fn binding_precedes(&self, name: &str) -> bool {
        self.bound
            .get(name)
            .and_then(|bound| bound.block)
            .is_some_and(|block| is_open(&self.frames, block))
    }

    /// A statement defines `name` in `frame`, with `levels` in all.
    fn bind(
        &mut self,
        name: &'static str,
        value: ValueTag,
        levels: Option<usize>,
        frame: Frame,
    ) -> Result<(), Issue> {
        match self.state.names.get(name) {
            Some(NameState::Undefined) => self.fail(format!(
                "`{name}` comes from `undefined`, so nothing may define it; \
                 use `forward` for a name that is defined"
            )),
            Some(NameState::Forward(shape)) => {
                if agrees(shape, value, levels) {
                    self.record_binding(name, frame);
                    Ok(())
                } else if self.binding_precedes(name) {
                    // An explicit redefinition, which leaves the handle stale.
                    Ok(())
                } else {
                    self.fail(format!(
                        "the forward handle `{name}` is {}, but this definition is {}; \
                         declare it with the definition's type, or as a `DynField`",
                        describe(shape.value, shape.levels, "any"),
                        describe(value, levels, "an unknown"),
                    ))
                }
            }
            _ => Ok(()),
        }
    }

    /// A statement other than `store` defines `definition` in `frame`.
    fn definition(&mut self, definition: Definition, frame: Frame) -> Result<(), Issue> {
        let Some(name) = definition.name else {
            return Ok(());
        };
        if name == "_" {
            return self.fail(
                "only `store` defines the entry named `_`: a declaration, a list or a \
                 capture discards it, and `let` and `param` reject it"
                    .to_owned(),
            );
        }
        let levels = definition.levels.map(|levels| frame.depth + levels);
        self.bind(name, definition.value, levels, frame)
    }

    /// A `store` defines `store` in `frame`. No repeat collects `_` into a
    /// column, so inside one it lasts for the iteration and leaves no entry.
    fn store(&mut self, store: Definition, frame: Frame) -> Result<(), Issue> {
        let Some(name) = store.name else {
            return Ok(());
        };
        if name == "_" && frame.depth > 0 {
            let local = Frame {
                required: false,
                ..frame
            };
            return self.bind(name, store.value, store.levels, local);
        }
        let levels = store.levels.map(|levels| frame.depth + levels);
        self.bind(name, store.value, levels, frame)
    }

    fn pattern(&mut self, pieces: &[Piece], frame: Frame) -> Result<(), Issue> {
        for piece in pieces {
            match piece {
                Piece::Capture(name) => self.capture(name.text, frame)?,
                Piece::Bind { name, value, .. } => self.definition(
                    Definition {
                        name: Some(name.text),
                        value: *value,
                        levels: Some(0),
                    },
                    frame,
                )?,
                _ => {}
            }
        }
        Ok(())
    }

    /// A pattern binds the capture `name` in `frame`.
    fn capture(&mut self, name: &'static str, frame: Frame) -> Result<(), Issue> {
        let state = self.state;
        let Some(NameState::Capture { spec, shape }) = state.names.get(name) else {
            return self.fail(format!(
                "`{name}` is not a capture; use `bind` to define it in a pattern, \
                 or `reference` to match its value"
            ));
        };
        if self.record_binding(name, frame) > 1 {
            return self.fail(format!(
                "the capture `{name}` is bound twice; a capture belongs to one pattern, \
                 so use `reference` to match its value again"
            ));
        }
        if shape.levels.is_some_and(|levels| levels != frame.depth) {
            return self.fail(format!(
                "the capture `{name}` is bound in a different scope: create it with \
                 `capture` in the scope of the pattern"
            ));
        }
        if shape.required != frame.required {
            let form = if frame.required {
                "capture"
            } else {
                "optional_capture"
            };
            return self.fail(format!(
                "the capture `{name}` is bound where its presence differs: use `{form}`"
            ));
        }
        if let CaptureSpec::Typed(declared) = spec {
            let mut result = Ok(());
            mention_declared(declared, &mut |mention| {
                self.mention(mention, &mut result);
            });
            result?;
        }
        Ok(())
    }

    fn mention(&mut self, mention: Mention<'_>, result: &mut Result<(), Issue>) {
        if let Mention::Type(named) = mention
            && result.is_ok()
        {
            *result = self.type_use(named);
        }
    }

    /// A statement uses the named type `named`, which must be the one visible.
    fn type_use(&mut self, named: TypeRef) -> Result<(), Issue> {
        let site = self.site();
        let declarations = self.types.entry(named.name.text).or_default();
        while declarations
            .last()
            .is_some_and(|declaration| !is_open(&self.frames, declaration.block))
        {
            declarations.pop();
        }
        match declarations.last() {
            Some(visible) if visible.stmt == named.declaration => Ok(()),
            _ => Err(Issue::new(
                IssueKind::Scope,
                format!(
                    "the declaration of the type `{}` that this handle came from \
                     is not the one visible here",
                    named.name.text
                ),
                site,
            )),
        }
    }

    fn finish(&self) -> Result<(), Issue> {
        for &Declaration { name, site } in &self.state.declared {
            let bound = self.bound.get(name).copied().unwrap_or_default();
            match &self.state.names[name] {
                NameState::Capture { .. } if bound.count == 0 => {
                    return fail_at(
                        site,
                        format!("the capture `{name}` is never bound; add it to a pattern"),
                    );
                }
                NameState::Forward(_) if bound.count == 0 => {
                    return fail_at(
                        site,
                        format!(
                            "the forward handle `{name}` is never bound; define it, \
                             or use `undefined` for a name left undefined"
                        ),
                    );
                }
                NameState::Forward(shape) if shape.required && !bound.required => {
                    return fail_at(
                        site,
                        format!(
                            "the forward handle `{name}` is required, but every definition \
                             may not execute; declare it with `Maybe`"
                        ),
                    );
                }
                _ => {}
            }
        }
        for record in &self.state.records {
            if let (Some(site), false) = (record.forward, record.bound) {
                let name = record.each.unwrap_or_default();
                return fail_at(
                    site,
                    format!(
                        "the forward record `{name}` is never bound; \
                         use `record_as` in its repeat"
                    ),
                );
            }
        }
        Ok(())
    }

    /// Record what `stmt`, at `index`, defines in `frame`.
    fn record_definitions(&mut self, index: usize, stmt: &Stmt, frame: Frame) -> Result<(), Issue> {
        match stmt {
            Stmt::Param { definition, .. }
            | Stmt::Declaration { definition, .. }
            | Stmt::List { definition, .. }
            | Stmt::Let { definition, .. } => self.definition(*definition, frame)?,
            Stmt::ElseDeclaration { definition, .. } => {
                let branch = self.own_block(Frame {
                    required: false,
                    ..frame
                });
                self.definition(*definition, branch)?;
            }
            Stmt::TypeDefinition { name, .. } => {
                let declaration = TypeDeclaration {
                    stmt: index,
                    block: frame.block,
                };
                self.types.entry(name).or_default().push(declaration);
            }
            Stmt::Line(pattern) | Stmt::Find(pattern) => self.pattern(&pattern.pieces, frame)?,
            Stmt::Sections(clauses) => {
                // End of file and `last` end sections without storing.
                let optional = self.own_block(Frame {
                    required: false,
                    ..frame
                });
                for clause in clauses {
                    if let SectionClause::Until {
                        store: Some(store), ..
                    } = clause
                    {
                        self.store(*store, optional)?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Open or close the block `stmt` starts or ends, and check an arm's
    /// pattern in its own block.
    fn follow_blocks(&mut self, stmt: &Stmt, frame: Frame) -> Result<(), Issue> {
        match stmt {
            Stmt::Close => {
                let closed = self.frames.pop().expect("a close ends an open block");
                let ended = |pending: &mut PendingStore| pending.repeat_block == closed.block;
                if let Some(pending) = self.stores.pop_if(ended) {
                    self.current = pending.stmt;
                    self.store(pending.store, pending.frame)?;
                }
            }
            Stmt::Repeat { count, .. } => {
                let repeat_block = self.enter(Frame {
                    depth: frame.depth + 1,
                    ..frame
                });
                if let RepeatCount::Until {
                    store: Some(store), ..
                } = count
                {
                    self.stores.push(PendingStore {
                        stmt: self.current,
                        repeat_block,
                        store: *store,
                        frame,
                    });
                }
            }
            Stmt::If(_) | Stmt::ElseIf(_) | Stmt::Else => {
                self.enter(Frame {
                    required: false,
                    ..frame
                });
            }
            Stmt::Arm { pattern, options } => {
                let arm = self.own_block(Frame {
                    required: frame.required && !options.optional,
                    ..frame
                });
                self.pattern(&pattern.pieces, arm)?;
                if options.body {
                    self.frames.push(arm);
                }
            }
            opener if opener.opens_block() => {
                self.enter(frame);
            }
            _ => {}
        }
        Ok(())
    }
}

/// Check every capture, forward handle and named type against how it was
/// declared.
pub(crate) fn check_bindings(state: &State) -> Result<(), Issue> {
    let mut walk = Walk {
        state,
        current: 0,
        bound: HashMap::new(),
        frames: vec![Frame {
            block: 0,
            depth: 0,
            required: true,
        }],
        next_block: 0,
        stores: Vec::new(),
        types: HashMap::new(),
    };
    for (index, stmt) in state.stmts.iter().enumerate() {
        walk.current = index;
        let frame = *walk.frames.last().expect("the root frame stays open");
        let mut result = Ok(());
        stmt.mentions(&mut |mention| walk.mention(mention, &mut result));
        result?;
        walk.record_definitions(index, stmt, frame)?;
        walk.follow_blocks(stmt, frame)?;
    }
    walk.finish()
}
