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
        frame.set(
            Name::new(NameId::INSTANCES.0, "instances"),
            Value::int(self.count as i64),
        );
        if !self.names.is_empty() && self.names.iter().all(|n| !n.is_empty()) {
            frame.set(Name::new(NameId::NAME.0, "name"), strings(self.names));
        }
        if self.failures.iter().any(|m| !m.is_empty()) {
            frame.set(
                Name::new(NameId::FAILED.0, "failed"),
                strings(self.failures),
            );
        }
        frame
            .into_data(&crate::cancellation::Control::default())
            .expect("uncancelled catalog")
    }
}
fn strings(values: Vec<String>) -> Value {
    let count = values.len();
    let mut value = Value {
        leaves: LeafStorage::String(values),
        levels: vec![],
    };
    value
        .set_dimensions_controlled(&[count], &crate::cancellation::Control::default())
        .expect("uncancelled catalog");
    value
}
/// Borrows a string name, so comparisons at every rejection do not copy it.
fn name(frame: &Frame) -> Cow<'_, str> {
    let Some(value) = frame.lookup(NameId::NAME).filter(|v| v.depth() == 0) else {
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
        self.input.control.check()?;
        Ok(self.catalog.take().expect("catalog execution").into_data())
    }
    pub(super) fn finish_input(&mut self) -> Result<(), Failure> {
        self.statement = None;
        self.context = None;
        if !self.selected && !self.input.settings.ignore_trailing {
            self.input.skip_to_data()?;
            if self.input.position().offset < self.input.bytes.len() {
                return Err(self.error(
                    ErrorCode::TrailingInput,
                    "unexpected input after the end of the spec",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn execute_instances(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let selection = Rc::new(match self.params.get(&NameId::INSTANCE) {
            None => Selection::Single,
            Some(text) => {
                match numeric::parse_integer_controlled(text.as_bytes(), &self.input.control)? {
                    Some(n) if n < 1 => {
                        return Err(
                            self.error(ErrorCode::InstanceNumber, "instance numbers start at 1")
                        );
                    }
                    Some(n) => Selection::Ordinal(n),
                    None => Selection::Name(self.input.control.copy_text(text)?),
                }
            }
        });
        let body = child(node, K::Block);
        let mut count = 0;
        loop {
            self.input.skip_to_data()?;
            if self.input.peek_byte().is_none() {
                break;
            }
            count += 1;
            let (instance, frame) = self.read_instance(node, &body, count, selection.clone())?;
            let instance_name = self.input.control.copy_text(&name(&frame))?;
            let retain = self.catalog.is_none()
                && match selection.as_ref() {
                    Selection::Single => count == 1,
                    Selection::Ordinal(n) => *n as u64 == count as u64,
                    Selection::Name(wanted) => self
                        .input
                        .control
                        .equal_bytes(wanted.as_bytes(), instance_name.as_bytes())?,
                };
            self.finish_instance_scope(&instance, retain)?;
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
                outer.extend(frame, &self.input.control)?;
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
        } else if let Some(wanted) = self.params.get(&NameId::INSTANCE) {
            (
                ErrorCode::InstanceNotFound,
                self.input.control.text_message(&[
                    "there is no instance '",
                    wanted,
                    &format!(
                        "' (the file has {count} instance{})",
                        if count == 1 { "" } else { "s" }
                    ),
                ])?,
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
        Err(self.error(code, message))
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
                error.details.earlier_rejection =
                    Some(self.input.control.copy_text(&rejection.summary)?);
            }
            return Err(failure);
        }
        let instance = self.instance.take().expect("completed instance");
        let frame = self.frames.pop().expect("instance frame");
        self.context = None;
        self.select_statement(node);
        if self.input.position().offset == start {
            return Err(self.error(ErrorCode::NoProgress, "an instance must read some input"));
        }
        Ok((instance, frame))
    }
    fn finish_instance_scope(&mut self, instance: &Instance, retain: bool) -> Result<(), Failure> {
        for (id, previous) in instance.types.iter().rev() {
            self.input.control.check()?;
            if let Some(previous) = previous {
                self.types.insert(*id, previous.clone());
            } else {
                self.types.remove(id);
            }
        }
        if !retain {
            for binding in instance.records.iter().rev() {
                self.input.control.check()?;
                self.work(WorkKind::Metadata, 1);
                let key = (binding.repeat, binding.parent);
                if let Some(previous) = binding.previous {
                    self.records.occurrences.insert(key, previous);
                } else {
                    self.records.occurrences.remove(&key);
                }
            }
        }
        Ok(())
    }
    pub(super) fn reject(&mut self, error: Error, summary: String) -> Result<(), Failure> {
        let defer = if let Some(instance) = self.instance.as_ref() {
            self.catalog.is_some()
                || match instance.selection.as_ref() {
                    Selection::Single => instance.ordinal != 1,
                    Selection::Ordinal(n) => *n as u64 != instance.ordinal as u64,
                    Selection::Name(wanted) => !self
                        .input
                        .control
                        .equal_bytes(wanted.as_bytes(), name(&self.frames[1]).as_bytes())?,
                }
        } else {
            false
        };
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
