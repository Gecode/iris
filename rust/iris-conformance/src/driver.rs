use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::case::{Case, Operation, OutputFormat};

#[derive(Debug)]
pub struct DriverCommand {
    words: Vec<String>,
}

#[derive(Debug)]
pub struct DriverOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub struct DriverFailure {
    pub message: String,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl DriverCommand {
    pub fn parse(command: &str) -> Result<Self, String> {
        let words = shell_words::split(command).map_err(|error| error.to_string())?;
        if words.first().is_none_or(String::is_empty) {
            return Err("driver command must name an executable".into());
        }
        Ok(Self { words })
    }

    pub fn invoke(
        &self,
        case: &Case,
        version: &str,
        timeout: Duration,
    ) -> Result<DriverOutput, DriverFailure> {
        let mut command = Command::new(&self.words[0]);
        command
            .args(&self.words[1..])
            .arg(case.operation.as_str())
            .arg(&case.spec);
        if let Some(input) = &case.input {
            command.arg(input);
        }
        if let Some(params) = &case.params {
            command.arg("--params").arg(params);
        }
        command.arg("--version").arg(version);
        if case.operation == Operation::Catalog {
            command
                .arg("--format")
                .arg(if case.output == OutputFormat::Dzn {
                    "dzn"
                } else {
                    "json"
                });
        }
        capture(&mut command, timeout)
    }
}

#[derive(Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

enum PipeEvent {
    Bytes(Stream, Vec<u8>),
    Finished(Stream, Result<(), String>),
}

fn spawn_pipe_reader(
    mut pipe: impl Read + Send + 'static,
    stream: Stream,
    sender: mpsc::Sender<PipeEvent>,
) {
    thread::spawn(move || {
        let mut buffer = [0; 8192];
        let result = loop {
            match pipe.read(&mut buffer) {
                Ok(0) => break Ok(()),
                Ok(count) => {
                    if sender
                        .send(PipeEvent::Bytes(stream, buffer[..count].to_vec()))
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => break Err(error.to_string()),
            }
        };
        let _ = sender.send(PipeEvent::Finished(stream, result));
    });
}

fn capture(command: &mut Command, timeout: Duration) -> Result<DriverOutput, DriverFailure> {
    let started = Instant::now();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| DriverFailure {
            message: format!("could not spawn driver: {error}"),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })?;
    let (sender, receiver) = mpsc::channel();
    spawn_pipe_reader(
        child.stdout.take().expect("stdout configured as piped"),
        Stream::Stdout,
        sender.clone(),
    );
    spawn_pipe_reader(
        child.stderr.take().expect("stderr configured as piped"),
        Stream::Stderr,
        sender,
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut finished = [false; 2];
    let mut status = None;
    let failure = loop {
        match child.try_wait() {
            Ok(result) => status = result.or(status),
            Err(error) => break format!("could not wait for driver: {error}"),
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            break format!(
                "timeout after {} seconds (including pipe completion)",
                timeout.as_secs_f64()
            );
        }
        if let Some(status) = status
            && finished == [true, true]
        {
            return Ok(DriverOutput {
                status,
                stdout,
                stderr,
            });
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(5))) {
            Ok(PipeEvent::Bytes(Stream::Stdout, bytes)) => stdout.extend(bytes),
            Ok(PipeEvent::Bytes(Stream::Stderr, bytes)) => stderr.extend(bytes),
            Ok(PipeEvent::Finished(stream, result)) => {
                let index = match stream {
                    Stream::Stdout => 0,
                    Stream::Stderr => 1,
                };
                finished[index] = true;
                if let Err(error) = result {
                    break format!(
                        "could not read {}: {error}",
                        if index == 0 { "stdout" } else { "stderr" }
                    );
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                // Both streams have completed; avoid spinning while the child exits.
                thread::sleep(remaining.min(Duration::from_millis(5)));
            }
        }
    };
    let mut message = failure;
    if status.is_none() {
        if let Err(error) = child.kill() {
            message.push_str(&format!("; could not kill direct child: {error}"));
        }
        if let Err(error) = child.wait() {
            message.push_str(&format!("; could not reap direct child: {error}"));
        }
    }
    // Reader threads own their pipes. Never join a descendant-held pipe after the deadline.
    Err(DriverFailure {
        message,
        stdout,
        stderr,
    })
}
