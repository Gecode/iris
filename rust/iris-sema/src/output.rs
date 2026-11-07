//! The entries that a successful read leaves in its data.
//!
//! The final flow environment cannot answer this: sections and instances keep
//! its facts uncertain for lookups, whatever their definitions say. A read's
//! final value of a name comes from one of its definitions, so an entry joins
//! the facts of all of them instead. Presence depends only on where they are.
use crate::infer::fallback_facts;
use crate::{model::*, syntax::*};
use gecode_iris_syntax::{SyntaxKind as K, SyntaxNode, WalkEvent};
use std::collections::HashMap;

pub(crate) fn collect(model: &mut SemanticModel, index: &SyntaxIndex) {
    let required = definitions_required_in_output(model, index);
    let mut positions = HashMap::new();
    let mut entries: Vec<OutputEntry> = vec![];
    for (position, definition) in model.definitions.iter().enumerate() {
        crate::work::record();
        if !reaches_output(definition) {
            continue;
        }
        let id = DefinitionId(position);
        let facts = entry_facts(definition);
        let possibly_absent = !required[position];
        match positions.get(&definition.name) {
            Some(&entry) => {
                let entry: &mut OutputEntry = &mut entries[entry];
                entry.definitions.push(id);
                entry.facts = entry.facts.join(&facts);
                entry.possibly_absent &= possibly_absent;
            }
            None => {
                positions.insert(definition.name.clone(), entries.len());
                entries.push(OutputEntry {
                    name: definition.name.clone(),
                    definitions: vec![id],
                    facts,
                    possibly_absent,
                });
            }
        }
    }
    model.output_entries = entries;
}

/// Whether a definition can leave an entry in the data. A `store _` keeps one
/// only outside every repeat: no column collects `_` (Specification §7.5).
fn reaches_output(definition: &Definition) -> bool {
    definition.is_ordinary()
        || (definition.kind == DefinitionKind::Store && definition.repeats.is_empty())
}

/// What the top level sees of one definition. A repeat that runs no iteration
/// leaves an empty column at the fallback depth, so that depth is possible too.
/// Such a column has no leaves, and no leaves match every value type, so the
/// fallback's value type does not count.
fn entry_facts(definition: &Definition) -> ValueFacts {
    let levels = definition.repeats.len() as isize;
    let actual = definition.facts.shifted(levels);
    if levels == 0 {
        return actual;
    }
    let fallback = fallback_facts(definition).shifted(levels);
    ValueFacts {
        value_types: actual.value_types,
        depths: actual.depths.join(&fallback.depths),
    }
}

/// For each definition, whether its entry is in the output of every successful
/// read. That says nothing about whether the definition runs.
///
/// An `if` body and an optional arm may not run, and sections may end without
/// their store. A required arm matches in every successful read, the retained
/// instance runs its whole body, and a repeat publishes a column for every
/// possible field, so `repeat 0 {if 0 {x:int}}` has an empty `x` column though
/// nothing defines it. Nothing inside a repeat that runs can lose its entry. A
/// walk with counters needs no recursion, however deep the spec nests.
fn definitions_required_in_output(model: &SemanticModel, index: &SyntaxIndex) -> Vec<bool> {
    let mut required = vec![false; model.definitions.len()];
    let mut repeats = 0_usize;
    let mut conditional_bodies = 0_usize;
    for event in model.scopes[0].node.preorder() {
        crate::work::record();
        match event {
            WalkEvent::Enter(node) => {
                for id in index.definitions_for(&node) {
                    let stored_by_sections = node.kind() == K::Sections && repeats == 0;
                    required[id.0] = conditional_bodies == 0 && !stored_by_sections;
                }
                if node.kind() == K::Repeat {
                    repeats += 1;
                } else if repeats == 0 && may_not_run(&node) {
                    conditional_bodies += 1;
                }
            }
            WalkEvent::Leave(node) => {
                if node.kind() == K::Repeat {
                    repeats -= 1;
                } else if repeats == 0 && may_not_run(&node) {
                    conditional_bodies -= 1;
                }
            }
        }
    }
    required
}

fn may_not_run(node: &SyntaxNode) -> bool {
    match node.kind() {
        K::If => true,
        K::Arm => tokens(node).iter().any(|token| token.text() == "optional"),
        _ => false,
    }
}
