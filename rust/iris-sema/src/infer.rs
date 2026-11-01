//! Structured, conservative summaries. This module never reports validity errors.
use crate::{model::*, syntax::*};
use iris_syntax::{SyntaxKind as K, SyntaxNode};
use std::collections::BTreeMap;

#[derive(Clone)]
struct Binding {
    definitions: Vec<DefinitionId>,
    facts: ValueFacts,
    possibly_unavailable: bool,
}
type Environment = BTreeMap<String, Binding>;

pub(crate) fn infer(model: &mut SemanticModel) {
    let root = model.scopes[0].node.clone();
    infer_block(&root, &mut Environment::new(), model);
    // Fallback is the first syntactic definition, independent of executed facts.
    for r in 0..model.repeats.len() {
        for f in 0..model.repeats[r].fields.len() {
            let id = model.repeats[r].fields[f].definitions[0];
            let definition = model.definition(id);
            let fallback = fallback_facts(definition);
            let index = definition
                .repeats
                .iter()
                .position(|id| id.0 == r)
                .expect("repeat field");
            model.repeats[r].fields[f].fallback =
                fallback.shifted((definition.repeats.len() - index - 1) as isize);
        }
    }
    // Reference targets can change during flow summaries; rebuild inverse links once.
    for definition in &mut model.definitions {
        definition.references.clear();
    }
    for (index, reference) in model.references.iter().enumerate() {
        for id in reference.resolution.definitions() {
            model.definitions[id.0].references.push(ReferenceId(index));
        }
    }
}
fn infer_block(container: &SyntaxNode, env: &mut Environment, model: &mut SemanticModel) {
    for statement in container.children() {
        infer_statement(&statement, env, model);
    }
}
fn infer_statement(node: &SyntaxNode, env: &mut Environment, model: &mut SemanticModel) {
    match node.kind() {
        K::Repeat => return infer_repeat(node, env, model),
        K::If => return infer_branches(node, env, model),
        K::Instances => return infer_instances(node, env, model),
        K::Sections => return infer_sections(node, env, model),
        _ => {}
    }
    for pattern in node.children().filter(|n| n.kind() == K::Pattern) {
        infer_pattern(&pattern, env, model);
    }
    for type_node in node.children().filter(|n| n.kind() == K::Type) {
        infer_type_bounds(&type_node, env, model);
    }
    for child in node
        .children()
        .filter(|n| matches!(n.kind(), K::Dimension | K::Layout | K::Offset))
    {
        for expression in expressions(&child) {
            infer_expression(&expression, env, model);
        }
    }
    let result = expressions(node)
        .into_iter()
        .map(|e| infer_expression(&e, env, model))
        .last();
    if let Some(id) = model
        .definitions
        .iter()
        .position(|d| d.node == *node && d.is_ordinary())
        .map(DefinitionId)
    {
        let facts = match model.definition(id).kind {
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
            _ => model.definition(id).facts.clone(),
        };
        model.definitions[id.0].facts = facts.clone();
        publish(id, facts, false, env, model);
    }
}
fn infer_repeat(node: &SyntaxNode, env: &mut Environment, model: &mut SemanticModel) {
    for expression in expressions(node) {
        infer_expression(&expression, env, model);
    }
    let repeat = model
        .repeats
        .iter()
        .position(|r| r.node == *node)
        .expect("repeat");
    let mut local = env.clone();
    if let Some(body) = node.children().find(|n| n.kind() == K::Block) {
        infer_block(&body, &mut local, model);
    }
    publish_store(node, env, model);
    for field in model.repeats[repeat].fields.clone() {
        let mut facts = None;
        for id in &field.definitions {
            let d = model.definition(*id);
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
                definitions: field.definitions,
                facts: facts.unwrap_or_else(ValueFacts::unknown),
                possibly_unavailable: false,
            },
        );
    }
}
fn infer_branches(node: &SyntaxNode, env: &mut Environment, model: &mut SemanticModel) {
    for expression in expressions(node) {
        infer_expression(&expression, env, model);
    }
    let mut branches = vec![];
    for body in node
        .children()
        .filter(|n| matches!(n.kind(), K::Block | K::If | K::Declaration))
    {
        let mut branch = env.clone();
        if body.kind() == K::Block {
            infer_block(&body, &mut branch, model);
        } else {
            infer_statement(&body, &mut branch, model);
        }
        branches.push(branch);
    }
    if branches.len() < 2 {
        branches.push(env.clone());
    }
    if let Some(first) = branches.first() {
        *env = branches
            .iter()
            .skip(1)
            .fold(first.clone(), |a, b| join(&a, b));
    }
}
fn infer_instances(node: &SyntaxNode, env: &mut Environment, model: &mut SemanticModel) {
    let mut local = env.clone();
    if let Some(body) = node.children().find(|n| n.kind() == K::Block) {
        // A retained earlier instance can publish into the enclosing frame
        // before this pass starts (Specification §9.3).
        admit_possible_definitions(&body, &mut local, model);
        infer_block(&body, &mut local, model);
    }
    *env = join(env, &local);
    for binding in env.values_mut() {
        binding.facts = ValueFacts::unknown();
        binding.possibly_unavailable = true;
    }
}
fn infer_sections(node: &SyntaxNode, env: &mut Environment, model: &mut SemanticModel) {
    // Input determines both arm order and occurrence. Admit all possible
    // definitions while keeping their facts and availability uncertain.
    let mut summary = env.clone();
    for arm in node.children().filter(|n| n.kind() == K::Arm) {
        admit_possible_definitions(&arm, &mut summary, model);
    }
    for arm in node.children().filter(|n| n.kind() == K::Arm) {
        let mut branch = summary.clone();
        for pattern in arm.children().filter(|n| n.kind() == K::Pattern) {
            infer_pattern(&pattern, &mut branch, model);
        }
        for body in arm.children().filter(|n| n.kind() == K::Block) {
            infer_block(&body, &mut branch, model);
        }
        summary = join(&summary, &branch);
    }
    *env = summary;
    publish_store(node, env, model);
}

fn admit_possible_definitions(
    container: &SyntaxNode,
    env: &mut Environment,
    model: &SemanticModel,
) {
    for (id, d) in model.definitions.iter().enumerate().filter(|(_, d)| {
        d.is_ordinary() && container.text_range().contains_range(d.node.text_range())
    }) {
        let binding = env.entry(d.name.clone()).or_insert(Binding {
            definitions: vec![],
            facts: ValueFacts::unknown(),
            possibly_unavailable: true,
        });
        if !binding.definitions.contains(&DefinitionId(id)) {
            binding.definitions.push(DefinitionId(id));
        }
        binding.facts = ValueFacts::unknown();
        binding.possibly_unavailable = true;
    }
}

fn publish_store(node: &SyntaxNode, env: &mut Environment, model: &mut SemanticModel) {
    if let Some(id) = model
        .definitions
        .iter()
        .position(|d| d.node == *node && d.kind == DefinitionKind::Store)
        .map(DefinitionId)
    {
        let facts = ValueFacts::known(ValueType::String, 0);
        model.definitions[id.0].facts = facts.clone();
        let mut stored = env.clone();
        publish(id, facts, false, &mut stored, model);
        // Sections can finish at EOF or a last arm without storing a token.
        *env = if node.kind() == K::Sections {
            join(env, &stored)
        } else {
            stored
        };
    }
}
fn publish(
    id: DefinitionId,
    facts: ValueFacts,
    possibly_unavailable: bool,
    env: &mut Environment,
    model: &SemanticModel,
) {
    env.insert(
        model.definition(id).name.clone(),
        Binding {
            definitions: vec![id],
            facts,
            possibly_unavailable,
        },
    );
}
fn join(a: &Environment, b: &Environment) -> Environment {
    let mut result = a.clone();
    for (name, binding) in &mut result {
        if let Some(other) = b.get(name) {
            for id in &other.definitions {
                if !binding.definitions.contains(id) {
                    binding.definitions.push(*id);
                }
            }
            binding.facts = binding.facts.join(&other.facts);
            binding.possibly_unavailable |= other.possibly_unavailable;
        } else {
            binding.possibly_unavailable = true;
        }
    }
    for (name, binding) in b {
        if !result.contains_key(name) {
            let mut binding = binding.clone();
            binding.possibly_unavailable = true;
            result.insert(name.clone(), binding);
        }
    }
    result
}
fn infer_pattern(pattern: &SyntaxNode, env: &mut Environment, model: &mut SemanticModel) {
    for current in pattern.children().filter(|n| n.kind() == K::CurrentValue) {
        if let Some(token) = first_ident(&current) {
            infer_reference(token.text_range(), env, model);
        }
    }
    let captures: Vec<_> = pattern
        .children()
        .filter(|n| n.kind() == K::Capture)
        .collect();
    for capture in &captures {
        if let Some(id) = definition_for(model, capture) {
            publish(id, model.definition(id).facts.clone(), false, env, model);
        }
    }
    for capture in captures {
        for node in capture.children().filter(|n| n.kind() == K::Type) {
            infer_type_bounds(&node, env, model);
        }
    }
}
fn infer_type_bounds(node: &SyntaxNode, env: &Environment, model: &mut SemanticModel) {
    for range in node.children().filter(|n| n.kind() == K::Range) {
        for expression in expressions(&range) {
            infer_expression(&expression, env, model);
        }
    }
}
fn infer_expression(node: &SyntaxNode, env: &Environment, model: &mut SemanticModel) -> ValueFacts {
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
            infer_reference(token.text_range(), env, model)
        }
        K::Parenthesized => infer_expression(&args[0], env, model),
        K::Comprehension => {
            let domain = infer_expression(&args[0], env, model);
            let id = definition_for(model, node).expect("binding");
            let facts = domain.shifted(-1);
            model.definitions[id.0].facts = facts.clone();
            let mut local = env.clone();
            publish(id, facts, false, &mut local, model);
            infer_expression(&args[1], &local, model);
            ValueFacts::known(ValueType::Int, 0)
        }
        K::IndexExpression => {
            let base = infer_expression(&args[0], env, model);
            infer_expression(&args[1], env, model);
            base.shifted(-1)
        }
        K::Field => {
            infer_expression(&args[0], env, model);
            let token = first_ident(node).expect("field");
            infer_reference(token.text_range(), env, model)
        }
        K::Unary => {
            let facts = infer_expression(&args[0], env, model);
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
                infer_expression(&argument, env, model);
            }
            ValueFacts::known(ValueType::Int, 0)
        }
    };
    if let Some(info) = model.expressions.iter_mut().find(|e| e.node == *node) {
        info.facts = if info.denotation == Denotation::Value {
            facts.clone()
        } else {
            ValueFacts::unknown()
        };
    }
    facts
}
fn infer_reference(
    range: iris_syntax::TextRange,
    env: &Environment,
    model: &mut SemanticModel,
) -> ValueFacts {
    let Some(index) = model.references.iter().position(|r| r.name_range == range) else {
        return ValueFacts::unknown();
    };
    let reference = model.references[index].clone();
    let mut resolution = reference.resolution.clone();
    let facts = match &mut resolution {
        Resolution::Value {
            definitions,
            possibly_unavailable,
            projection,
        } => {
            if projection.is_empty() {
                if let Some(binding) = env.get(&reference.name) {
                    *definitions = binding.definitions.clone();
                    *possibly_unavailable = binding.possibly_unavailable;
                    binding.facts.clone()
                } else {
                    ValueFacts::unknown()
                }
            } else {
                *definitions = model
                    .definitions
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.is_ordinary() && d.name == reference.name)
                    .map(|(i, _)| DefinitionId(i))
                    .collect();
                joined_definition_facts(definitions, model, |_| 0)
            }
        }
        Resolution::Bound(id) => env
            .get(&reference.name)
            .map_or_else(|| model.definition(*id).facts.clone(), |b| b.facts.clone()),
        Resolution::Field {
            definitions,
            record,
        } => joined_definition_facts(definitions, model, |d| {
            d.repeats.len()
                - d.repeats
                    .iter()
                    .position(|r| r == record)
                    .expect("field record")
                - 1
        }),
        Resolution::HostConstant(_) => ValueFacts::known(ValueType::Int, 0),
        Resolution::NamedType(id) => model
            .named_types
            .iter()
            .find(|t| t.definition == *id)
            .map_or_else(ValueFacts::unknown, |t| t.kind.facts(0)),
        Resolution::Structural { .. } => ValueFacts::unknown(),
    };
    model.references[index].resolution = resolution;
    model.references[index].facts = facts.clone();
    facts
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

fn fallback_facts(definition: &Definition) -> ValueFacts {
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
