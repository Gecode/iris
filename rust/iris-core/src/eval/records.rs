//! Logical records remain metadata; fields remain borrowed column selections.
use super::*;
use crate::reader::repeat::{MarkerSubtree, RecordId};
use iris_sema::{Denotation, RepeatId, Resolution};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ListSelection {
    pub occurrence: Option<usize>,
}

impl<'a> Evaluator<'a, '_, '_> {
    pub(crate) fn list(&self, repeat: RepeatId, parent: Option<RecordId>) -> Option<ListSelection> {
        self.reader.work(WorkKind::Metadata, 1);
        let occurrence = self
            .reader
            .records
            .occurrences
            .get(&(repeat, parent))
            .copied();
        let available = occurrence.is_some_and(|i| self.reader.records.lists[i].complete)
            || parent.is_some_and(|p| self.reader.records.lists[p.occurrence].complete);
        available.then_some(ListSelection { occurrence })
    }
    pub(crate) fn list_len(&self, list: ListSelection) -> usize {
        list.occurrence
            .map_or(0, |i| self.reader.records.lists[i].starts.len())
    }
    pub(crate) fn list_record(&self, list: ListSelection, row: usize) -> RecordId {
        self.reader.work(WorkKind::Metadata, 1);
        RecordId {
            occurrence: list.occurrence.expect("nonempty list has metadata"),
            row,
        }
    }
    pub(crate) fn parent_for(&self, repeat: RepeatId) -> Option<RecordId> {
        self.reader.spec.analysis.model.repeats()[repeat.0]
            .parent
            .and_then(|p| {
                self.selected
                    .get(&p)
                    .or_else(|| self.reader.records.active.get(&p))
                    .copied()
            })
    }
    pub(crate) fn record_start(&self, record: RecordId) -> crate::input::Position {
        self.reader.records.lists[record.occurrence].starts[record.row]
    }
    pub(crate) fn is_defined(&mut self, node: &SyntaxNode) -> bool {
        let token = &tokens(node)[0];
        let id = self.reader.spec.index.references[&token.text_range()];
        match &self.reader.spec.analysis.model.reference(id).resolution {
            Resolution::Bound(id) => self.bindings.contains_key(id),
            Resolution::Structural {
                denotation: Denotation::RecordList(repeat),
                ..
            } => self.list(*repeat, self.parent_for(*repeat)).is_some(),
            _ => self.lookup_name(token.text()).is_some(),
        }
    }
    pub(crate) fn name(&mut self, node: &SyntaxNode) -> Result<Kind<'a>, Failure> {
        let token = &tokens(node)[0];
        let reference = self.reader.spec.index.references[&token.text_range()];
        let resolution = &self
            .reader
            .spec
            .analysis
            .model
            .reference(reference)
            .resolution;
        match resolution {
            Resolution::Bound(id) => Ok(self.bindings.get(id).cloned().expect("active binding")),
            Resolution::Structural { denotation, .. } => match denotation {
                Denotation::Record(repeat) => self
                    .selected
                    .get(repeat)
                    .or_else(|| self.reader.records.active.get(repeat))
                    .copied()
                    .map(Kind::Record)
                    .ok_or_else(|| self.undefined(token.text())),
                Denotation::RecordList(repeat) => self
                    .list(*repeat, self.parent_for(*repeat))
                    .map(Kind::Records)
                    .ok_or_else(|| self.undefined(token.text())),
                Denotation::Value => unreachable!("structural denotation"),
            },
            Resolution::Value { projection, .. } if !projection.is_empty() => {
                if let Some(&slot) = self.dependency_slots.get(token.text()) {
                    return self.dependencies[slot]
                        .clone()
                        .ok_or_else(|| self.undefined(token.text()));
                }
                self.value_name(token.text()).map(Kind::Value)
            }
            _ => self.value_name(token.text()).map(Kind::Value),
        }
    }
    pub(crate) fn value_name(&mut self, name: &str) -> Result<View<'a>, Failure> {
        self.lookup_name(name).ok_or_else(|| self.undefined(name))
    }
    pub(crate) fn lookup_name(&mut self, name: &str) -> Option<View<'a>> {
        self.reader.work(WorkKind::Probe, 1);
        if let Some(value) = self.names.get(name) {
            return value.clone();
        }
        let mut value = None;
        for frame in self.reader.frames.iter().rev() {
            self.reader.work(WorkKind::Probe, 1);
            if let Some(root) = frame.lookup(name) {
                value = Some(View::borrowed(root));
                break;
            }
        }
        self.names.insert(name.to_owned(), value.clone());
        value
    }
    pub(crate) fn undefined(&self, name: &str) -> Failure {
        self.reader
            .error(
                ErrorCode::NameUndefined,
                format!("'{name}' is not defined yet"),
            )
            .into()
    }
    pub(crate) fn field(
        &mut self,
        node: &SyntaxNode,
        record: RecordId,
    ) -> Result<Kind<'a>, Failure> {
        let info = &self.reader.spec.analysis.model.expressions()
            [self.reader.spec.index.expressions[&node.text_range()]];
        if let Denotation::RecordList(repeat) = info.denotation {
            return self
                .list(repeat, Some(record))
                .map(Kind::Records)
                .ok_or_else(|| self.undefined(field_name(node).text()));
        }
        let name = field_name(node).text().to_owned();
        let root = self.value_name(&name)?;
        let mut view = self.record_field(root, record).ok_or_else(|| {
            self.reader.error(
                ErrorCode::FieldUndefined,
                format!("'{name}' is not defined in this record"),
            )
        })?;
        if view.level > 0 {
            let leaves = self.leaf_span(&view).len();
            self.charge(1 + leaves as u128)?;
        }
        view.record_parent = Some(record);
        Ok(Kind::Value(view))
    }
    fn record_field(&mut self, root: View<'a>, record: RecordId) -> Option<View<'a>> {
        let key = root.key();
        let mut pending = Vec::new();
        let mut at = record;
        let mut view = loop {
            self.reader.work(WorkKind::Probe, 1);
            if let Some(view) = self.fields.get(&(key, at)) {
                break view.clone();
            }
            let list = &self.reader.records.lists[at.occurrence];
            if self.reader.records.active.get(&list.repeat) == Some(&at) {
                break root.clone();
            }
            pending.push(at);
            match list.parent {
                Some(parent) => at = parent,
                None => break root.clone(),
            }
        };
        while let Some(record) = pending.pop() {
            self.reader.work(WorkKind::Metadata, 1);
            view = view.child(record.row)?;
            self.fields.insert((key, record), view.clone());
        }
        Some(view)
    }
    /// Count just the requested marker subtree, caching every intermediate node.
    pub(crate) fn marker_leaves(
        &self,
        chain: &[RepeatId],
        level: usize,
        parent: Option<RecordId>,
    ) -> usize {
        if level == chain.len() {
            return 1;
        }
        let target = *chain.last().expect("marker chain");
        let key = MarkerSubtree {
            target,
            next: chain[level],
            parent,
        };
        let mut counts = self.reader.records.marker_counts.borrow_mut();
        if let Some(&count) = counts.get(&key) {
            return count;
        }
        let mut stack = vec![(level, parent, false)];
        while let Some((level, parent, finish)) = stack.pop() {
            let key = MarkerSubtree {
                target,
                next: chain[level],
                parent,
            };
            if counts.contains_key(&key) {
                continue;
            }
            let Some(list) = self.list(chain[level], parent) else {
                counts.insert(key, 0);
                continue;
            };
            let length = self.list_len(list);
            if level + 1 == chain.len() {
                counts.insert(key, length);
                continue;
            }
            if finish {
                let mut count = 0;
                for row in 0..length {
                    let record = self.list_record(list, row);
                    count += counts[&MarkerSubtree {
                        target,
                        next: chain[level + 1],
                        parent: Some(record),
                    }];
                }
                counts.insert(key, count);
            } else {
                stack.push((level, parent, true));
                for row in (0..length).rev() {
                    stack.push((level + 1, Some(self.list_record(list, row)), false));
                }
            }
        }
        counts[&key]
    }
}

fn field_name(node: &SyntaxNode) -> iris_syntax::SyntaxToken {
    tokens(node)
        .into_iter()
        .find(|t| t.kind() == K::Ident)
        .expect("resolved field name")
}
