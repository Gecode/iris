use crate::{
    Result, assist, diagnostic,
    document::{Document, PositionEncoding, Positions},
    formatting, navigation, sample,
    sample_worker::Worker,
    semantic_tokens, structure,
};
use crossbeam_channel::{after, never, select};
use iris_syntax::ParseError;
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::{
    CompletionOptions, FoldingRangeProviderCapability, HoverProviderCapability, InitializeParams,
    InitializeResult, MarkupKind, OneOf, PositionEncodingKind, PublishDiagnosticsParams,
    RenameOptions, SemanticTokensFullOptions, SemanticTokensOptions,
    SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo, SymbolKind,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
    WorkDoneProgressOptions,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, PublishDiagnostics,
    },
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::Instant,
};

pub(crate) fn run(connection: Connection) -> Result<()> {
    run_with_executor(connection, sample::execute)
}

fn run_with_executor(
    connection: Connection,
    execute: impl Fn(sample::Job) -> sample::Batch + Send + 'static,
) -> Result<()> {
    let (encoding, structure_options, assist_options, refresh_supported) = initialize(&connection)?;
    Server {
        connection,
        encoding,
        structure_options,
        assist_options,
        documents: HashMap::new(),
        shutdown_requested: false,
        worker: Worker::new(execute),
        pending: VecDeque::new(),
        queued: HashSet::new(),
        generation: 0,
        refresh_supported,
        next_request: 0,
    }
    .run()
}

fn initialize(
    connection: &Connection,
) -> Result<(PositionEncoding, structure::Options, assist::Options, bool)> {
    let (id, params) = connection.initialize_start()?;
    let params: InitializeParams = match serde_json::from_value(params) {
        Ok(params) => params,
        Err(error) => {
            connection.sender.send(
                Response::new_err(id, ErrorCode::InvalidParams as i32, error.to_string()).into(),
            )?;
            return Err(error.into());
        }
    };
    let supports_utf8 = params
        .capabilities
        .general
        .as_ref()
        .and_then(|general| general.position_encodings.as_ref())
        .is_some_and(|encodings| encodings.contains(&PositionEncodingKind::UTF8));
    let encoding = if supports_utf8 {
        PositionEncoding::Utf8
    } else {
        PositionEncoding::Utf16
    };
    let text_document = params.capabilities.text_document.as_ref();
    let structure_options = structure::Options {
        hierarchical_symbols: text_document
            .and_then(|c| c.document_symbol.as_ref())
            .and_then(|c| c.hierarchical_document_symbol_support)
            .unwrap_or(false),
        line_folding_only: text_document
            .and_then(|c| c.folding_range.as_ref())
            .and_then(|c| c.line_folding_only)
            .unwrap_or(false),
        key_symbols: text_document
            .and_then(|c| c.document_symbol.as_ref())
            .and_then(|c| c.symbol_kind.as_ref())
            .and_then(|c| c.value_set.as_ref())
            .is_some_and(|kinds| kinds.contains(&SymbolKind::KEY)),
    };
    let assist_options = assist::Options {
        hover_markdown: prefers_markdown(
            text_document
                .and_then(|c| c.hover.as_ref())
                .and_then(|c| c.content_format.as_deref()),
        ),
        completion_markdown: prefers_markdown(
            text_document
                .and_then(|c| c.completion.as_ref())
                .and_then(|c| c.completion_item.as_ref())
                .and_then(|c| c.documentation_format.as_deref()),
        ),
        snippets: text_document
            .and_then(|c| c.completion.as_ref())
            .and_then(|c| c.completion_item.as_ref())
            .and_then(|c| c.snippet_support)
            .unwrap_or(false),
    };
    let refresh_supported = params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|c| c.inlay_hint.as_ref())
        .and_then(|c| c.refresh_support)
        .unwrap_or(false);
    let result = InitializeResult {
        capabilities: ServerCapabilities {
            position_encoding: Some(encoding.protocol_kind()),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(CompletionOptions::default()),
            inlay_hint_provider: Some(OneOf::Left(true)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
            document_formatting_provider: Some(OneOf::Left(true)),
            document_range_formatting_provider: Some(OneOf::Left(true)),
            document_symbol_provider: Some(OneOf::Left(true)),
            folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: semantic_tokens::legend(),
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                    ..SemanticTokensOptions::default()
                }),
            ),
            rename_provider: Some(OneOf::Right(RenameOptions {
                prepare_provider: Some(true),
                work_done_progress_options: WorkDoneProgressOptions::default(),
            })),
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::INCREMENTAL),
                    ..TextDocumentSyncOptions::default()
                },
            )),
            ..ServerCapabilities::default()
        },
        server_info: Some(ServerInfo {
            name: "iris".into(),
            version: Some(env!("CARGO_PKG_VERSION").into()),
        }),
    };
    connection.initialize_finish(id, serde_json::to_value(result)?)?;
    Ok((
        encoding,
        structure_options,
        assist_options,
        refresh_supported,
    ))
}

fn prefers_markdown(formats: Option<&[MarkupKind]>) -> bool {
    formats.and_then(|formats| {
        formats
            .iter()
            .find(|format| **format == MarkupKind::Markdown || **format == MarkupKind::PlainText)
    }) == Some(&MarkupKind::Markdown)
}

struct Server {
    connection: Connection,
    encoding: PositionEncoding,
    structure_options: structure::Options,
    assist_options: assist::Options,
    documents: HashMap<Uri, Document>,
    shutdown_requested: bool,
    worker: Worker,
    pending: VecDeque<Uri>,
    queued: HashSet<Uri>,
    generation: u64,
    refresh_supported: bool,
    next_request: u64,
}

impl Server {
    fn run(mut self) -> Result<()> {
        loop {
            if !self.shutdown_requested {
                self.publish_due_diagnostics(Instant::now())?;
            }
            let deadline = self
                .documents
                .values()
                .filter_map(|document| document.deadline)
                .min();
            self.dispatch_sample()?;
            let timeout = deadline.map_or_else(never, |deadline| {
                after(deadline.saturating_duration_since(Instant::now()))
            });
            let results = if self.shutdown_requested {
                never()
            } else {
                self.worker.results().clone()
            };
            let message = select! {
                recv(self.connection.receiver) -> message => message.map_err(|_| "LSP connection closed before exit")?,
                recv(results) -> result => {
                    self.receive_sample(result.map_err(|_| "sample worker disconnected")?)?;
                    continue;
                }
                recv(timeout) -> _ => continue,
            };
            match message {
                Message::Request(request) => self.handle_request(request)?,
                Message::Notification(notification) if notification.method == "exit" => {
                    return if self.shutdown_requested {
                        Ok(())
                    } else {
                        Err("LSP exit without shutdown".into())
                    };
                }
                Message::Notification(notification) if !self.shutdown_requested => {
                    self.handle_notification(notification)?
                }
                Message::Notification(_) | Message::Response(_) => {}
            }
        }
    }

    fn handle_request(&mut self, request: Request) -> Result<()> {
        let response = if self.shutdown_requested {
            Response::new_err(
                request.id,
                ErrorCode::InvalidRequest as i32,
                "server has shut down".into(),
            )
        } else if request.method == "shutdown" {
            match serde_json::from_value::<()>(request.params) {
                Ok(()) => {
                    self.shutdown_requested = true;
                    for document in self.documents.values_mut() {
                        document.samples.cancel();
                    }
                    self.documents.clear();
                    self.pending.clear();
                    self.queued.clear();
                    self.worker.stop();
                    Response::new_ok(request.id, ())
                }
                Err(error) => Response::new_err(
                    request.id,
                    ErrorCode::InvalidParams as i32,
                    error.to_string(),
                ),
            }
        } else if matches!(
            request.method.as_str(),
            "textDocument/definition"
                | "textDocument/references"
                | "textDocument/prepareRename"
                | "textDocument/rename"
        ) {
            navigation::handle_request(request, &mut self.documents, self.encoding)
        } else if matches!(
            request.method.as_str(),
            "textDocument/documentSymbol"
                | "textDocument/foldingRange"
                | "textDocument/semanticTokens/full"
        ) {
            structure::handle_request(
                request,
                &mut self.documents,
                self.encoding,
                &self.structure_options,
            )
        } else if matches!(
            request.method.as_str(),
            "textDocument/formatting" | "textDocument/rangeFormatting"
        ) {
            formatting::handle_request(request, &self.documents, self.encoding)
        } else if matches!(
            request.method.as_str(),
            "textDocument/hover" | "textDocument/completion" | "textDocument/inlayHint"
        ) {
            assist::handle_request(
                request,
                &mut self.documents,
                self.encoding,
                &self.assist_options,
            )
        } else {
            Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unsupported method: {}", request.method),
            )
        };
        self.connection.sender.send(response.into())?;
        Ok(())
    }

    fn handle_notification(&mut self, notification: Notification) -> Result<()> {
        match notification.method.as_str() {
            "textDocument/didOpen" => {
                if let Some(params) =
                    decode_notification::<DidOpenTextDocument>(notification.params)
                {
                    let item = params.text_document;
                    if let Some(previous) = self.documents.get_mut(&item.uri)
                        && previous.samples.cancel()
                    {
                        self.refresh_hints()?;
                    }
                    self.generation += 1;
                    let mut document = Document::new(item.text, item.version);
                    document.generation = self.generation;
                    self.documents.insert(item.uri.clone(), document);
                    self.prepare_diagnostics(&item.uri)?;
                }
            }
            "textDocument/didChange" => {
                if let Some(params) =
                    decode_notification::<DidChangeTextDocument>(notification.params)
                    && let Some(document) = self.documents.get_mut(&params.text_document.uri)
                {
                    let previous = document.version;
                    match document.change(
                        params.text_document.version,
                        params.content_changes,
                        self.encoding,
                        Instant::now(),
                    ) {
                        Ok(()) if document.version != previous => {
                            self.generation += 1;
                            document.generation = self.generation;
                            let refresh = document.samples.cancel();
                            if refresh {
                                self.refresh_hints()?;
                            }
                        }
                        Ok(()) => {}
                        Err(error) => eprintln!(
                            "iris lsp: invalid edit for {}: {error}",
                            params.text_document.uri.as_str()
                        ),
                    }
                }
            }

            "textDocument/didClose" => {
                if let Some(params) =
                    decode_notification::<DidCloseTextDocument>(notification.params)
                {
                    if let Some(mut document) = self.documents.remove(&params.text_document.uri) {
                        document.samples.cancel();
                    }
                    self.pending.retain(|uri| uri != &params.text_document.uri);
                    self.queued.remove(&params.text_document.uri);
                    self.send_diagnostics(PublishDiagnosticsParams::new(
                        params.text_document.uri,
                        vec![],
                        None,
                    ))?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn publish_due_diagnostics(&mut self, now: Instant) -> Result<()> {
        let due: Vec<Uri> = self
            .documents
            .iter()
            .filter(|(_, document)| document.deadline.is_some_and(|deadline| deadline <= now))
            .map(|(uri, _)| uri.clone())
            .collect();
        for uri in due {
            self.prepare_diagnostics(&uri)?;
        }
        Ok(())
    }

    fn prepare_diagnostics(&mut self, uri: &Uri) -> Result<()> {
        let document = self
            .documents
            .get_mut(uri)
            .expect("only open documents are analyzed");
        document.deadline = None;
        // Analysis contains at most one error. Retain the tree/model in the
        // document and release its mutable borrow before converting ranges.
        let errors = match document.analyze(uri.as_str()) {
            Ok(analysis) => analysis.diagnostics.clone(),
            Err(error) => {
                match error {
                    ParseError::InvalidUtf8(_) => eprintln!(
                        "iris lsp: Unicode buffer invariant failed for {}: {error}",
                        uri.as_str()
                    ),
                    ParseError::SourceTooLarge { .. } => {
                        eprintln!("iris lsp: cannot analyze {}: {error}", uri.as_str())
                    }
                }
                return Ok(());
            }
        };
        let positions = Positions::new(&document.source, &document.lines, self.encoding);
        document.samples.checker = errors
            .iter()
            .map(|error| diagnostic::to_lsp(error, &positions))
            .collect();
        document.samples.directives = sample::parse(
            document.cached_syntax().expect("analyzed syntax"),
            uri,
            &positions,
        );
        document.samples.outcomes.clear();
        if errors.is_empty() && document.samples.directives.iter().any(|d| d.path.is_some()) {
            document.samples.cancellation = Some(iris_core::CancellationToken::default());
            if self.queued.insert(uri.clone()) {
                self.pending.push_back(uri.clone());
            }
            return Ok(());
        }
        let diagnostics = document.samples.diagnostics(&positions);
        let version = document.version;
        self.send_diagnostics(PublishDiagnosticsParams::new(
            uri.clone(),
            diagnostics,
            Some(version),
        ))
    }

    fn dispatch_sample(&mut self) -> Result<()> {
        if self.worker.active.is_some() || self.shutdown_requested {
            return Ok(());
        }
        while let Some(uri) = self.pending.pop_front() {
            self.queued.remove(&uri);
            let Some(document) = self.documents.get(&uri) else {
                continue;
            };
            // An accepted edit invalidated this pending batch; its next deadline
            // will queue the newest generation, after other ready documents.
            let Some(cancellation) = document.samples.cancellation.clone() else {
                continue;
            };
            if document.deadline.is_some() {
                continue;
            }
            return self.worker.dispatch(sample::Job {
                identity: sample::Identity {
                    uri,
                    version: document.version,
                    generation: document.generation,
                    cancellation,
                },
                source: document.source.clone(),
                directives: document.samples.directives.clone(),
            });
        }
        Ok(())
    }

    fn receive_sample(&mut self, batch: sample::Batch) -> Result<()> {
        // Retire even cancelled/obsolete jobs, otherwise the worker stays busy.
        self.worker.active.take();
        let identity = batch.identity;
        let Some(document) = self.documents.get_mut(&identity.uri) else {
            return Ok(());
        };
        if document.generation != identity.generation
            || document.version != identity.version
            || identity.cancellation.is_cancelled()
        {
            return Ok(());
        }
        let Some(outputs) = batch.outputs else {
            return Ok(());
        };
        let positions = Positions::new(&document.source, &document.lines, self.encoding);
        sample::install(document, outputs, &positions);
        let diagnostics = document.samples.diagnostics(&positions);
        let has_hints = document
            .samples
            .outcomes
            .iter()
            .any(|outcome| matches!(outcome, sample::Outcome::Success { .. }));
        self.send_diagnostics(PublishDiagnosticsParams::new(
            identity.uri,
            diagnostics,
            Some(identity.version),
        ))?;
        if has_hints {
            self.refresh_hints()?;
        }
        Ok(())
    }

    fn refresh_hints(&mut self) -> Result<()> {
        if self.refresh_supported {
            self.next_request += 1;
            self.connection.sender.send(
                Request::new(
                    format!("iris-inlay-{}", self.next_request).into(),
                    "workspace/inlayHint/refresh".into(),
                    (),
                )
                .into(),
            )?;
        }
        Ok(())
    }

    fn send_diagnostics(&self, params: PublishDiagnosticsParams) -> Result<()> {
        use lsp_types::notification::Notification as _;
        self.connection
            .sender
            .send(Notification::new(PublishDiagnostics::METHOD.into(), params).into())?;
        Ok(())
    }
}

fn decode_notification<N: lsp_types::notification::Notification>(
    params: serde_json::Value,
) -> Option<N::Params> {
    match serde_json::from_value(params) {
        Ok(params) => Some(params),
        Err(error) => {
            eprintln!("iris lsp: invalid {} parameters: {error}", N::METHOD);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as support;
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use support::{Client, TIMEOUT};

    struct Gate(Option<crossbeam_channel::Sender<()>>);
    impl Drop for Gate {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }
    fn fence(client: &mut Client, uri: &Uri) {
        assert!(
            client
                .request(
                    "textDocument/hover",
                    json!({"textDocument":{"uri":uri},"position":{"line":1,"character":0}})
                )
                .response_result
                .is_ok()
        );
    }
    fn gated_client(
        stale_success: bool,
    ) -> (
        Client,
        crossbeam_channel::Receiver<iris_core::CancellationToken>,
        Gate,
    ) {
        let (started, wait_started) = crossbeam_channel::bounded(1);
        let (release, wait_release) = crossbeam_channel::bounded(1);
        let calls = Arc::new(AtomicUsize::new(0));
        let client = Client::with_server(json!({}), move |connection| {
            run_with_executor(connection, move |job| {
                if calls.fetch_add(1, Ordering::Relaxed) == 0 {
                    started.send(job.identity.cancellation.clone()).unwrap();
                    wait_release
                        .recv_timeout(TIMEOUT)
                        .expect("test gate was not released");
                    if stale_success {
                        // Deliberately violate cooperative cancellation to check identity separately.
                        return sample::Batch {
                            identity: job.identity,
                            outputs: Some(vec![sample::ReadOutput::Success {
                                preview: "OBSOLETE".into(),
                                hint: "OBSOLETE".into(),
                            }]),
                        };
                    }
                    assert!(job.identity.cancellation.is_cancelled());
                    return sample::Batch {
                        identity: job.identity,
                        outputs: None,
                    };
                }
                sample::execute(job)
            })
            .map_err(|e| e.to_string())
        });
        (client, wait_started, Gate(Some(release)))
    }
    #[test]
    fn accepted_edits_cancel_before_debounce_without_blocking_requests() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("input"), "7").unwrap();
        let uri = support::file_uri(&dir.path().join("r.iris"));
        let (mut client, started, gate) = gated_client(false);
        client.open(&uri, 1, "# iris: sample input\nx:int");
        let token = started.recv_timeout(TIMEOUT).unwrap();
        fence(&mut client, &uri);
        client.change(&uri, 1, vec![]);
        fence(&mut client, &uri);
        assert!(!token.is_cancelled());
        client.change(
            &uri,
            2,
            vec![lsp_types::TextDocumentContentChangeEvent {
                range: Some(lsp_types::Range::new(
                    lsp_types::Position::new(90, 0),
                    lsp_types::Position::new(90, 1),
                )),
                range_length: None,
                text: "invalid".into(),
            }],
        );
        fence(&mut client, &uri);
        assert!(!token.is_cancelled());
        client.change(
            &uri,
            2,
            vec![lsp_types::TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "# iris: sample input\nx:int let answer=x+1".into(),
            }],
        );
        fence(&mut client, &uri);
        assert!(token.is_cancelled());
        let hints=client.request("textDocument/inlayHint",json!({"textDocument":{"uri":uri},"range":{"start":{"line":0,"character":0},"end":{"line":1,"character":100}}})).response_result.unwrap();
        assert!(!hints.to_string().contains("OBSOLETE"));
        drop(gate);
        assert!(client.diagnostics(&uri, Some(2)).diagnostics.is_empty());
        fence(&mut client, &uri);
        assert_eq!(client.pending_diagnostic_count(&uri, Some(1)), 0);
        let hover = client
            .request(
                "textDocument/hover",
                json!({"textDocument":{"uri":uri},"position":{"line":0,"character":4}}),
            )
            .response_result
            .unwrap();
        assert!(hover.to_string().contains("answer: int 8"));
        client.shutdown();
    }
    #[test]
    fn stale_success_cannot_survive_close_reopen_with_equal_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("input"), "7").unwrap();
        let uri = support::file_uri(&dir.path().join("r.iris"));
        let (mut client, started, gate) = gated_client(true);
        client.open(&uri, 1, "# iris: sample input\nx:int");
        let token = started.recv_timeout(TIMEOUT).unwrap();
        client.close(&uri);
        client.diagnostics(&uri, None);
        client.open(&uri, 1, "# iris: sample input\ny:int");
        fence(&mut client, &uri);
        assert!(token.is_cancelled());
        drop(gate);
        assert!(client.diagnostics(&uri, Some(1)).diagnostics.is_empty());
        // Without the identity guard, either hover sees the obsolete preview or
        // a second publication is queued before its response. Check both, without
        // depending on which request/result the main loop selects first.
        let hover = client
            .request(
                "textDocument/hover",
                json!({"textDocument":{"uri":uri},"position":{"line":0,"character":4}}),
            )
            .response_result
            .unwrap();
        assert!(hover.to_string().contains("y: int 7"), "{hover}");
        assert_eq!(client.pending_diagnostic_count(&uri, Some(1)), 0);
        client.shutdown();
    }
    #[test]
    fn pending_replacement_has_no_previous_success_summary() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("input"), "7").unwrap();
        let uri = support::file_uri(&dir.path().join("r.iris"));
        let (started, wait_started) = crossbeam_channel::bounded(1);
        let (release, wait_release) = crossbeam_channel::bounded(1);
        let calls = AtomicUsize::new(0);
        let mut client = Client::with_server(json!({}), move |connection| {
            run_with_executor(connection, move |job| {
                if calls.fetch_add(1, Ordering::Relaxed) == 1 {
                    started.send(()).unwrap();
                    wait_release.recv_timeout(TIMEOUT).unwrap();
                }
                sample::execute(job)
            })
            .map_err(|e| e.to_string())
        });
        let gate = Gate(Some(release));
        client.open(&uri, 1, "# iris: sample input\nx:int");
        assert!(client.diagnostics(&uri, Some(1)).diagnostics.is_empty());
        client.change(
            &uri,
            2,
            vec![lsp_types::TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "# iris: sample input\ny:int".into(),
            }],
        );
        wait_started.recv_timeout(TIMEOUT).unwrap();
        let hover = client
            .request(
                "textDocument/hover",
                json!({"textDocument":{"uri":uri},"position":{"line":0,"character":4}}),
            )
            .response_result
            .unwrap();
        assert!(hover.to_string().contains("Reading sample"));
        assert!(!hover.to_string().contains("x: int 7"));
        let hints=client.request("textDocument/inlayHint",json!({"textDocument":{"uri":uri},"range":{"start":{"line":0,"character":0},"end":{"line":1,"character":100}}})).response_result.unwrap();
        let hints: Vec<lsp_types::InlayHint> = serde_json::from_value(hints).unwrap();
        assert!(hints.iter().all(|hint| hint.kind.is_some()));
        drop(gate);
        assert!(client.diagnostics(&uri, Some(2)).diagnostics.is_empty());
        client.shutdown();
    }

    #[test]
    fn shutdown_responds_and_cancels_a_gated_worker() {
        let dir = tempfile::tempdir().unwrap();
        let uri = support::file_uri(&dir.path().join("r.iris"));
        let (mut client, started, gate) = gated_client(false);
        client.open(&uri, 1, "# iris: sample input\nx:int");
        let token = started.recv_timeout(TIMEOUT).unwrap();
        assert!(
            client
                .request("shutdown", serde_json::Value::Null)
                .response_result
                .is_ok()
        );
        assert!(token.is_cancelled());
        drop(gate);
        client.send(Notification::new("exit".into(), ()).into());
        // Drop disconnects and joins; the worker cannot hang on its result send.
    }
    #[test]
    fn refresh_is_negotiated_and_invalidation_refreshes_visible_hints() {
        for refresh in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("input"), "7").unwrap();
            let uri = support::file_uri(&dir.path().join("r.iris"));
            let mut client =
                Client::new(json!({"workspace":{"inlayHint":{"refreshSupport":refresh}}}));
            client.open(&uri, 1, "# iris: sample input\nx:int");
            client.diagnostics(&uri, Some(1));
            if refresh {
                let Message::Request(request) =
                    client.receive_until(|m| matches!(m, Message::Request(_)))
                else {
                    unreachable!()
                };
                assert_eq!(request.method, "workspace/inlayHint/refresh");
                client.send(Response::new_ok(request.id, ()).into());
            }
            client.change(&uri, 2, vec![]);
            fence(&mut client, &uri);
            if refresh {
                let Message::Request(request) =
                    client.receive_until(|m| matches!(m, Message::Request(_)))
                else {
                    unreachable!()
                };
                assert_eq!(request.method, "workspace/inlayHint/refresh");
                client.send(Response::new_ok(request.id, ()).into());
            }
            client.diagnostics(&uri, Some(2));
            fence(&mut client, &uri);
            // Client queues every unmatched request; absence is checked after fences.
            if !refresh {
                assert_eq!(client.pending_server_request_count(), 0);
            }
            client.shutdown();
        }
    }
}
