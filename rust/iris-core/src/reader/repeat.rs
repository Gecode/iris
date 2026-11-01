use super::*;
use crate::value::Column;
use iris_sema::{Possibilities, ValueType};

pub(super) struct RepeatState {
    node: SyntaxNode,
    pub body: SyntaxNode,
    mode: Mode,
    columns: Vec<(String, Column)>,
    row: usize,
    before: Option<usize>,
    occurrence: usize,
}
enum Mode {
    Count(usize),
    Eof,
    Tokens {
        alternatives: Vec<String>,
        store: Option<String>,
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
struct RecordId {
    occurrence: usize,
    row: usize,
}
struct RecordList {
    starts: Vec<Position>,
    complete: bool,
}
#[derive(Default)]
pub(super) struct RecordLists {
    lists: Vec<RecordList>,
    occurrences: HashMap<(RepeatId, Option<RecordId>), usize>,
}
impl RecordLists {
    fn begin(&mut self, repeat: RepeatId, parent: Option<RecordId>) -> usize {
        let index = self.lists.len();
        self.lists.push(RecordList {
            starts: vec![],
            complete: false,
        });
        self.occurrences.insert((repeat, parent), index);
        index
    }
}

impl Interpreter<'_, '_> {
    pub(super) fn begin_repeat(&mut self, node: &SyntaxNode) -> Result<(), Failure> {
        let mode = self.repeat_mode(node)?;
        let id = self.spec.index.repeats[&node.text_range()];
        let columns = self.repeat_columns(id);
        let parent = self.repeats.last().map(|repeat| RecordId {
            occurrence: repeat.occurrence,
            row: repeat.row,
        });
        let occurrence = self.records.begin(id, parent);
        self.repeats.push(RepeatState {
            node: node.clone(),
            body: child(node, K::Block),
            mode,
            columns,
            row: 0,
            before: None,
            occurrence,
        });
        Ok(())
    }
    fn repeat_mode(&mut self, node: &SyntaxNode) -> Result<Mode, Failure> {
        let own_tokens = tokens(node);
        Ok(if own_tokens.get(1).is_some_and(|t| t.text() == "until") {
            let terminators = node.children().find(|n| n.kind() == K::Terminators);
            let terminator_tokens = terminators.as_ref().map(tokens).unwrap_or_default();
            let alternatives: Vec<_> = terminator_tokens
                .iter()
                .filter(|t| t.kind() == K::String)
                .map(|t| DecodedString::new(t.text()).text)
                .collect();
            if alternatives.is_empty() {
                Mode::Eof
            } else {
                let store = terminator_tokens
                    .iter()
                    .position(|t| t.text() == "store")
                    .map(|i| terminator_tokens[i + 1].text().to_owned());
                Mode::Tokens {
                    alternatives,
                    store,
                }
            }
        } else {
            Mode::Count(self.admit_count(&expression(node), "repeat count")?)
        })
    }
    fn repeat_columns(&self, id: RepeatId) -> Vec<(String, Column)> {
        let info = &self.spec.analysis.model.repeats()[id.0];
        info.fields
            .iter()
            .map(|field| {
                let Possibilities::Known(depths) = &field.fallback.depths else {
                    unreachable!("valid repeat field has a syntactic fallback depth")
                };
                let Possibilities::Known(types) = &field.fallback.value_types else {
                    unreachable!("valid repeat field has a syntactic fallback type")
                };
                let kind = match types[0] {
                    ValueType::Int => "int",
                    ValueType::Float => "float",
                    ValueType::String => "string",
                };
                (field.name.clone(), Column::new(kind, depths[0]))
            })
            .collect()
    }
    pub(super) fn begin_iteration(&mut self) -> Result<bool, Failure> {
        let repeat = self.repeats.last().expect("active repeat");
        match &repeat.mode {
            Mode::Count(count) if repeat.row == *count => return Ok(false),
            Mode::Count(_) => {}
            Mode::Eof => {
                self.input.skip_to_data();
                if self.input.peek_byte().is_none() {
                    return Ok(false);
                }
            }
            Mode::Tokens {
                alternatives,
                store,
            } => {
                self.input.skip_to_data();
                let Some(token) = self.input.peek_token() else {
                    let expected = alternatives
                        .iter()
                        .map(|s| format!("'{s}'"))
                        .collect::<Vec<_>>()
                        .join(" or ");
                    return Err(self
                        .error(
                            ErrorCode::RepeatTerminator,
                            format!("expected {expected} before end of input"),
                        )
                        .into());
                };
                if alternatives.iter().any(|s| s.as_bytes() == token.bytes) {
                    self.input.next_token();
                    if let Some(name) = store {
                        self.frames.last_mut().expect("enclosing frame").set(
                            name.clone(),
                            Value::string(
                                String::from_utf8(token.bytes.to_vec())
                                    .expect("whole input is valid UTF-8"),
                            ),
                        );
                    }
                    return Ok(false);
                }
            }
        }
        let start = self.input.next_data_position();
        let repeat = self.repeats.last_mut().expect("active repeat");
        self.records.lists[repeat.occurrence].starts.push(start);
        repeat.before = Some(self.input.position().offset);
        self.frames.push(Frame::default());
        Ok(true)
    }
    pub(super) fn collect_iteration(&mut self) -> Result<(), Failure> {
        let repeat = self.repeats.last().expect("active iteration");
        if repeat.before == Some(self.input.position().offset) {
            if !matches!(repeat.mode, Mode::Count(_)) {
                return Err(self
                    .error(
                        ErrorCode::NoProgress,
                        "the body of a repeat until read no input",
                    )
                    .into());
            }
            self.charge_idle(1)?;
        }
        // Keep the iteration and last body statement live during row admission.
        let mut columns =
            std::mem::take(&mut self.repeats.last_mut().expect("active repeat").columns);
        for (name, column) in &mut columns {
            let value = self.frames.last_mut().expect("local frame").take(name);
            column
                .add_row(value)
                .map_err(|code| self.column_error(code, name, false))?;
        }
        self.frames.pop();
        let repeat = self.repeats.last_mut().expect("active repeat");
        repeat.columns = columns;
        repeat.row += 1;
        repeat.before = None;
        let node = repeat.node.clone();
        self.select_statement(&node);
        Ok(())
    }
    pub(super) fn finish_repeat(&mut self) -> Result<(), Failure> {
        let repeat = self.repeats.pop().expect("finishing repeat");
        for (name, column) in repeat.columns {
            let value = column
                .finish()
                .map_err(|code| self.column_error(code, &name, true))?;
            self.frames
                .last_mut()
                .expect("enclosing frame")
                .set(name, value);
        }
        self.records.lists[repeat.occurrence].complete = true;
        Ok(())
    }
    fn column_error(&self, code: ErrorCode, name: &str, finishing: bool) -> Error {
        let detail = match code {
            ErrorCode::RepeatMissing if finishing => "is defined in no repetition",
            ErrorCode::RepeatMissing => "is defined in only some repetitions",
            ErrorCode::RepeatDepth => "has rows of different depths",
            ErrorCode::ValueTypeMix => "mixes strings and numbers",
            _ => unreachable!("column construction error"),
        };
        self.error(code, format!("'{name}' {detail}"))
    }
    fn admit_count(&mut self, node: &SyntaxNode, description: &str) -> Result<usize, Failure> {
        let value = self.eval(node)?;
        let integer = value.root().as_int().map_err(|_| {
            self.error(
                ErrorCode::Count,
                format!("{description} must be an integer"),
            )
        })?;
        if integer < 0 {
            return Err(self
                .error(
                    ErrorCode::Count,
                    format!("{description} is negative ({integer})"),
                )
                .into());
        }
        if integer > i32::MAX as i64 {
            return Err(self
                .error(
                    ErrorCode::Count,
                    format!("{description} is too large ({integer})"),
                )
                .into());
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
        let token = self.input.next_token().ok_or_else(|| {
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
            let mut error = self.error_at(
                ErrorCode::IndexMismatch,
                format!("expected index {expected}, found {value}"),
                token.start,
            );
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
        let mut reader = Interpreter::new(&spec, Input::new(b" # tail\n  ", "input"));
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
        assert!(reader.frames[0].lookup("rows").is_none());
        assert!(reader.frames[0].lookup("row").is_none());

        let spec = Spec::parse(b"comment \"#\" repeat 2 as rows {each row repeat 2 as ticks {each tick} k:int if k {repeat 0 as children {each child}}}", "spec").unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"# skip\n0\n1", "input"));
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
