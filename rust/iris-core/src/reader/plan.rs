//! Immutable block membership and statement setup, prepared once per spec.
use super::*;
use gecode_iris_sema::{Possibilities, ValueType};

#[derive(Debug, Clone, Copy)]
pub(super) struct StatementId(usize);
#[derive(Debug, Clone, Copy)]
pub(super) struct ListId(pub usize);
#[derive(Debug, Clone, Copy)]
pub(super) struct Site {
    pub line: usize,
    pub range: TextRange,
}
#[derive(Debug)]
pub(super) struct Statement {
    pub site: Site,
    pub operation: Operation,
}
#[derive(Debug)]
pub(super) enum Operation {
    Block(Box<[StatementId]>),
    List(ListId),
    Repeat(Repeat),
    If {
        condition: prepared::Bound,
        yes: StatementId,
        no: Option<StatementId>,
    },
    Generic(SyntaxNode),
}
#[derive(Debug)]
pub(super) struct Repeat {
    pub id: RepeatId,
    pub body: StatementId,
    pub single_list: Option<StatementId>,
    pub mode: RepeatMode,
    pub fields: Box<[Field]>,
}
#[derive(Debug)]
pub(super) struct Field {
    pub name: Name,
    pub kind: &'static str,
    pub depth: usize,
}
#[derive(Debug)]
pub(super) enum RepeatMode {
    Count(prepared::Bound),
    Until {
        alternatives: Box<[String]>,
        store: Option<Name>,
    },
}
#[derive(Debug, Default)]
pub(super) struct ExecutionPlan {
    root: Option<StatementId>,
    statements: Vec<Statement>,
    ids: HashMap<TextRange, StatementId>,
}
impl ExecutionPlan {
    pub fn new(root: &SyntaxNode, index: &ModelIndex, model: &SemanticModel) -> Self {
        // Reverse traversal makes every child available before its parent,
        // without recursive preparation of nested blocks or conditionals.
        let source = root.text().to_string();
        let mut nodes = Vec::new();
        let mut offset = 0;
        let mut line = 1;
        for node in root.descendants() {
            let start = usize::from(node.text_range().start());
            for &byte in &source.as_bytes()[offset..start] {
                line += usize::from(byte == b'\n');
            }
            offset = start;
            let site = Site {
                line,
                range: node.text_range(),
            };
            nodes.push((node, site));
        }
        let mut plan = Self::default();
        for (node, site) in nodes.into_iter().rev() {
            if node != *root && !is_statement(node.kind()) && node.kind() != K::Block {
                continue;
            }
            let operation = match node.kind() {
                K::Block => Operation::Block(node.children().map(|n| plan.id(&n)).collect()),
                K::List => Operation::List(index.list_ids[&node.text_range()]),
                K::Repeat => {
                    let body_node = child(&node, K::Block);
                    let body = plan.id(&body_node);
                    let single_list = match &plan.statement(body).operation {
                        Operation::Block(statements)
                            if statements.len() == 1
                                && matches!(
                                    plan.statement(statements[0]).operation,
                                    Operation::List(_)
                                ) =>
                        {
                            Some(statements[0])
                        }
                        _ => None,
                    };
                    let id = index.repeats[&node.text_range()];
                    let mode = RepeatMode::new(&node, index, model);
                    let fields = repeat_fields(id, index, model);
                    Operation::Repeat(Repeat {
                        id,
                        body,
                        single_list,
                        mode,
                        fields,
                    })
                }
                K::If => {
                    let mut bodies = node.children().filter(|n| !is_expression(n.kind()));
                    Operation::If {
                        condition: prepared::Bound::new(
                            expression(&node),
                            &index.names,
                            &index.references,
                            model,
                        ),
                        yes: plan.id(&bodies.next().expect("if body")),
                        no: bodies.next().map(|n| plan.id(&n)),
                    }
                }
                _ if node == *root => {
                    Operation::Block(node.children().map(|n| plan.id(&n)).collect())
                }
                _ => Operation::Generic(node.clone()),
            };
            if node == *root {
                plan.root = Some(StatementId(plan.statements.len()));
            } else {
                plan.ids
                    .insert(node.text_range(), StatementId(plan.statements.len()));
            }
            plan.statements.push(Statement { site, operation });
        }
        plan
    }
    pub fn id(&self, node: &SyntaxNode) -> StatementId {
        if node.parent().is_none() {
            self.root.expect("prepared root")
        } else {
            self.ids[&node.text_range()]
        }
    }
    pub fn statement(&self, id: StatementId) -> &Statement {
        &self.statements[id.0]
    }
}
impl RepeatMode {
    fn new(node: &SyntaxNode, index: &ModelIndex, model: &SemanticModel) -> Self {
        if !tokens(node).get(1).is_some_and(|t| t.text() == "until") {
            return Self::Count(prepared::Bound::new(
                expression(node),
                &index.names,
                &index.references,
                model,
            ));
        }
        let terminators = node.children().find(|n| n.kind() == K::Terminators);
        let parts = terminators.as_ref().map(tokens).unwrap_or_default();
        Self::Until {
            alternatives: parts
                .iter()
                .filter(|t| t.kind() == K::String)
                .map(|t| index.string_literals[&t.text_range()].clone())
                .collect(),
            store: parts
                .iter()
                .position(|t| t.text() == "store")
                .map(|i| index.names[&parts[i + 1].text_range()].clone()),
        }
    }
}
fn repeat_fields(id: RepeatId, index: &ModelIndex, model: &SemanticModel) -> Box<[Field]> {
    model.repeats()[id.0]
        .fields
        .iter()
        .zip(&index.repeat_names[id.0])
        .map(|(field, name)| {
            let Possibilities::Known(depths) = &field.fallback.depths else {
                unreachable!("validated field depth")
            };
            let Possibilities::Known(types) = &field.fallback.value_types else {
                unreachable!("validated field type")
            };
            Field {
                name: name.clone(),
                depth: depths[0],
                kind: match types[0] {
                    ValueType::Int => "int",
                    ValueType::Float => "float",
                    ValueType::String => "string",
                },
            }
        })
        .collect()
}
fn is_statement(kind: K) -> bool {
    matches!(
        kind,
        K::Format
            | K::Setting
            | K::Declaration
            | K::TypeDefinition
            | K::Let
            | K::List
            | K::Each
            | K::Index
            | K::Line
            | K::Find
            | K::Skip
            | K::Expect
            | K::Check
            | K::Param
            | K::Instances
            | K::Repeat
            | K::Sections
            | K::If
    )
}

impl Interpreter<'_, '_> {
    pub(super) fn select_site(&mut self, site: Site) {
        self.statement = Some((site.line, site.range));
    }
    pub(super) fn execute_plan(&mut self, root: StatementId) -> Result<(), Failure> {
        let mut work = vec![Work::Statement(root)];
        while let Some(next) = work.pop() {
            self.input.control.check()?;
            self.work(WorkKind::Dispatch, 1);
            match next {
                Work::Sequence(block, next) => {
                    let Operation::Block(statements) =
                        &self.spec.index.plan.statement(block).operation
                    else {
                        unreachable!("block continuation")
                    };
                    if let Some(&statement) = statements.get(next) {
                        work.push(Work::Sequence(block, next + 1));
                        work.push(Work::Statement(statement));
                    }
                }
                Work::RepeatNext => {
                    if self.begin_iteration()? {
                        work.push(Work::AfterBody);
                        work.push(Work::Statement(
                            self.repeats.last().expect("active repeat").body,
                        ));
                    } else {
                        self.finish_repeat()?;
                    }
                }
                Work::AfterBody => {
                    self.collect_iteration(None)?;
                    work.push(Work::RepeatNext);
                }
                Work::SectionsNext => self.next_section(&mut work)?,
                Work::AfterSectionBody => self.after_section_body(&mut work)?,
                Work::Statement(id) => {
                    let statement = self.spec.index.plan.statement(id);
                    if !matches!(statement.operation, Operation::Block(_)) {
                        self.select_site(statement.site);
                    }
                    match &statement.operation {
                        Operation::Block(_) => work.push(Work::Sequence(id, 0)),
                        Operation::List(list) => {
                            self.execute_list_descriptor(&self.spec.index.lists[list.0], None)?
                        }
                        Operation::Repeat(repeat) => {
                            self.begin_prepared_repeat(repeat, statement.site)?;
                            if let Some(list) = repeat.single_list {
                                self.execute_list_repeat(list)?;
                            } else {
                                work.push(Work::RepeatNext);
                            }
                        }
                        Operation::If { condition, yes, no } => {
                            let condition = self.eval(&condition.node)?;
                            if let Some(body) = if truth(&condition) { Some(*yes) } else { *no } {
                                work.push(Work::Statement(body));
                            }
                        }
                        Operation::Generic(node) if node.kind() == K::Sections => {
                            self.begin_sections(node);
                            work.push(Work::SectionsNext);
                        }
                        Operation::Generic(node) => self.execute_statement(node)?,
                    }
                }
            }
        }
        Ok(())
    }
    fn execute_list_repeat(&mut self, id: StatementId) -> Result<(), Failure> {
        let statement = self.spec.index.plan.statement(id);
        let Operation::List(list) = statement.operation else {
            unreachable!("single-list repeat")
        };
        let mut bindings = prepared::ListBindings::default();
        loop {
            self.input.control.check()?;
            self.work(WorkKind::Dispatch, 1);
            if !self.begin_iteration()? {
                break;
            }
            self.input.control.check()?;
            self.work(WorkKind::Dispatch, 1);
            self.select_site(statement.site);
            self.execute_list_descriptor(&self.spec.index.lists[list.0], Some(&mut bindings))?;
            self.input.control.check()?;
            self.work(WorkKind::Dispatch, 1);
            self.collect_iteration(bindings.output)?;
        }
        self.finish_repeat()
    }
}
