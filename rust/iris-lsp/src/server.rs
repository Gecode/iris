use crate::{
    Result, assist, diagnostic,
    document::{Document, PositionEncoding},
    navigation, semantic_tokens, structure,
};
use crossbeam_channel::RecvTimeoutError;
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
use std::{collections::HashMap, time::Instant};

pub(crate) fn run(connection: Connection) -> Result<()> {
    let (encoding, structure_options, assist_options) = initialize(&connection)?;
    Server {
        connection,
        encoding,
        structure_options,
        assist_options,
        documents: HashMap::new(),
        shutdown_requested: false,
    }
    .run()
}

fn initialize(
    connection: &Connection,
) -> Result<(PositionEncoding, structure::Options, assist::Options)> {
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
    let result = InitializeResult {
        capabilities: ServerCapabilities {
            position_encoding: Some(encoding.protocol_kind()),
            hover_provider: Some(HoverProviderCapability::Simple(true)),
            completion_provider: Some(CompletionOptions::default()),
            inlay_hint_provider: Some(OneOf::Left(true)),
            definition_provider: Some(OneOf::Left(true)),
            references_provider: Some(OneOf::Left(true)),
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
    Ok((encoding, structure_options, assist_options))
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
            let message = if let Some(deadline) = deadline {
                match self.connection.receiver.recv_deadline(deadline) {
                    Ok(message) => message,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err("LSP connection closed before exit".into());
                    }
                }
            } else {
                self.connection
                    .receiver
                    .recv()
                    .map_err(|_| "LSP connection closed before exit")?
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
                    self.documents.clear();
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
                    self.documents
                        .insert(item.uri.clone(), Document::new(item.text, item.version));
                    self.publish_diagnostics(&item.uri)?;
                }
            }
            "textDocument/didChange" => {
                if let Some(params) =
                    decode_notification::<DidChangeTextDocument>(notification.params)
                    && let Some(document) = self.documents.get_mut(&params.text_document.uri)
                    && let Err(error) = document.change(
                        params.text_document.version,
                        params.content_changes,
                        self.encoding,
                        Instant::now(),
                    )
                {
                    eprintln!(
                        "iris lsp: invalid edit for {}: {error}",
                        params.text_document.uri.as_str()
                    );
                }
            }
            "textDocument/didClose" => {
                if let Some(params) =
                    decode_notification::<DidCloseTextDocument>(notification.params)
                {
                    self.documents.remove(&params.text_document.uri);
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
            self.publish_diagnostics(&uri)?;
        }
        Ok(())
    }

    fn publish_diagnostics(&mut self, uri: &Uri) -> Result<()> {
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
        let diagnostics = errors
            .iter()
            .map(|error| {
                diagnostic::to_lsp(error, &document.source, &document.lines, self.encoding)
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let params =
            PublishDiagnosticsParams::new(uri.clone(), diagnostics, Some(document.version));
        self.send_diagnostics(params)
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
