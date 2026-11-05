use super::*;
use crate::value::Column;

pub(crate) struct RepeatState {
    site: plan::Site,
    pub(super) body: plan::StatementId,
    mode: Mode,
    columns: Vec<(Name, Column)>,
    pub row: usize,
    frame: Frame,
    before: Option<usize>,
    pub occurrence: usize,
}
enum Mode {
    Count(usize),
    Eof,
    Tokens {
        alternatives: Vec<String>,
        store: Option<Name>,
    },
}
impl RepeatState {
    pub fn context(&self) -> Option<String> {
        self.before.map(|_| match self.mode {
            Mode::Count(count) => format!("repeat [{} of {count}]", self.row + 1),
            _ => format!("repeat [{}]", self.row + 1),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct RecordId {
    pub occurrence: usize,
    pub row: usize,
}
pub(crate) struct RecordList {
    pub repeat: RepeatId,
    pub parent: Option<RecordId>,
    pub starts: Vec<Position>,
    pub complete: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MarkerSubtree {
    pub target: RepeatId,
    pub occurrence: Option<usize>,
    pub next: RepeatId,
    pub parent: Option<RecordId>,
}
#[derive(Default)]
pub(crate) struct RecordLists {
    pub lists: Vec<RecordList>,
    pub active: HashMap<RepeatId, RecordId>,
    pub occurrences: HashMap<(RepeatId, Option<RecordId>), usize>,
    pub marker_counts: std::cell::RefCell<HashMap<MarkerSubtree, usize>>,
}
impl RecordLists {
    fn begin(&mut self, repeat: RepeatId, parent: Option<RecordId>) -> usize {
        let index = self.lists.len();
        self.lists.push(RecordList {
            repeat,
            parent,
            starts: vec![],
            complete: false,
        });
        self.occurrences.insert((repeat, parent), index);
        index
    }
}

impl Interpreter<'_, '_> {
    #[cfg(test)]
    pub(super) fn begin_repeat(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let statement = self
            .spec
            .index
            .plan
            .statement(self.spec.index.plan.id(node));
        let plan::Operation::Repeat(repeat) = &statement.operation else {
            unreachable!("repeat statement")
        };
        self.begin_prepared_repeat(repeat, statement.site)
    }
    pub(super) fn begin_prepared_repeat(
        &mut self,
        repeat: &plan::Repeat,
        site: plan::Site,
    ) -> Result<(), Failure> {
        let mode = self.prepared_repeat_mode(&repeat.mode)?;
        let id = repeat.id;
        let columns = repeat
            .fields
            .iter()
            .map(|field| {
                self.input.control.check()?;
                self.work(WorkKind::Metadata, 1);
                Ok((field.name.clone(), Column::new(field.kind, field.depth)))
            })
            .collect::<Result<_, Failure>>()?;
        let parent = self.repeats.last().map(|repeat| RecordId {
            occurrence: repeat.occurrence,
            row: repeat.row,
        });
        let key = (id, parent);
        let previous = self.records.occurrences.get(&key).copied();
        let occurrence = self.records.begin(id, parent);
        if let Some(instance) = &mut self.instance {
            instance.records.push(instances::RecordBinding {
                repeat: id,
                parent,
                previous,
            });
        }
        self.repeats.push(RepeatState {
            site,
            body: repeat.body,
            mode,
            columns,
            row: 0,
            frame: Frame::default(),
            before: None,
            occurrence,
        });
        Ok(())
    }
    fn prepared_repeat_mode(&mut self, mode: &plan::RepeatMode) -> Result<Mode, Failure> {
        Ok(match mode {
            plan::RepeatMode::Until {
                alternatives,
                store,
            } if !alternatives.is_empty() => Mode::Tokens {
                alternatives: alternatives
                    .iter()
                    .map(|s| self.input.control.copy_text(s))
                    .collect::<Result<_, _>>()?,
                store: store.clone(),
            },
            plan::RepeatMode::Until { .. } => Mode::Eof,
            plan::RepeatMode::Count(bound) => {
                Mode::Count(self.admit_count(&bound.node, "repeat count")?)
            }
        })
    }
    pub(super) fn begin_iteration(&mut self) -> Result<bool, Failure> {
        let repeat = self.repeats.last().expect("active repeat");
        match &repeat.mode {
            Mode::Count(count) if repeat.row == *count => return Ok(false),
            Mode::Count(_) => {}
            Mode::Eof => {
                self.input.skip_to_data()?;
                if self.input.peek_byte().is_none() {
                    return Ok(false);
                }
            }
            Mode::Tokens {
                alternatives,
                store,
            } => {
                self.input.skip_to_data()?;
                let Some(token) = self.input.peek_token()? else {
                    let expected = self.input.control.quoted_alternatives(alternatives)?;
                    return Err(self.error(
                        ErrorCode::RepeatTerminator,
                        self.input.control.text_message(&[
                            "expected ",
                            &expected,
                            " before end of input",
                        ])?,
                    ));
                };
                if self.matches_token(alternatives, token.bytes)? {
                    self.input.next_token()?;
                    if let Some(name) = store {
                        self.frames
                            .last_mut()
                            .expect("enclosing frame")
                            .set_controlled(
                                name.clone(),
                                Value::string(
                                    self.input
                                        .control
                                        .copy_utf8(token.bytes)?
                                        .expect("whole input is valid UTF-8"),
                                ),
                                &self.input.control,
                            )?;
                    }
                    return Ok(false);
                }
            }
        }
        let start = self.input.next_data_position()?;
        let repeat = self.repeats.last_mut().expect("active repeat");
        self.records.lists[repeat.occurrence].starts.push(start);
        self.records.active.insert(
            self.records.lists[repeat.occurrence].repeat,
            RecordId {
                occurrence: repeat.occurrence,
                row: repeat.row,
            },
        );
        repeat.before = Some(self.input.position().offset);
        self.frames.push(std::mem::take(&mut repeat.frame));
        self.contexts
            .push(ExecutionContext::Repeat(self.repeats.len() - 1));
        Ok(true)
    }
    pub(super) fn collect_iteration(
        &mut self,
        output: Option<crate::value::EntryId>,
    ) -> Result<(), Failure> {
        let repeat = self.repeats.last().expect("active iteration");
        self.owner = Some(repeat.site.range.start());
        if repeat.before == Some(self.input.position().offset) {
            if !matches!(repeat.mode, Mode::Count(_)) {
                return Err(self.error(
                    ErrorCode::NoProgress,
                    "the body of a repeat until read no input",
                ));
            }
            self.charge_idle(1)?;
        }
        // Keep the iteration and last body statement live during row admission.
        let mut columns =
            std::mem::take(&mut self.repeats.last_mut().expect("active repeat").columns);
        for (name, column) in &mut columns {
            self.work(WorkKind::Probe, 1);
            let frame = self.frames.last_mut().expect("local frame");
            let value = match output {
                Some(entry) => frame.take_at(entry),
                None => frame.take(name.id),
            };
            #[cfg(test)]
            if let Some(value) = &value {
                self.work(WorkKind::Copy, value.leaf_count());
                for level in &value.levels {
                    self.work(WorkKind::Metadata, level.offsets.len().saturating_sub(1));
                }
            }
            let buffer = column
                .add_row_reusing(value, &self.input.control)
                .map_err(|e| self.column_failure(e, name, false))?;
            if buffer.is_some() {
                self.work(WorkKind::Probe, 1);
                let frame = self.frames.last_mut().expect("local frame");
                match output {
                    Some(entry) => frame.recycle_at(entry, buffer),
                    None => frame.recycle(name.id, buffer),
                }
            }
        }
        let mut frame = self.frames.pop().expect("local frame");
        frame.clear(&self.input.control)?;
        self.contexts.pop();
        let repeat = self.repeats.last_mut().expect("active repeat");
        repeat.columns = columns;
        repeat.frame = frame;
        self.records
            .active
            .remove(&self.records.lists[repeat.occurrence].repeat);
        repeat.row += 1;
        repeat.before = None;
        let site = repeat.site;
        self.select_site(site);
        Ok(())
    }
    pub(super) fn finish_repeat(&mut self) -> Result<(), Failure> {
        let repeat = self.repeats.pop().expect("finishing repeat");
        for (name, column) in repeat.columns {
            let value = column
                .finish_controlled(&self.input.control)
                .map_err(|e| self.column_failure(e, &name, true))?;
            self.frames
                .last_mut()
                .expect("enclosing frame")
                .set_controlled(name, value, &self.input.control)?;
        }
        self.records.lists[repeat.occurrence].complete = true;
        Ok(())
    }
    fn column_failure(
        &self,
        error: crate::value::BuildError,
        name: &str,
        finishing: bool,
    ) -> Failure {
        match error {
            crate::value::BuildError::Host(e) => e.into(),
            crate::value::BuildError::Type(code) => self.column_error(code, name, finishing),
        }
    }
    fn column_error(&self, code: ErrorCode, name: &str, finishing: bool) -> Failure {
        let detail = match code {
            ErrorCode::RepeatMissing if finishing => "is defined in no repetition",
            ErrorCode::RepeatMissing => "is defined in only some repetitions",
            ErrorCode::RepeatDepth => "has rows of different depths",
            ErrorCode::ValueTypeMix => "mixes strings and numbers",
            _ => unreachable!("column construction error"),
        };
        self.error_text(code, &["'", name, "' ", detail])
    }
    pub(super) fn admit_count(
        &mut self,
        node: &SyntaxNode,
        description: &str,
    ) -> Result<usize, Failure> {
        let value = self.eval(node)?;
        let integer = value.root().as_int().map_err(|_| {
            self.error(
                ErrorCode::Count,
                format!("{description} must be an integer"),
            )
        })?;
        if integer < 0 {
            return Err(self.error(
                ErrorCode::Count,
                format!("{description} is negative ({integer})"),
            ));
        }
        if integer > i32::MAX as i64 {
            return Err(self.error(
                ErrorCode::Count,
                format!("{description} is too large ({integer})"),
            ));
        }
        Ok(integer as usize)
    }
    pub(super) fn execute_index(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let row = self
            .repeats
            .last()
            .ok_or_else(|| self.error(ErrorCode::IndexContext, "'index' must be inside a repeat"))?
            .row;
        let base = self.admit_count(&expression(node), "index base")?;
        let expected = row as i128 + base as i128;
        let token = self.input.next_token()?.ok_or_else(|| {
            self.error(
                ErrorCode::ExpectedValue,
                "expected a value, found end of input",
            )
        })?;
        let value = self
            .convert_scalar(&EvaluatedType::Int, &token)?
            .root()
            .as_int()
            .expect("converted integer");
        if i128::from(value) != expected {
            let mut error = self.language_error_at(
                ErrorCode::IndexMismatch,
                format!("expected index {expected}, found {value}"),
                token.start,
            )?;
            error.set_input_end(token.end.offset);
            return Err(error.into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn record_starts_parents_and_absent_lists() {
        let spec = Spec::parse(b"comment \"#\" repeat 3 as rows {each row}", "spec").unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b" # tail\n  ", "input")).unwrap();
        reader.execute(spec.parsed.syntax()).unwrap();
        assert_eq!(reader.input.position().offset, 0);
        let list = &reader.records.lists[reader.records.occurrences[&(RepeatId(0), None)]];
        assert_eq!(
            list.starts,
            vec![
                Position {
                    offset: 10,
                    line: 2,
                    column: 3
                };
                3
            ]
        );
        assert!(list.complete);
        assert!(
            reader.frames[0]
                .lookup(spec.index.test_name("rows").id)
                .is_none()
        );
        assert!(
            reader.frames[0]
                .lookup(spec.index.test_name("row").id)
                .is_none()
        );

        let spec = Spec::parse(b"comment \"#\" repeat 2 as rows {each row repeat 2 as ticks {each tick} k:int if k {repeat 0 as children {each child}}}", "spec").unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"# skip\n0\n1", "input")).unwrap();
        reader.execute(spec.parsed.syntax()).unwrap();
        let root = reader.records.occurrences[&(RepeatId(0), None)];
        assert_eq!(
            reader.records.lists[root]
                .starts
                .iter()
                .map(|p| p.offset)
                .collect::<Vec<_>>(),
            [7, 9]
        );
        for row in 0..2 {
            let parent = Some(RecordId {
                occurrence: root,
                row,
            });
            let ticks = reader.records.occurrences[&(RepeatId(1), parent)];
            let list = &reader.records.lists[ticks];
            assert!(list.complete);
            assert_eq!(list.starts.len(), 2);
            assert!(list.starts.iter().all(|p| p.offset == 7 + row * 2));
            let children = reader.records.occurrences.get(&(RepeatId(2), parent));
            if row == 0 {
                assert!(children.is_none());
            } else {
                let list = &reader.records.lists[*children.unwrap()];
                assert!(list.complete);
                assert!(list.starts.is_empty());
            }
        }
    }
}
