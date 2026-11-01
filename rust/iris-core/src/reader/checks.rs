//! Admission and ordered traversal of the dependencies of one check statement.
use super::*;
use crate::eval::{Evaluator, Kind, View};
use crate::reader::repeat::RecordId;
use iris_sema::{CheckInfo, Denotation};
use std::fmt::Write;

#[derive(Clone)]
enum Cursor<'a> {
    Value(View<'a>),
    Marker(Option<RecordId>),
}
struct LiftFrame<'a> {
    level: usize,
    length: usize,
    next: usize,
    parents: Vec<(usize, Cursor<'a>)>,
}
enum LiftWork<'a> {
    Enter(usize),
    Rows(LiftFrame<'a>),
}

impl Interpreter<'_, '_> {
    pub(crate) fn execute_check(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        self.work(WorkKind::Probe, 1);
        let info = &self.spec.analysis.model.checks()[self.spec.index.checks[&node.text_range()]];
        let mut evaluator = Evaluator::new(self);
        let result = run_check(&mut evaluator, info);
        #[cfg(test)]
        {
            let charges = std::mem::take(&mut evaluator.charges);
            drop(evaluator);
            self.charges.extend(charges);
        }
        result
    }
}
fn run_check(evaluator: &mut Evaluator<'_, '_, '_>, info: &CheckInfo) -> Result<(), Failure> {
    let mut cursors = Vec::new();
    let mut deepest = None;
    for (slot, dependency) in info.dependencies.iter().enumerate() {
        let cursor = if matches!(
            evaluator
                .reader
                .spec
                .index
                .structural
                .get(&dependency.label),
            Some(Denotation::Record(_))
        ) {
            let parent = evaluator.parent_for(dependency.repeats[0]);
            evaluator
                .list(dependency.repeats[0], parent)
                .map(|_| Cursor::Marker(parent))
        } else {
            evaluator.lookup_name(&dependency.label).map(Cursor::Value)
        };
        if cursor.is_some()
            && deepest.is_none_or(|i: usize| {
                info.dependencies[i].repeats.len() < dependency.repeats.len()
            })
        {
            deepest = Some(slot);
        }
        evaluator
            .dependency_slots
            .insert(dependency.label.clone(), slot);
        evaluator.dependencies.push(match &cursor {
            Some(Cursor::Value(v)) => Some(Kind::Value(v.clone())),
            _ => None,
        });
        cursors.push(cursor);
    }
    let depth = deepest.map_or(0, |i| info.dependencies[i].repeats.len());
    let mut participants = vec![Vec::new(); depth];
    for (slot, dependency) in info.dependencies.iter().enumerate() {
        if cursors[slot].is_some() {
            for slots in &mut participants[..dependency.repeats.len()] {
                slots.push(slot);
            }
        }
    }
    let condition = expression(&info.node);
    let mut work = vec![LiftWork::Enter(0)];
    let mut indices = Vec::new();
    while let Some(next) = work.pop() {
        match next {
            LiftWork::Enter(level) if level == depth => {
                evaluator.witnesses.clear();
                let result = evaluator.evaluate(&condition)?;
                if !result.truth() {
                    return Err(failure(evaluator, info, deepest, &indices, result.witness).into());
                }
            }
            LiftWork::Enter(level) => {
                let mut parents = Vec::new();
                let mut length = None;
                for &slot in &participants[level] {
                    let cursor = cursors[slot].as_ref().expect("available dependency");
                    let n = match cursor {
                        Cursor::Value(value) => value.len().ok_or_else(|| {
                            evaluator
                                .reader
                                .error(ErrorCode::CheckRecords, "check dependency has no records")
                        })?,
                        Cursor::Marker(parent) => evaluator
                            .list(info.dependencies[slot].repeats[level], *parent)
                            .map_or(0, |list| evaluator.list_len(list)),
                    };
                    if length.is_some_and(|length| length != n) {
                        return Err(evaluator
                            .reader
                            .error(
                                ErrorCode::CheckRecords,
                                "check dependencies have different numbers of records",
                            )
                            .into());
                    }
                    length = Some(n);
                    parents.push((slot, cursor.clone()));
                }
                work.push(LiftWork::Rows(LiftFrame {
                    level,
                    length: length.expect("available deepest dependency"),
                    next: 0,
                    parents,
                }));
            }
            LiftWork::Rows(mut frame) => {
                indices.truncate(frame.level);
                let chain = &info.dependencies[deepest.expect("lifted check")].repeats;
                if frame.next == frame.length {
                    for (slot, parent) in frame.parents {
                        evaluator.dependencies[slot] = match &parent {
                            Cursor::Value(v) => Some(Kind::Value(v.clone())),
                            _ => None,
                        };
                        cursors[slot] = Some(parent);
                    }
                    evaluator.selected.remove(&chain[frame.level]);
                    continue;
                }
                let row = frame.next;
                let repeat = chain[frame.level];
                if let Some(list) = evaluator.list(repeat, evaluator.parent_for(repeat))
                    && row < evaluator.list_len(list)
                {
                    let record = evaluator.list_record(list, row);
                    evaluator.selected.insert(repeat, record);
                }
                for (slot, parent) in &frame.parents {
                    let cursor = match parent {
                        Cursor::Value(value) => {
                            let selected = value.child(row).expect("aligned row");
                            evaluator.reader.work(WorkKind::Metadata, 1);
                            let leaves = evaluator.leaf_span(&selected).len();
                            evaluator.charge(1 + leaves as u128)?;
                            evaluator.dependencies[*slot] = Some(Kind::Value(selected.clone()));
                            Cursor::Value(selected)
                        }
                        Cursor::Marker(parent) => {
                            let chain = &info.dependencies[*slot].repeats;
                            let list = evaluator
                                .list(chain[frame.level], *parent)
                                .expect("nonempty marker list");
                            let record = evaluator.list_record(list, row);
                            let leaves =
                                evaluator.marker_leaves(chain, frame.level + 1, Some(record));
                            evaluator.charge(1 + leaves as u128)?;
                            Cursor::Marker(Some(record))
                        }
                    };
                    cursors[*slot] = Some(cursor);
                }
                indices.push(row);
                frame.next += 1;
                let level = frame.level + 1;
                work.push(LiftWork::Rows(frame));
                work.push(LiftWork::Enter(level));
            }
        }
    }
    Ok(())
}
fn failure(
    evaluator: &Evaluator<'_, '_, '_>,
    info: &CheckInfo,
    deepest: Option<usize>,
    indices: &[usize],
    witness: Option<usize>,
) -> Error {
    let mut witnesses = Vec::new();
    let mut next = witness;
    while let Some(id) = next {
        evaluator.reader.work(WorkKind::Link, 1);
        witnesses.push(&evaluator.witnesses[id]);
        next = evaluator.witnesses[id].inner;
    }
    let mut path = indices.to_vec();
    path.extend(witnesses.iter().map(|w| w.index));
    evaluator.reader.work(WorkKind::Link, path.len());
    let mut explanation = String::new();
    let mut position = None;
    for witness in witnesses.iter().rev() {
        if !explanation.is_empty() {
            explanation.push_str(" in ");
        }
        let label = witness
            .record
            .and_then(|r| evaluator.reader.spec.analysis.model.repeats()[r.0].list_definition)
            .map(|id| {
                format!(
                    "'{}'",
                    evaluator.reader.spec.analysis.model.definition(id).name
                )
            })
            .unwrap_or_else(|| array_name(&witness.domain));
        write!(
            explanation,
            "{} {} of {label}",
            if witness.record.is_some() {
                "record"
            } else {
                "element"
            },
            witness.index + 1
        )
        .expect("string write");
        if position.is_none() {
            position = witness.position;
        }
    }
    if let Some(slot) = deepest.filter(|_| !indices.is_empty()) {
        if !explanation.is_empty() {
            explanation.push_str(" in ");
        }
        let dependency = &info.dependencies[slot];
        let model = &evaluator.reader.spec.analysis.model;
        let named = dependency
            .repeats
            .iter()
            .all(|r| model.repeats()[r.0].list_definition.is_some());
        for (level, &index) in indices.iter().enumerate().rev() {
            write!(explanation, "record {} of ", index + 1).expect("string write");
            if named {
                let id = model.repeats()[dependency.repeats[level].0]
                    .list_definition
                    .expect("named");
                write!(explanation, "'{}'", model.definition(id).name).expect("string write");
                if level > 0 {
                    explanation.push_str(" in ");
                }
            }
        }
        if !named {
            write!(explanation, "'{}'", dependency.label).expect("string write");
        }
        if position.is_none() {
            position = dependency
                .repeats
                .last()
                .and_then(|r| evaluator.selected.get(r))
                .map(|r| evaluator.record_start(*r));
        }
    }
    let mut message = String::from("check failed");
    if !explanation.is_empty() {
        message.push_str(" for ");
        message.push_str(&explanation);
    }
    if let Some(token) = tokens(&info.node).iter().find(|t| t.kind() == K::String) {
        let text = DecodedString::new(token.text()).text;
        if !text.is_empty() {
            message.push_str(": ");
            message.push_str(&text);
        }
    }
    let mut error = evaluator.reader.error_at(
        ErrorCode::CheckFailed,
        message,
        position.unwrap_or_else(|| evaluator.reader.input.position()),
    );
    error.details.record_path = Some(path);
    error
}
fn array_name(node: &SyntaxNode) -> String {
    let mut node = node.clone();
    while node.kind() == K::Parenthesized {
        node = expression(&node);
    }
    if node.kind() == K::Name {
        return format!("'{}'", tokens(&node)[0].text());
    }
    if node.kind() == K::Field {
        let mut base = expression(&node);
        while base.kind() == K::Parenthesized {
            base = expression(&base);
        }
        if base.kind() == K::Name {
            let field = tokens(&node)
                .into_iter()
                .find(|t| t.kind() == K::Ident)
                .expect("resolved field name");
            return format!("'{}.{}'", tokens(&base)[0].text(), field.text());
        }
    }
    "the array".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{ArrayLevel, LeafStorage};
    #[test]
    fn malformed_private_columns_fail_without_a_record_path() {
        for replacement in [
            Value::int(1),
            Value {
                leaves: LeafStorage::Int(vec![1]),
                levels: vec![ArrayLevel {
                    offsets: vec![0, 1],
                }],
            },
        ] {
            let spec = Spec::parse(b"repeat 2 {x:int let y=x} if 0 {check x==y}", "spec").unwrap();
            let mut reader = Interpreter::new(&spec, Input::new(b"1 2", "input"));
            reader.execute(spec.parsed.syntax()).unwrap();
            reader.frames[0].set("x".into(), replacement);
            let node = &spec.analysis.model.checks()[0].node;
            reader.select_statement(node);
            let Failure::Language(error) = reader.execute_check(node).unwrap_err() else {
                panic!("language error")
            };
            assert_eq!(error.code, ErrorCode::CheckRecords);
            assert_eq!(error.record_path(), None);
        }
        let spec = Spec::parse(b"repeat 2 as rows {x:int} if 0 {let v=rows[1].x}", "spec").unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"1 2", "input"));
        reader.execute(spec.parsed.syntax()).unwrap();
        reader.frames[0].set(
            "x".into(),
            Value {
                leaves: LeafStorage::Int(vec![1]),
                levels: vec![ArrayLevel {
                    offsets: vec![0, 1],
                }],
            },
        );
        let field = spec
            .analysis
            .model
            .expressions()
            .iter()
            .find(|e| e.node.kind() == K::Field)
            .unwrap();
        let Failure::Language(error) = reader.eval(&field.node).unwrap_err() else {
            panic!("language error")
        };
        assert_eq!(error.code, ErrorCode::FieldUndefined);
        assert_eq!(error.record_path(), None);
    }
}
