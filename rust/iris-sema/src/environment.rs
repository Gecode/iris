//! Flow bindings with checkpoints that save and restore only changed names.
use crate::{ValueFacts, targets::TargetSetId};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) struct Binding {
    pub(crate) definitions: TargetSetId,
    pub(crate) facts: ValueFacts,
    pub(crate) possibly_unavailable: bool,
}
pub(crate) type Changes = BTreeMap<String, Binding>;
struct Change {
    name: String,
    previous: Option<Binding>,
}
#[derive(Default)]
pub(crate) struct Environment {
    bindings: BTreeMap<String, Binding>,
    undo: Vec<Change>,
}
impl Environment {
    pub(crate) fn get(&self, name: &str) -> Option<&Binding> {
        crate::work::record();
        self.bindings.get(name)
    }
    pub(crate) fn insert(&mut self, name: String, binding: Binding) {
        crate::work::record();
        let previous = self.bindings.insert(name.clone(), binding);
        self.undo.push(Change { name, previous });
    }
    pub(crate) fn checkpoint(&self) -> usize {
        self.undo.len()
    }
    pub(crate) fn rollback(&mut self, checkpoint: usize) -> Changes {
        let mut changed = Changes::new();
        while self.undo.len() > checkpoint {
            crate::work::record();
            let Change { name, previous } = self.undo.pop().expect("change after checkpoint");
            // The last write supplies the branch result; earlier writes only
            // restore its base. Nested checkpoints use this same undo log.
            let current = self.bindings.remove(&name).expect("written binding");
            changed.entry(name.clone()).or_insert(current);
            if let Some(previous) = previous {
                self.bindings.insert(name, previous);
            }
        }
        changed
    }
    pub(crate) fn make_uncertain(&mut self) {
        // Instances can invalidate every enclosing name (Specification §9.3).
        // There is at most one instances statement, at top level.
        let names: Vec<_> = self.bindings.keys().cloned().collect();
        for name in names {
            let mut binding = self.bindings[&name].clone();
            binding.facts = ValueFacts::unknown();
            binding.possibly_unavailable = true;
            self.insert(name, binding);
        }
    }
}
