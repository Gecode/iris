use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    InitializeParams, InitializeResult, InitializedParams, PublishDiagnosticsParams,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem, Uri,
    VersionedTextDocumentIdentifier,
    notification::{DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Initialized},
};
use serde_json::Value;
use std::{
    collections::VecDeque,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const TIMEOUT: Duration = Duration::from_secs(5);

pub struct Client {
    connection: Option<Connection>,
    server: Option<JoinHandle<Result<(), String>>>,
    pending: VecDeque<Message>,
    next_id: i32,
    pub initialization: InitializeResult,
}

impl Client {
    pub fn new(capabilities: Value) -> Self {
        let (connection, server_connection) = Connection::memory();
        let server = thread::spawn(move || {
            iris_lsp::run(server_connection).map_err(|error| error.to_string())
        });
        let mut client = Self {
            connection: Some(connection),
            server: Some(server),
            pending: VecDeque::new(),
            next_id: 1,
            initialization: InitializeResult::default(),
        };
        let params: InitializeParams =
            serde_json::from_value(serde_json::json!({"capabilities": capabilities})).unwrap();
        let response = client.request("initialize", serde_json::to_value(params).unwrap());
        client.initialization = serde_json::from_value(response.response_result.unwrap()).unwrap();
        client.notify::<Initialized>(InitializedParams {});
        client
    }

    pub fn notify<N: lsp_types::notification::Notification>(&self, params: N::Params) {
        self.send(Notification::new(N::METHOD.into(), params).into());
    }

    pub fn send(&self, message: Message) {
        self.connection
            .as_ref()
            .unwrap()
            .sender
            .send(message)
            .unwrap();
    }

    pub fn request(&mut self, method: &str, params: Value) -> Response {
        let id = self.next_id;
        self.next_id += 1;
        self.send(Request::new(id.into(), method.into(), params).into());
        match self.receive_until(
            |message| matches!(message, Message::Response(response) if response.id == id.into()),
        ) {
            Message::Response(response) => response,
            _ => unreachable!(),
        }
    }

    pub fn open(&self, uri: &Uri, version: i32, text: &str) {
        self.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(uri.clone(), "iris".into(), version, text.into()),
        });
    }

    pub fn change(
        &self,
        uri: &Uri,
        version: i32,
        content_changes: Vec<TextDocumentContentChangeEvent>,
    ) {
        self.notify::<DidChangeTextDocument>(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(uri.clone(), version),
            content_changes,
        });
    }

    pub fn close(&self, uri: &Uri) {
        self.notify::<DidCloseTextDocument>(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier::new(uri.clone()),
        });
    }

    pub fn diagnostics(&mut self, uri: &Uri, version: Option<i32>) -> PublishDiagnosticsParams {
        let message = self.receive_until(|message| diagnostics_match(message, uri, version));
        match message {
            Message::Notification(notification) => {
                serde_json::from_value(notification.params).unwrap()
            }
            _ => unreachable!(),
        }
    }

    pub fn pending_diagnostic_count(&self, uri: &Uri, version: Option<i32>) -> usize {
        self.pending
            .iter()
            .filter(|message| diagnostics_match(message, uri, version))
            .count()
    }

    /// Keep unmatched messages so requests never hide another document's diagnostics.
    fn receive_until(&mut self, predicate: impl Fn(&Message) -> bool) -> Message {
        if let Some(index) = self.pending.iter().position(&predicate) {
            return self.pending.remove(index).unwrap();
        }
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let message = self
                .connection
                .as_ref()
                .unwrap()
                .receiver
                .recv_deadline(deadline)
                .expect("timed out waiting for LSP message");
            if predicate(&message) {
                return message;
            }
            self.pending.push_back(message);
        }
    }

    pub fn exit_without_shutdown(mut self) -> String {
        self.send(Notification::new("exit".into(), ()).into());
        self.connection.take();
        self.server.take().unwrap().join().unwrap().unwrap_err()
    }

    pub fn shutdown(mut self) {
        let response = self.request("shutdown", Value::Null);
        assert_eq!(response.response_result.unwrap(), Value::Null);
        // Requests after shutdown are rejected and cannot schedule more work.
        assert_eq!(
            self.request("unsupported", Value::Null)
                .response_result
                .unwrap_err()
                .code,
            -32600
        );
        self.send(Notification::new("exit".into(), ()).into());
        self.connection.take();
        assert_eq!(self.server.take().unwrap().join().unwrap(), Ok(()));
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        // Disconnect first, including during assertion unwinding, so the server
        // cannot remain blocked waiting for initialized, another edit, or exit.
        self.connection.take();
        if let Some(server) = self.server.take() {
            let _ = server.join();
        }
    }
}

fn diagnostics_match(message: &Message, uri: &Uri, version: Option<i32>) -> bool {
    let Message::Notification(notification) = message else {
        return false;
    };
    notification.method == "textDocument/publishDiagnostics"
        && notification.params["uri"] == uri.as_str()
        && notification.params["version"] == serde_json::to_value(version).unwrap()
}
