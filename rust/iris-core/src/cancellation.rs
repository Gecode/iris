//! Optional cooperative host cancellation, independent of language work budgets.
use crate::HostError;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}
#[derive(Clone, Debug, Default)]
pub struct ReadOptions {
    pub cancellation: Option<CancellationToken>,
}

pub(crate) const SCAN_CHUNK: usize = 64 * 1024;
pub(crate) const VALUE_CHUNK: usize = 1024;

// Internal loops can fail only by cancellation. Keep that boundary small;
// convert to the public host error once it leaves the interpreter.
#[derive(Debug)]
pub(crate) struct Cancelled;
impl From<Cancelled> for crate::Failure {
    fn from(_: Cancelled) -> Self {
        crate::Failure::Host(HostError::Cancelled)
    }
}

#[cfg(test)]
type CheckpointObserver = Box<dyn FnMut(usize)>;

#[derive(Default)]
pub(crate) struct Control {
    token: Option<CancellationToken>,
    #[cfg(test)]
    work: std::cell::Cell<usize>,
    #[cfg(test)]
    observer: std::cell::RefCell<Option<CheckpointObserver>>,
}
impl Control {
    pub fn new(options: &ReadOptions) -> Self {
        Self {
            token: options.cancellation.clone(),
            #[cfg(test)]
            work: std::cell::Cell::new(0),
            #[cfg(test)]
            observer: std::cell::RefCell::default(),
        }
    }
    #[inline]
    pub fn worked(&self, _count: usize) {
        #[cfg(test)]
        {
            self.work.set(self.work.get() + _count);
        }
    }
    #[cfg(test)]
    pub(crate) fn test_work(&self) -> usize {
        self.work.get()
    }
    pub fn is_active(&self) -> bool {
        self.token.is_some()
    }
    #[inline]
    pub fn check(&self) -> Result<(), Cancelled> {
        #[cfg(test)]
        if let Some(observer) = self.observer.borrow_mut().as_mut() {
            observer(self.work.get());
        }
        if self
            .token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
    /// Preserve the bulk path when cancellation is absent.
    pub fn chunk_size(&self) -> usize {
        if self.is_active() {
            SCAN_CHUNK
        } else {
            usize::MAX
        }
    }
    pub fn copy_text(&self, text: &str) -> Result<String, Cancelled> {
        if !self.is_active() {
            return Ok(text.to_owned());
        }
        let mut out = String::with_capacity(text.len());
        let mut start: usize = 0;
        while start < text.len() {
            self.check()?;
            let mut end = start.saturating_add(SCAN_CHUNK).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            self.worked(end - start);
            out.push_str(&text[start..end]);
            start = end;
        }
        self.check()?;
        Ok(out)
    }
    pub fn starts_with(&self, bytes: &[u8], prefix: &[u8]) -> Result<bool, Cancelled> {
        if !self.is_active() {
            return Ok(bytes.starts_with(prefix));
        }
        if prefix.len() > bytes.len() {
            return Ok(false);
        }
        for (a, b) in bytes[..prefix.len()]
            .chunks(self.chunk_size())
            .zip(prefix.chunks(self.chunk_size()))
        {
            self.check()?;
            self.worked(a.len());
            if a != b {
                return Ok(false);
            }
        }
        Ok(true)
    }
    pub fn equal_bytes(&self, left: &[u8], right: &[u8]) -> Result<bool, Cancelled> {
        Ok(left.len() == right.len() && self.starts_with(left, right)?)
    }
    pub fn find_byte(&self, bytes: &[u8], byte: u8) -> Result<Option<usize>, Cancelled> {
        for (index, chunk) in bytes.chunks(self.chunk_size()).enumerate() {
            self.check()?;
            if let Some(offset) = chunk.iter().position(|&b| b == byte) {
                self.worked(offset + 1);
                return Ok(Some(if self.is_active() {
                    index * SCAN_CHUNK + offset
                } else {
                    offset
                }));
            }
            self.worked(chunk.len());
        }
        Ok(None)
    }
    /// Appends `bytes` as text in checked chunks, for diagnostics that quote
    /// an input token of any length.
    pub fn push_text(&self, out: &mut String, bytes: &[u8]) -> Result<(), Cancelled> {
        let start = out.len();
        out.reserve(bytes.len());
        if self.utf8_parts(bytes, |part| out.push_str(part))?.is_err() {
            // Input admission rejects invalid UTF-8; keep lossy quoting anyway.
            out.truncate(start);
            out.push_str(&String::from_utf8_lossy(bytes));
        }
        Ok(())
    }
    pub fn text_message(&self, parts: &[&str]) -> Result<String, Cancelled> {
        let mut message = String::new();
        for part in parts {
            self.push_text(&mut message, part.as_bytes())?;
        }
        Ok(message)
    }
    pub fn quoted_alternatives(&self, alternatives: &[String]) -> Result<String, Cancelled> {
        let mut message = String::new();
        for (index, alternative) in alternatives.iter().enumerate() {
            if index > 0 {
                message.push_str(" or ");
            }
            message.push('\'');
            self.push_text(&mut message, alternative.as_bytes())?;
            message.push('\'');
        }
        Ok(message)
    }
    pub fn utf8(&self, bytes: &[u8]) -> Result<Result<(), usize>, Cancelled> {
        self.utf8_parts(bytes, |_| {})
    }
    pub fn copy_utf8(&self, bytes: &[u8]) -> Result<Result<String, usize>, Cancelled> {
        let mut text = String::with_capacity(bytes.len());
        Ok(self
            .utf8_parts(bytes, |part| text.push_str(part))?
            .map(|()| text))
    }
    fn utf8_parts(
        &self,
        bytes: &[u8],
        mut visit: impl FnMut(&str),
    ) -> Result<Result<(), usize>, Cancelled> {
        if !self.is_active() {
            return Ok(std::str::from_utf8(bytes)
                .map(&mut visit)
                .map_err(|e| e.valid_up_to()));
        }
        let mut start = 0;
        while start < bytes.len() {
            self.check()?;
            let end = start.saturating_add(SCAN_CHUNK).min(bytes.len());
            self.worked(end - start);
            match std::str::from_utf8(&bytes[start..end]) {
                Ok(text) => {
                    self.check()?;
                    visit(text);
                    start = end;
                }
                Err(e) if e.error_len().is_none() && end < bytes.len() => {
                    let valid_end = start + e.valid_up_to();
                    self.check()?;
                    let prefix =
                        std::str::from_utf8(&bytes[start..valid_end]).expect("validated prefix");
                    self.check()?;
                    visit(prefix);
                    start = valid_end;
                }
                Err(e) => return Ok(Err(start + e.valid_up_to())),
            }
        }
        self.check()?;
        Ok(Ok(()))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        Failure, Params, Spec, Value, input::Input, reader::Interpreter, value::LeafStorage,
    };
    use std::{cell::Cell, rc::Rc, sync::mpsc, thread, time::Duration};
    const TIMEOUT: Duration = Duration::from_secs(5);

    pub(crate) fn cancel_at<T>(
        checkpoint: usize,
        operation: impl FnOnce(Control) -> Result<T, Failure>,
    ) {
        let token = CancellationToken::default();
        let mut control = Control::new(&ReadOptions {
            cancellation: Some(token.clone()),
        });
        let (entered, wait_entered) = mpsc::sync_channel(1);
        let (resume, wait_resume) = mpsc::sync_channel(1);
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        *control.observer.get_mut() = Some(Box::new(move |_| {
            observed.set(observed.get() + 1);
            if observed.get() == checkpoint {
                entered.send(()).unwrap();
                wait_resume.recv_timeout(TIMEOUT).unwrap();
            }
        }));
        let controller = thread::spawn(move || {
            wait_entered
                .recv_timeout(TIMEOUT)
                .expect("operation did not reach its checkpoint");
            token.cancel();
            resume.send(()).unwrap();
        });
        let result = operation(control);
        controller.join().unwrap();
        assert!(matches!(result, Err(Failure::Host(HostError::Cancelled))));
        assert_eq!(
            calls.get(),
            checkpoint,
            "cancellation must stop at the gated checkpoint"
        );
    }
    #[test]
    fn cancellation_repeats_arrays_comprehensions_and_deferred_rejections() {
        for source in [
            "repeat 512 {let x=1}",
            "x[512]:int",
            "x[512]:int check all(i in x:i==1)",
        ] {
            let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
            let input = "1 ".repeat(512);
            cancel_at(100, |control| {
                let mut input = Input::new(input.as_bytes(), "input");
                input.control = control;
                Interpreter::new(&spec, input)?.read()
            });
            let token = CancellationToken::default();
            let params = Params::new();
            let bytes = if source.starts_with("repeat") {
                b"".as_slice()
            } else {
                input.as_bytes()
            };
            assert!(spec.read(bytes, "input", &params).is_ok());
            assert!(
                spec.read_with_options(
                    bytes,
                    "input",
                    &params,
                    &ReadOptions {
                        cancellation: Some(token)
                    }
                )
                .is_ok()
            );
        }
        let spec = Spec::parse(
            b"instances {x:int check x>0 repeat 512 {let y=1} name:word}",
            "spec",
        )
        .unwrap();
        let mut params = Params::new();
        params.set("instance", "2");
        cancel_at(100, |control| {
            let mut input = Input::new(b"-1 rejected 1 selected", "input");
            input.control = control;
            Interpreter::new(&spec, input)?.with_params(&params)?.read()
        });
    }
    #[test]
    fn cancellation_in_planned_list_after_deferred_rejection() {
        let spec = Spec::parse(
            b"instances {x:int check x>0 repeat 512 {list xs:int until 0} name:word}",
            "spec",
        )
        .unwrap();
        let input = format!(
            "-1 {}rejected 1 {}selected",
            "0 ".repeat(512),
            "0 ".repeat(512)
        );
        let mut params = Params::new();
        params.set("instance", "2");
        cancel_at(100, |control| {
            let mut input = Input::new(input.as_bytes(), "input");
            input.control = control;
            Interpreter::new(&spec, input)?.with_params(&params)?.read()
        });
        assert!(spec.read(input.as_bytes(), "input", &params).is_ok());
    }
    #[test]
    fn cancellation_empty_row_offsets_is_checked_inside_the_bulk_loop() {
        cancel_at(5, |control| {
            let mut value = Value::empty("int");
            value.set_dimensions_controlled(&[8192, 0], &control)?;
            Ok(value)
        });
    }
    #[test]
    fn cancellation_bulk_copy_aggregate_and_input_scan() {
        let spec = Spec::parse(b"x[8192]:int let y=x check sum(y)==8192", "spec").unwrap();
        // Populate before attaching the observer to isolate bulk copy/evaluation.
        let bytes = "1 ".repeat(8192);
        let mut reader = Interpreter::new(&spec, Input::new(bytes.as_bytes(), "input")).unwrap();
        let declaration = spec.parsed.syntax().first_child().unwrap();
        reader.execute(declaration.parent().unwrap()).unwrap();
        cancel_at(4, |control| {
            reader.input.control = control;
            let value = reader.lookup(spec.index.test_name("x").id).unwrap();
            value
                .root()
                .copy_with(&reader.input.control, |_| {})
                .map_err(Into::into)
        });
        cancel_at(10, |control| {
            reader.input.control = control;
            let check = spec.analysis.model.checks()[0].node.clone();
            reader.execute_check(&check)
        });
        cancel_at(3, |control| {
            let bytes = vec![b' '; 4 * SCAN_CHUNK];
            let mut input = Input::new(&bytes, "input");
            input.control = control;
            input.skip_to_data()?;
            Ok(())
        });
        cancel_at(3, |control| {
            let value = Value {
                leaves: LeafStorage::String(vec!["é".repeat(2 * SCAN_CHUNK)]),
                levels: vec![],
            };
            value.root().copy_with(&control, |_| {}).map_err(Into::into)
        });
    }
    #[test]
    fn cancellation_inside_comprehension_and_decimal_conversion() {
        let spec = Spec::parse(b"x[8192]:int check all(i in x:i==1)", "spec").unwrap();
        let bytes = "1 ".repeat(8192);
        let mut reader = Interpreter::new(&spec, Input::new(bytes.as_bytes(), "input")).unwrap();
        reader.execute(spec.parsed.syntax()).unwrap();
        cancel_at(100, |control| {
            reader.input.control = control;
            let check = spec.analysis.model.checks()[0].node.clone();
            reader.execute_check(&check)
        });
        let zeros = vec![b'0'; 4 * SCAN_CHUNK + 1];
        cancel_at(3, |control| {
            crate::numeric::parse_integer_controlled(&zeros, &control).map_err(Into::into)
        });
    }

    #[test]
    fn cancellation_long_names_use_interned_keys() {
        let long = "a".repeat(512 * SCAN_CHUNK);
        let source = format!("repeat 8 {{{long}:int let y={long}}}");
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let name = spec.index.test_name(&long);
        // A frame stores only an identity plus shared text. No name bytes are
        // scanned or copied by set/lookup/take, regardless of identifier size.
        let control = Control::new(&ReadOptions {
            cancellation: Some(CancellationToken::default()),
        });
        let mut frame = crate::value::Frame::default();
        frame
            .set_controlled(name.clone(), Value::int(7), &control)
            .unwrap();
        assert_eq!(frame.lookup(name.id).unwrap().root().as_int().unwrap(), 7);
        assert!(frame.take(name.id).is_some());
        assert_eq!(control.test_work(), 0);
        // Gate execution after construction, including publication and generic
        // expression lookup. The other thread cancels at the next checkpoint.
        let mut reader = Interpreter::new(&spec, Input::new(b"1 2 3 4 5 6 7 8", "input")).unwrap();
        cancel_at(20, |control| {
            reader.input.control = control;
            reader.read()
        });
        println!("32 MiB identifier: frame name-byte work=0; cancelled at checkpoint 20");
    }

    /// Counts the checkpoints of an uncancelled read of `source` on empty input.
    fn checkpoints(source: &[u8]) -> usize {
        let spec = Spec::parse(source, "spec").unwrap();
        let mut control = Control::new(&ReadOptions {
            cancellation: Some(CancellationToken::default()),
        });
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        *control.observer.get_mut() = Some(Box::new(move |_| observed.set(observed.get() + 1)));
        let mut input = Input::new(b"", "input");
        input.control = control;
        Interpreter::new(&spec, input).unwrap().read().unwrap();
        calls.get()
    }
    #[test]
    fn cancellation_inside_long_literals_and_quoted_tokens() {
        let long = 16 * SCAN_CHUNK;
        let padding = "0".repeat(long);
        for (literal, value) in [
            ("0", padding.clone()),
            ("0.5", format!("0.5{padding}")),
            ("\"a\"", format!("\"{}\"", "a".repeat(long))),
        ] {
            // The same spec length with the long text in a comment fixes the
            // spec-scanning checkpoints; past them lies the long literal's
            // conversion or copy.
            let checkpoint = checkpoints(format!("let x={literal}\n#{value}").as_bytes()) + 8;
            let spec =
                Spec::parse(format!("let x={value}\n#{literal}").as_bytes(), "spec").unwrap();
            cancel_at(checkpoint, |control| {
                let mut input = Input::new(b"", "input");
                input.control = control;
                Interpreter::new(&spec, input)?.read()
            });
        }
        let spec = Spec::parse(b"x:int", "spec").unwrap();
        let bytes = vec![b'a'; 4 * SCAN_CHUNK];
        let mut reader = Interpreter::new(&spec, Input::new(&bytes, "input")).unwrap();
        let token = reader.input.next_token().unwrap().unwrap();
        cancel_at(3, |control| {
            reader.input.control = control;
            reader.expected_int(&token).map_err(Into::into)
        });
    }
    /// Attach after interpreter construction to exclude the spec line scan.
    fn execution_checkpoints(source: &str, bytes: &[u8]) -> usize {
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(bytes, "input")).unwrap();
        let mut control = Control::new(&ReadOptions {
            cancellation: Some(CancellationToken::default()),
        });
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        *control.observer.get_mut() = Some(Box::new(move |_| observed.set(observed.get() + 1)));
        reader.input.control = control;
        let result = reader.read();
        assert!(matches!(result, Err(Failure::Language(_))));
        calls.get()
    }
    #[test]
    fn cancellation_while_quoting_spec_diagnostics() {
        let long = "a".repeat(16 * SCAN_CHUNK);
        for (template, bytes) in [
            ("expect \"TEXT\"", b"".as_slice()),
            ("expect \"TEXT\"", b"x".as_slice()),
            ("check 0 \"TEXT\"", b"".as_slice()),
            ("sections {\"TEXT\" {}}", b"x\n".as_slice()),
            ("line \"TEXT\"", b"x\n".as_slice()),
            ("find \"TEXT\"", b"x\n".as_slice()),
            ("repeat until \"TEXT\" {x:int}", b"".as_slice()),
        ] {
            let short = template.replace("TEXT", "a");
            let source = template.replace("TEXT", &long);
            let early = execution_checkpoints(&short, bytes) + 3;
            let late = if template.starts_with("check") {
                early + 2 * (long.len() / SCAN_CHUNK - 1)
            } else {
                execution_checkpoints(&source, bytes) - 3
            };
            // Early checkpoints lie inside quoting, and the late check covers
            // the second copy retained as a check's rejection summary.
            for checkpoint in [early, late] {
                let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
                let mut reader = Interpreter::new(&spec, Input::new(bytes, "input")).unwrap();
                cancel_at(checkpoint, |control| {
                    reader.input.control = control;
                    reader.read()
                });
            }
        }
    }
    #[test]
    fn cancellation_at_read_boundary_replaces_language_failure() {
        let token = CancellationToken::default();
        let options = ReadOptions {
            cancellation: Some(token.clone()),
        };
        let spec = Spec::parse(b"expect \"a\"", "spec").unwrap();
        let failure = || spec.read(b"", "input", &Params::new());
        assert!(matches!(
            Spec::finish_read(failure(), &options),
            Err(Failure::Language(_))
        ));
        token.cancel();
        assert!(matches!(
            Spec::finish_read(failure(), &options),
            Err(Failure::Host(HostError::Cancelled))
        ));
        // Preserve a host failure even when the token is cancelled.
        assert!(matches!(
            Spec::finish_read(Err(HostError::SourceTooLarge { len: 1 }.into()), &options),
            Err(Failure::Host(HostError::SourceTooLarge { len: 1 }))
        ));
    }
    #[test]
    fn cancellation_precancelled_admission_and_utf8_chunk_boundaries() {
        let spec = Spec::parse(b"x:rest", "spec").unwrap();
        let token = CancellationToken::default();
        token.cancel();
        assert!(matches!(
            spec.read_with_options(
                b"abc",
                "input",
                &Params::new(),
                &ReadOptions {
                    cancellation: Some(token)
                }
            ),
            Err(Failure::Host(HostError::Cancelled))
        ));
        let control = Control::new(&ReadOptions {
            cancellation: Some(CancellationToken::default()),
        });
        for prefix in [SCAN_CHUNK - 3, SCAN_CHUNK - 2, SCAN_CHUNK - 1, SCAN_CHUNK] {
            let mut bytes = vec![b'a'; prefix];
            bytes.extend_from_slice("😀é".as_bytes());
            assert_eq!(control.utf8(&bytes).unwrap(), Ok(()));
            bytes.push(0xff);
            assert_eq!(control.utf8(&bytes).unwrap(), Err(prefix + 6));
            let mut params = Params::new();
            params.set("unknown", "x");
            let Err(Failure::Language(error)) = spec.read_with_options(
                &bytes,
                "input",
                &params,
                &ReadOptions {
                    cancellation: Some(CancellationToken::default()),
                },
            ) else {
                panic!("unknown parameter precedes UTF-8")
            };
            assert_eq!(error.code, crate::ErrorCode::ParamUnknown);
        }
    }
}

#[cfg(test)]
mod scaling_tests {
    use super::*;
    use crate::{Value, input::Input};
    #[test]
    fn cancellable_scans_copies_and_empty_offsets_scale_linearly() {
        let measure = |n: usize| {
            let bytes = vec![b'a'; n];
            let mut input = Input::new(&bytes, "input");
            input.control = Control::new(&ReadOptions {
                cancellation: Some(CancellationToken::default()),
            });
            assert_eq!(input.next_token().unwrap().unwrap().bytes.len(), n);
            let mut value = Value::empty("int");
            value
                .set_dimensions_controlled(&[n, 0], &input.control)
                .unwrap();
            input
                .control
                .copy_text(std::str::from_utf8(&bytes).unwrap())
                .unwrap();
            input.control.find_byte(&bytes, b'z').unwrap();
            assert!(input.control.equal_bytes(&bytes, &bytes).unwrap());
            assert_eq!(input.control.utf8(&bytes).unwrap(), Ok(()));
            input.control.work.get()
        };
        let small = measure(SCAN_CHUNK);
        let large = measure(4 * SCAN_CHUNK);
        assert!(large <= 6 * small, "{small} -> {large}");
        assert!(large <= 9 * 4 * SCAN_CHUNK);
    }
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    fn measure(limit: usize, operation: impl FnOnce(Control)) {
        let mut control = Control::new(&ReadOptions {
            cancellation: Some(CancellationToken::default()),
        });
        let maximum = Rc::new(Cell::new(0));
        let observed = maximum.clone();
        let mut previous = 0;
        *control.observer.get_mut() = Some(Box::new(move |work| {
            observed.set(observed.get().max(work - previous));
            previous = work;
        }));
        operation(control);
        println!("maximum checkpoint gap: {} (limit {limit})", maximum.get());
        assert!(maximum.get() > 0 && maximum.get() <= limit);
    }

    #[test]
    fn planned_list_checkpoint_gaps() {
        use crate::{Spec, input::Input, reader::Interpreter};
        for input in [
            format!("{} 1 0", "0".repeat(5 * SCAN_CHUNK + 19)),
            format!("{}0", "1 ".repeat(8 * VALUE_CHUNK + 7)),
        ] {
            let spec =
                Spec::parse(b"repeat 1 {list xs:int until 0} ignore trailing", "spec").unwrap();
            measure(SCAN_CHUNK, |control| {
                let mut input = Input::new(input.as_bytes(), "input");
                input.control = control;
                Interpreter::new(&spec, input).unwrap().read().unwrap();
            });
        }
        measure(VALUE_CHUNK, |control| {
            use crate::value::{Column, Frame, LeafStorage, Name};
            let mut frame = Frame::default();
            for i in 0..(5 * VALUE_CHUNK + 7) {
                frame.set(Name::new(i, &format!("n{i}")), crate::Value::int(1));
            }
            frame.clear(&control).unwrap();
            let mut column = Column::new("int", 0);
            for _ in 0..2 {
                let row = crate::Value {
                    leaves: LeafStorage::Int(vec![1; 5 * VALUE_CHUNK + 7]),
                    levels: vec![],
                };
                assert!(column.add_row_reusing(Some(row), &control).is_ok());
            }
            control.check().unwrap();
        });
    }
}
