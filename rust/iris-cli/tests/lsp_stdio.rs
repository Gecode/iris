use lsp_server::{Message, Notification, Request};
use serde_json::{Value, json};
use std::{
    io::{BufReader, Read},
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(5);

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn iris_lsp_serves_framed_stdio_and_exits_cleanly() {
    let mut child = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_iris"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stdin = child.0.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.0.stdout.take().unwrap());
    let mut stderr = child.0.stderr.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        loop {
            let result = Message::read(&mut stdout).map_err(|error| error.to_string());
            let finished = !matches!(result, Ok(Some(_)));
            if sender.send(result).is_err() || finished {
                break;
            }
        }
    });
    let errors = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let receive = || {
        receiver
            .recv_timeout(TIMEOUT)
            .expect("timed out waiting for framed stdout")
            .expect("stdout must contain only LSP frames")
            .expect("unexpected stdout EOF")
    };
    Message::from(Request::new(
        1.into(),
        "initialize".into(),
        json!({"capabilities": {}}),
    ))
    .write(&mut stdin)
    .unwrap();
    let Message::Response(response) = receive() else {
        panic!("expected initialize response")
    };
    assert_eq!(response.id, 1.into());
    let capabilities = response.response_result.unwrap();
    assert_eq!(capabilities["capabilities"]["positionEncoding"], "utf-16");
    Message::from(Notification::new("initialized".into(), json!({})))
        .write(&mut stdin)
        .unwrap();
    Message::from(Notification::new("textDocument/didOpen".into(), json!({
        "textDocument": {"uri": "untitled:stdio", "languageId": "iris", "version": 1, "text": "x:unknown"}
    }))).write(&mut stdin).unwrap();
    let Message::Notification(notification) = receive() else {
        panic!("expected diagnostics")
    };
    assert_eq!(notification.method, "textDocument/publishDiagnostics");
    assert_eq!(notification.params["version"], 1);
    assert_eq!(
        notification.params["diagnostics"][0]["code"],
        "E-SPEC-UNKNOWN-TYPE"
    );
    Message::from(Request::new(2.into(), "shutdown".into(), Value::Null))
        .write(&mut stdin)
        .unwrap();
    let Message::Response(response) = receive() else {
        panic!("expected shutdown response")
    };
    assert_eq!(response.id, 2.into());
    assert_eq!(response.response_result.unwrap(), Value::Null);
    Message::from(Notification::new("exit".into(), ()))
        .write(&mut stdin)
        .unwrap();
    drop(stdin);
    assert!(receiver.recv_timeout(TIMEOUT).unwrap().unwrap().is_none());
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "server did not terminate after exit"
        );
        thread::yield_now();
    };
    assert!(status.success(), "{status}");
    reader.join().unwrap();
    assert!(errors.join().unwrap().is_empty());
}
