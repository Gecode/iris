//! Instance traversal owns local frames, selection, and deferred rejections.
use super::*;
use crate::Data;
use crate::reader::repeat::RecordId;
use std::borrow::Cow;
use std::rc::Rc;

pub(super) enum Selection {
    Single,
    Ordinal(i64),
    Name(String),
}
pub(super) struct Instance {
    pub ordinal: usize,
    selection: Rc<Selection>,
    deferred: Option<Rejection>,
    pub types: Vec<(DefinitionId, Option<EvaluatedType>)>,
    pub records: Vec<RecordBinding>,
}
pub(super) struct RecordBinding {
    pub repeat: RepeatId,
    pub parent: Option<RecordId>,
    pub previous: Option<usize>,
}
struct Rejection {
    error: Error,
    summary: String,
}
#[derive(Default)]
pub(super) struct Catalog {
    count: usize,
    names: Vec<String>,
    failures: Vec<String>,
}
impl Catalog {
    fn into_data(self) -> Data {
        let mut frame = Frame::default();
        frame.set("instances".into(), Value::int(self.count as i64));
        if !self.names.is_empty() && self.names.iter().all(|n| !n.is_empty()) {
            frame.set("name".into(), strings(self.names));
        }
        if self.failures.iter().any(|m| !m.is_empty()) {
            frame.set("failed".into(), strings(self.failures));
        }
        frame.into_data()
    }
}
fn strings(values: Vec<String>) -> Value {
    let count = values.len();
    let mut value = Value {
        leaves: LeafStorage::String(values),
        levels: vec![],
    };
    value.set_dimensions(&[count]);
    value
}
/// Borrows a string name, so comparisons at every rejection do not copy it.
fn name(frame: &Frame) -> Cow<'_, str> {
    let Some(value) = frame.lookup("name").filter(|v| v.depth() == 0) else {
        return Cow::Borrowed("");
    };
    match value.value_type() {
        "int" => value
            .root()
            .as_int()
            .expect("scalar int")
            .to_string()
            .into(),
        "string" => value.root().as_str().expect("scalar string").into(),
        _ => Cow::Borrowed(""),
    }
}
impl Interpreter<'_, '_> {
    pub fn catalog(mut self) -> Result<Data, Failure> {
        self.catalog = Some(Catalog {
            count: 1,
            ..Catalog::default()
        });
        // Read also performs the ordinary trailing-input check. Keep catalog
        // results outside that consuming boundary.
        self.execute(self.spec.parsed.syntax())?;
        self.finish_input()?;
        Ok(self.catalog.take().expect("catalog execution").into_data())
    }
    pub(super) fn finish_input(&mut self) -> Result<(), Failure> {
        self.statement = None;
        self.context = None;
        if !self.selected && !self.input.settings.ignore_trailing {
            self.input.skip_to_data();
            if self.input.position().offset < self.input.bytes.len() {
                return Err(self
                    .error(
                        ErrorCode::TrailingInput,
                        "unexpected input after the end of the spec",
                    )
                    .into());
            }
        }
        Ok(())
    }
    pub(super) fn execute_instances(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let selection = Rc::new(match self.params.get("instance") {
            None => Selection::Single,
            Some(text) => match numeric::parse_integer(text.as_bytes()) {
                Some(n) if n < 1 => {
                    return Err(self
                        .error(ErrorCode::InstanceNumber, "instance numbers start at 1")
                        .into());
                }
                Some(n) => Selection::Ordinal(n),
                None => Selection::Name(text.into()),
            },
        });
        let body = child(node, K::Block);
        let mut count = 0;
        loop {
            self.input.skip_to_data();
            if self.input.peek_byte().is_none() {
                break;
            }
            count += 1;
            let (instance, frame) = self.read_instance(node, &body, count, selection.clone())?;
            let instance_name = name(&frame).into_owned();
            let retain = self.catalog.is_none()
                && match selection.as_ref() {
                    Selection::Single => count == 1,
                    Selection::Ordinal(n) => *n as u64 == count as u64,
                    Selection::Name(wanted) => *wanted == instance_name,
                };
            self.finish_instance_scope(&instance, retain);
            if let Some(catalog) = &mut self.catalog {
                catalog.names.push(instance_name);
                catalog
                    .failures
                    .push(instance.deferred.map_or_else(String::new, |r| r.summary));
            } else if retain {
                if let Some(rejection) = instance.deferred {
                    return Err(rejection.error.into());
                }
                let outer = self.frames.last_mut().expect("outer frame");
                for entry in frame.into_data().entries {
                    outer.set(entry.name, entry.value);
                }
                if !matches!(selection.as_ref(), Selection::Single) {
                    self.selected = true;
                    return Ok(());
                }
            }
        }
        if let Some(catalog) = &mut self.catalog {
            catalog.count = count;
            return Ok(());
        }
        let (code, message) = if count == 0 {
            (
                ErrorCode::InstanceEmpty,
                "the file contains no instance".into(),
            )
        } else if let Some(wanted) = self.params.get("instance") {
            (
                ErrorCode::InstanceNotFound,
                format!(
                    "there is no instance '{wanted}' (the file has {count} instance{})",
                    if count == 1 { "" } else { "s" }
                ),
            )
        } else if count > 1 {
            (
                ErrorCode::InstanceSelection,
                format!(
                    "the file has {count} instances; select one with the parameter instance (a number from 1, or a name)"
                ),
            )
        } else {
            return Ok(());
        };
        Err(self.error(code, message).into())
    }
    fn read_instance(
        &mut self,
        node: &SyntaxNode,
        body: &SyntaxNode,
        ordinal: usize,
        selection: Rc<Selection>,
    ) -> Result<(Instance, Frame), Failure> {
        let start = self.input.position().offset;
        self.frames.push(Frame::default());
        self.instance = Some(Instance {
            ordinal,
            selection,
            deferred: None,
            types: vec![],
            records: vec![],
        });
        self.select_statement(node);
        if let Err(mut failure) = self.execute(body.clone()) {
            if let (Failure::Language(error), Some(rejection)) = (
                &mut failure,
                self.instance.as_ref().and_then(|i| i.deferred.as_ref()),
            ) {
                error.details.earlier_rejection = Some(rejection.summary.clone());
            }
            return Err(failure);
        }
        let instance = self.instance.take().expect("completed instance");
        let frame = self.frames.pop().expect("instance frame");
        self.context = None;
        self.select_statement(node);
        if self.input.position().offset == start {
            return Err(self
                .error(ErrorCode::NoProgress, "an instance must read some input")
                .into());
        }
        Ok((instance, frame))
    }
    fn finish_instance_scope(&mut self, instance: &Instance, retain: bool) {
        for (id, previous) in instance.types.iter().rev() {
            if let Some(previous) = previous {
                self.types.insert(*id, previous.clone());
            } else {
                self.types.remove(id);
            }
        }
        if !retain {
            for binding in instance.records.iter().rev() {
                self.work(WorkKind::Metadata, 1);
                let key = (binding.repeat, binding.parent);
                if let Some(previous) = binding.previous {
                    self.records.occurrences.insert(key, previous);
                } else {
                    self.records.occurrences.remove(&key);
                }
            }
        }
    }
    pub(super) fn reject(&mut self, error: Error, summary: String) -> Result<(), Failure> {
        let defer = self.instance.as_ref().is_some_and(|instance| {
            self.catalog.is_some()
                || match instance.selection.as_ref() {
                    Selection::Single => instance.ordinal != 1,
                    Selection::Ordinal(n) => *n as u64 != instance.ordinal as u64,
                    Selection::Name(wanted) => *wanted != name(&self.frames[1]),
                }
        });
        if !defer {
            return Err(error.into());
        }
        let instance = self.instance.as_mut().expect("deferring inside instance");
        if instance.deferred.is_none() {
            instance.deferred = Some(Rejection { error, summary });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
