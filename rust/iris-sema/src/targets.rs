//! Ordered definition unions shared by flow environments until a reference needs them.
use crate::DefinitionId;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TargetSetId(usize);
impl TargetSetId {
    pub(crate) const EMPTY: Self = Self(0);
}
enum TargetSet {
    Empty,
    Definition(DefinitionId),
    Union(TargetSetId, TargetSetId),
}
pub(crate) struct TargetSets {
    nodes: Vec<TargetSet>,
}
impl TargetSets {
    pub(crate) fn new(definitions: usize) -> Self {
        // Definition leaves occupy id + 1; zero denotes the empty set.
        let mut nodes = vec![TargetSet::Empty];
        nodes.extend((0..definitions).map(|i| TargetSet::Definition(DefinitionId(i))));
        Self { nodes }
    }
    pub(crate) fn single(id: DefinitionId) -> TargetSetId {
        TargetSetId(id.0 + 1)
    }
    pub(crate) fn insert(&mut self, definitions: &[DefinitionId]) -> TargetSetId {
        definitions.iter().fold(TargetSetId::EMPTY, |set, &id| {
            self.join(set, Self::single(id))
        })
    }
    pub(crate) fn join(&mut self, a: TargetSetId, b: TargetSetId) -> TargetSetId {
        crate::work::record();
        if a == b || b == TargetSetId::EMPTY {
            return a;
        }
        if a == TargetSetId::EMPTY {
            return b;
        }
        let id = TargetSetId(self.nodes.len());
        self.nodes.push(TargetSet::Union(a, b));
        id
    }
    pub(crate) fn to_vec(&self, set: TargetSetId) -> Vec<DefinitionId> {
        let mut pending = vec![set];
        let mut visited = HashSet::new();
        let mut definitions = HashSet::new();
        let mut result = vec![];
        // Shared subtrees are visited once, with left targets first. The arena
        // and iterative traversal keep long chains off the call stack, both
        // when expanding targets and when dropping the analysis state.
        while let Some(id) = pending.pop() {
            crate::work::record();
            if !visited.insert(id) {
                continue;
            }
            match self.nodes[id.0] {
                TargetSet::Empty => {}
                TargetSet::Definition(id) => {
                    if definitions.insert(id) {
                        result.push(id);
                    }
                }
                TargetSet::Union(a, b) => {
                    pending.push(b);
                    pending.push(a);
                }
            }
        }
        result
    }
}
