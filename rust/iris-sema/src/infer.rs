//! Structured, conservative summaries. This module never reports validity errors.
use crate::environment::{Binding, Changes, Environment};
use crate::targets::{TargetSetId, TargetSets};
use crate::{model::*, syntax::*};
use gecode_iris_syntax::{SyntaxKind as K, SyntaxNode};
use std::collections::{BTreeSet, HashMap};

pub(crate) fn infer(model: &mut SemanticModel, index: &SyntaxIndex) {
    let mut references = HashMap::new();
    for (i, reference) in model.references.iter().enumerate() {
        references.entry(reference.name_range).or_insert(i);
    }
    let mut expressions = HashMap::new();
    for (i, expression) in model.expressions.iter().enumerate() {
        expressions.entry(expression.node.clone()).or_insert(i);
    }
    let type_kinds = model
        .named_types
        .iter()
        .map(|t| (t.definition, t.kind))
        .collect();
    let mut inference = Inference {
        references,
        expressions,
        targets: TargetSets::new(model.definitions.len()),
        type_kinds,
        model,
        index,
    };
    inference.run();
}
struct Inference<'a> {
    model: &'a mut SemanticModel,
    index: &'a SyntaxIndex,
    references: HashMap<gecode_iris_syntax::TextRange, usize>,
    expressions: HashMap<SyntaxNode, usize>,
    targets: TargetSets,
    type_kinds: HashMap<DefinitionId, ReadingKind>,
}
impl Inference<'_> {
    fn run(&mut self) {
        let root = self.model.scopes[0].node.clone();
        self.infer_block(&root, &mut Environment::default());
        // Fallback is the first syntactic definition, independent of executed facts.
        for r in 0..self.model.repeats.len() {
            for f in 0..self.model.repeats[r].fields.len() {
                let id = self.model.repeats[r].fields[f].definitions[0];
                let definition = self.model.definition(id);
                let fallback = fallback_facts(definition);
                let index = definition
                    .repeats
                    .iter()
                    .position(|id| id.0 == r)
                    .expect("repeat field");
                self.model.repeats[r].fields[f].fallback =
                    fallback.shifted((definition.repeats.len() - index - 1) as isize);
            }
        }
        // Reference targets can change during flow summaries; rebuild inverse links once.
        for definition in &mut self.model.definitions {
            definition.references.clear();
        }
        for (index, reference) in self.model.references.iter().enumerate() {
            for id in reference.resolution.definitions() {
                self.model.definitions[id.0]
                    .references
                    .push(ReferenceId(index));
            }
        }
    }
    fn infer_block(&mut self, container: &SyntaxNode, env: &mut Environment) {
        for statement in container.children() {
            self.infer_statement(&statement, env);
        }
    }
    fn infer_statement(&mut self, node: &SyntaxNode, env: &mut Environment) {
        match node.kind() {
            K::Repeat => return self.infer_repeat(node, env),
            K::If => return self.infer_branches(node, env),
            K::Instances => return self.infer_instances(node, env),
            K::Sections => return self.infer_sections(node, env),
            _ => {}
        }
        for pattern in node.children().filter(|n| n.kind() == K::Pattern) {
            self.infer_pattern(&pattern, env);
        }
        for type_node in node.children().filter(|n| n.kind() == K::Type) {
            self.infer_type_bounds(&type_node, env);
        }
        for child in node
            .children()
            .filter(|n| matches!(n.kind(), K::Dimension | K::Layout | K::Offset))
        {
            for expression in expressions(&child) {
                self.infer_expression(&expression, env);
            }
        }
        let result = expressions(node)
            .into_iter()
            .map(|e| self.infer_expression(&e, env))
            .last();
        if let Some(id) = self
            .index
            .definitions_for(node)
            .iter()
            .copied()
            .find(|id| self.model.definition(*id).is_ordinary())
        {
            let facts = match self.model.definition(id).kind {
                DefinitionKind::Let => result.unwrap_or_else(ValueFacts::unknown),
                DefinitionKind::Param => match result {
                    Some(mut facts) => {
                        facts.depths = facts.depths.join(&Possibilities::one(0));
                        facts
                    }
                    None => ValueFacts {
                        value_types: Possibilities::Known(vec![
                            ValueType::Int,
                            ValueType::Float,
                            ValueType::String,
                        ]),
                        depths: Possibilities::one(0),
                    },
                },
                _ => self.model.definition(id).facts.clone(),
            };
            self.model.definitions[id.0].facts = facts.clone();
            self.publish(id, facts, false, env);
        }
    }
    fn infer_repeat(&mut self, node: &SyntaxNode, env: &mut Environment) {
        for expression in expressions(node) {
            self.infer_expression(&expression, env);
        }
        let repeat = self.index.repeat_for(node).expect("repeat");
        let checkpoint = env.checkpoint();
        if let Some(body) = node.children().find(|n| n.kind() == K::Block) {
            self.infer_block(&body, env);
        }
        env.rollback(checkpoint);
        self.publish_store(node, env);
        for field in self.model.repeats[repeat].fields.clone() {
            let mut facts = None;
            for id in &field.definitions {
                let d = self.model.definition(*id);
                let p = d
                    .repeats
                    .iter()
                    .position(|r| r.0 == repeat)
                    .expect("field chain");
                let projected = collected_facts(d, d.repeats.len() - p);
                facts = Some(facts.map_or(projected.clone(), |f: ValueFacts| f.join(&projected)));
            }
            env.insert(
                field.name,
                Binding {
                    definitions: self.targets.insert(&field.definitions),
                    facts: facts.unwrap_or_else(ValueFacts::unknown),
                    possibly_unavailable: false,
                },
            );
        }
    }
    fn infer_branches(&mut self, node: &SyntaxNode, env: &mut Environment) {
        for expression in expressions(node) {
            self.infer_expression(&expression, env);
        }
        let mut branches = vec![];
        for body in node
            .children()
            .filter(|n| matches!(n.kind(), K::Block | K::If | K::Declaration))
        {
            let checkpoint = env.checkpoint();
            if body.kind() == K::Block {
                self.infer_block(&body, env);
            } else {
                self.infer_statement(&body, env);
            }
            branches.push(env.rollback(checkpoint));
        }
        if branches.len() < 2 {
            branches.push(Changes::new());
        }
        self.join_branches(env, &branches);
    }
    fn infer_instances(&mut self, node: &SyntaxNode, env: &mut Environment) {
        let checkpoint = env.checkpoint();
        if let Some(body) = node.children().find(|n| n.kind() == K::Block) {
            // A retained earlier instance can publish into the enclosing frame
            // before this pass starts (Specification §9.3).
            self.admit_possible_definitions(&body, env);
            self.infer_block(&body, env);
        }
        let changed = env.rollback(checkpoint);
        self.join_branches(env, &[Changes::new(), changed]);
        env.make_uncertain();
    }
    fn infer_sections(&mut self, node: &SyntaxNode, env: &mut Environment) {
        // Input determines both arm order and occurrence. Admit all possible
        // definitions while keeping their facts and availability uncertain.
        for arm in node.children().filter(|n| n.kind() == K::Arm) {
            self.admit_possible_definitions(&arm, env);
        }
        for arm in node.children().filter(|n| n.kind() == K::Arm) {
            let checkpoint = env.checkpoint();
            for pattern in arm.children().filter(|n| n.kind() == K::Pattern) {
                self.infer_pattern(&pattern, env);
            }
            for body in arm.children().filter(|n| n.kind() == K::Block) {
                self.infer_block(&body, env);
            }
            let changed = env.rollback(checkpoint);
            self.join_branches(env, &[Changes::new(), changed]);
        }
        self.publish_store(node, env);
    }
    fn admit_possible_definitions(&mut self, container: &SyntaxNode, env: &mut Environment) {
        for id in self.index.ordinary_definitions_in(container, self.model) {
            crate::work::record();
            let d = self.model.definition(id);
            let definitions = env
                .get(&d.name)
                .map_or(TargetSetId::EMPTY, |b| b.definitions);
            let definitions = self.targets.join(definitions, TargetSets::single(id));
            env.insert(
                d.name.clone(),
                Binding {
                    definitions,
                    facts: ValueFacts::unknown(),
                    possibly_unavailable: true,
                },
            );
        }
    }
    fn publish_store(&mut self, node: &SyntaxNode, env: &mut Environment) {
        if let Some(id) = self
            .index
            .definitions_for(node)
            .iter()
            .copied()
            .find(|id| self.model.definition(*id).kind == DefinitionKind::Store)
        {
            let facts = ValueFacts::known(ValueType::String, 0);
            self.model.definitions[id.0].facts = facts.clone();
            let stored = Binding {
                definitions: TargetSets::single(id),
                facts,
                possibly_unavailable: false,
            };
            let name = self.model.definition(id).name.clone();
            // Sections can finish at EOF or a last arm without storing a token.
            let binding = if node.kind() == K::Sections {
                self.join_bindings(env.get(&name), Some(&stored))
                    .expect("stored binding")
            } else {
                stored
            };
            env.insert(name, binding);
        }
    }
    fn publish(
        &mut self,
        id: DefinitionId,
        facts: ValueFacts,
        possibly_unavailable: bool,
        env: &mut Environment,
    ) {
        env.insert(
            self.model.definition(id).name.clone(),
            Binding {
                definitions: TargetSets::single(id),
                facts,
                possibly_unavailable,
            },
        );
    }
    fn join_branches(&mut self, env: &mut Environment, branches: &[Changes]) {
        let names: BTreeSet<_> = branches.iter().flat_map(|b| b.keys()).collect();
        for name in names {
            crate::work::record();
            let base = env.get(name);
            let mut binding = branches.first().and_then(|b| b.get(name).or(base)).cloned();
            for branch in branches.iter().skip(1) {
                binding = self.join_bindings(binding.as_ref(), branch.get(name).or(base));
            }
            if let Some(binding) = binding {
                env.insert(name.clone(), binding);
            }
        }
    }
    fn join_bindings(&mut self, a: Option<&Binding>, b: Option<&Binding>) -> Option<Binding> {
        crate::work::record();
        match (a, b) {
            (Some(a), Some(b)) => Some(Binding {
                definitions: self.targets.join(a.definitions, b.definitions),
                facts: a.facts.join(&b.facts),
                possibly_unavailable: a.possibly_unavailable || b.possibly_unavailable,
            }),
            (Some(binding), None) | (None, Some(binding)) => {
                let mut binding = binding.clone();
                binding.possibly_unavailable = true;
                Some(binding)
            }
            (None, None) => None,
        }
    }
    fn infer_pattern(&mut self, pattern: &SyntaxNode, env: &mut Environment) {
        for current in pattern.children().filter(|n| n.kind() == K::CurrentValue) {
            if let Some(token) = first_ident(&current) {
                self.infer_reference(token.text_range(), env);
            }
        }
        let captures: Vec<_> = pattern
            .children()
            .filter(|n| n.kind() == K::Capture)
            .collect();
        for capture in &captures {
            if let Some(id) = self.index.definition_for(capture) {
                self.publish(id, self.model.definition(id).facts.clone(), false, env);
            }
        }
        for capture in captures {
            for node in capture.children().filter(|n| n.kind() == K::Type) {
                self.infer_type_bounds(&node, env);
            }
        }
    }
    fn infer_type_bounds(&mut self, node: &SyntaxNode, env: &mut Environment) {
        for range in node.children().filter(|n| n.kind() == K::Range) {
            for expression in expressions(&range) {
                self.infer_expression(&expression, env);
            }
        }
    }
    fn infer_expression(&mut self, node: &SyntaxNode, env: &mut Environment) -> ValueFacts {
        let args = expressions(node);
        let facts = match node.kind() {
            K::Literal => {
                let token = tokens(node).into_iter().next().expect("literal");
                ValueFacts::known(
                    match token.kind() {
                        K::Float => ValueType::Float,
                        K::String => ValueType::String,
                        _ => ValueType::Int,
                    },
                    0,
                )
            }
            K::HostConstant => ValueFacts::known(ValueType::Int, 0),
            K::Name => {
                let token = first_ident(node).expect("name");
                self.infer_reference(token.text_range(), env)
            }
            K::Parenthesized => self.infer_expression(&args[0], env),
            K::Comprehension => {
                let domain = self.infer_expression(&args[0], env);
                let id = self.index.definition_for(node).expect("binding");
                let facts = domain.shifted(-1);
                self.model.definitions[id.0].facts = facts.clone();
                let checkpoint = env.checkpoint();
                self.publish(id, facts, false, env);
                self.infer_expression(&args[1], env);
                env.rollback(checkpoint);
                ValueFacts::known(ValueType::Int, 0)
            }
            K::IndexExpression => {
                let base = self.infer_expression(&args[0], env);
                self.infer_expression(&args[1], env);
                base.shifted(-1)
            }
            K::Field => {
                self.infer_expression(&args[0], env);
                let token = first_ident(node).expect("field");
                self.infer_reference(token.text_range(), env)
            }
            K::Unary => {
                let facts = self.infer_expression(&args[0], env);
                if tokens(node).first().is_some_and(|t| t.text() == "-") {
                    ValueFacts {
                        value_types: facts.value_types,
                        depths: Possibilities::one(0),
                    }
                } else {
                    ValueFacts::known(ValueType::Int, 0)
                }
            }
            _ => {
                for argument in args {
                    self.infer_expression(&argument, env);
                }
                ValueFacts::known(ValueType::Int, 0)
            }
        };
        crate::work::record();
        if let Some(&index) = self.expressions.get(node) {
            let info = &mut self.model.expressions[index];
            info.facts = if info.denotation == Denotation::Value {
                facts.clone()
            } else {
                ValueFacts::unknown()
            };
        }
        facts
    }
    fn infer_reference(
        &mut self,
        range: gecode_iris_syntax::TextRange,
        env: &Environment,
    ) -> ValueFacts {
        crate::work::record();
        let Some(&index) = self.references.get(&range) else {
            return ValueFacts::unknown();
        };
        let reference = self.model.references[index].clone();
        let mut resolution = reference.resolution.clone();
        let facts = match &mut resolution {
            Resolution::Value {
                definitions,
                possibly_unavailable,
                projection,
            } => {
                if projection.is_empty() {
                    if let Some(binding) = env.get(&reference.name) {
                        *definitions = self.targets.to_vec(binding.definitions);
                        *possibly_unavailable = binding.possibly_unavailable;
                        binding.facts.clone()
                    } else {
                        ValueFacts::unknown()
                    }
                } else {
                    *definitions = self.index.ordinary_definitions(&reference.name).to_vec();
                    joined_definition_facts(definitions, self.model, |_| 0)
                }
            }
            Resolution::Bound(id) => env.get(&reference.name).map_or_else(
                || self.model.definition(*id).facts.clone(),
                |b| b.facts.clone(),
            ),
            Resolution::Field {
                definitions,
                record,
            } => joined_definition_facts(definitions, self.model, |d| {
                d.repeats.len()
                    - d.repeats
                        .iter()
                        .position(|r| r == record)
                        .expect("field record")
                    - 1
            }),
            Resolution::HostConstant(_) => ValueFacts::known(ValueType::Int, 0),
            Resolution::NamedType(id) => self
                .type_kinds
                .get(id)
                .map_or_else(ValueFacts::unknown, |kind| kind.facts(0)),
            Resolution::Structural { .. } => ValueFacts::unknown(),
        };
        self.model.references[index].resolution = resolution;
        self.model.references[index].facts = facts.clone();
        facts
    }
}
fn joined_definition_facts(
    ids: &[DefinitionId],
    model: &SemanticModel,
    extra: impl Fn(&Definition) -> usize,
) -> ValueFacts {
    ids.iter()
        .map(|id| {
            let d = model.definition(*id);
            collected_facts(d, extra(d))
        })
        .reduce(|a, b| a.join(&b))
        .unwrap_or_else(ValueFacts::unknown)
}

pub(crate) fn fallback_facts(definition: &Definition) -> ValueFacts {
    if matches!(definition.kind, DefinitionKind::Let | DefinitionKind::Param) {
        return ValueFacts::known(ValueType::Int, 0);
    }
    let mut fallback = definition.facts.clone();
    if let Possibilities::Known(types) = &mut fallback.value_types
        && types.contains(&ValueType::Int)
        && types.contains(&ValueType::Float)
    {
        *types = vec![ValueType::Int];
    }
    fallback
}
fn collected_facts(definition: &Definition, levels: usize) -> ValueFacts {
    let actual = definition.facts.shifted(levels as isize);
    if levels == 0 {
        return actual;
    }
    // A repeat may be empty, so its syntactic fallback is also possible.
    actual.join(&fallback_facts(definition).shifted(levels as isize))
}
