mod support;

use lsp_server::Notification;
use lsp_types::{
    Diagnostic, DiagnosticSeverity, NumberOrString, Position, PositionEncodingKind,
    PublishDiagnosticsParams, Range, TextDocumentContentChangeEvent, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri,
};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use support::Client;

fn client() -> Client {
    Client::new(
        json!({"textDocument": {"publishDiagnostics": {"versionSupport": true, "codeDescriptionSupport": true}}}),
    )
}

fn edit(start: u32, end: u32, text: &str) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: Some(Range::new(Position::new(0, start), Position::new(0, end))),
        range_length: None,
        text: text.into(),
    }
}

fn replace(text: &str) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: None,
        range_length: None,
        text: text.into(),
    }
}

fn only_error(params: &PublishDiagnosticsParams, code: &str) -> Diagnostic {
    assert_eq!(params.diagnostics.len(), 1, "{params:?}");
    let diagnostic = &params.diagnostics[0];
    assert_eq!(diagnostic.code, Some(NumberOrString::String(code.into())));
    assert_eq!(diagnostic.source.as_deref(), Some("iris"));
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::ERROR));
    assert!(!diagnostic.message.is_empty());
    assert_eq!(
        diagnostic.code_description.as_ref().unwrap().href.as_str(),
        "https://github.com/Gecode/iris/blob/main/spec/0.1/errors.md#114-parse-time-codes"
    );
    // Check the wire spelling as well as the typed value.
    assert_eq!(
        serde_json::to_value(diagnostic).unwrap()["codeDescription"]["href"],
        "https://github.com/Gecode/iris/blob/main/spec/0.1/errors.md#114-parse-time-codes"
    );
    diagnostic.clone()
}

#[test]
fn diagnostics_follow_open_change_fix_and_close() {
    let mut client = client();
    assert_eq!(
        client.initialization.capabilities.position_encoding,
        Some(PositionEncodingKind::UTF16)
    );
    let Some(TextDocumentSyncCapability::Options(sync)) =
        &client.initialization.capabilities.text_document_sync
    else {
        panic!("missing synchronization capabilities")
    };
    assert_eq!(sync.open_close, Some(true));
    assert_eq!(sync.change, Some(TextDocumentSyncKind::INCREMENTAL));
    let uri = "untitled:lifecycle".parse().unwrap();
    client.open(&uri, 1, "x:");
    let error = only_error(&client.diagnostics(&uri, Some(1)), "E-SPEC-SYNTAX");
    assert_eq!(
        error.range,
        Range::new(Position::new(0, 2), Position::new(0, 2))
    );
    client.change(&uri, 2, vec![edit(2, 2, "unknown")]);
    let error = only_error(&client.diagnostics(&uri, Some(2)), "E-SPEC-UNKNOWN-TYPE");
    assert_eq!(
        error.range,
        Range::new(Position::new(0, 9), Position::new(0, 9))
    );
    client.change(&uri, 4, vec![edit(2, 9, "int")]);
    assert!(client.diagnostics(&uri, Some(4)).diagnostics.is_empty());
    client.close(&uri);
    let clear = client.diagnostics(&uri, None);
    assert!(clear.diagnostics.is_empty());
    assert!(
        serde_json::to_value(clear)
            .unwrap()
            .get("version")
            .is_none()
    );
    client.shutdown();
}

#[test]
fn unicode_ranges_and_ordered_edits_use_the_negotiated_encoding() {
    for (capabilities, encoding, emoji_start, emoji_end, at) in [
        (json!({}), PositionEncodingKind::UTF16, 8, 10, 12),
        (
            json!({"general": {"positionEncodings": ["utf-16", "utf-8"]}}),
            PositionEncodingKind::UTF8,
            9,
            13,
            15,
        ),
    ] {
        let mut client = Client::new(capabilities);
        assert_eq!(
            client.initialization.capabilities.position_encoding,
            Some(encoding)
        );
        let uri = "untitled:unicode".parse().unwrap();
        client.open(&uri, 1, "let s=\"é😀\" @");
        let error = only_error(&client.diagnostics(&uri, Some(1)), "E-SPEC-CHARACTER");
        assert_eq!(
            error.range,
            Range::new(Position::new(0, at), Position::new(0, at + 1))
        );
        // Replacing the emoji shifts the second edit's range in this same batch.
        client.change(
            &uri,
            2,
            vec![
                edit(emoji_start, emoji_end, "q"),
                edit(
                    at - (emoji_end - emoji_start) + 1,
                    at - (emoji_end - emoji_start) + 2,
                    "",
                ),
            ],
        );
        assert!(client.diagnostics(&uri, Some(2)).diagnostics.is_empty());
        client.change(&uri, 3, vec![replace("x:unknown")]);
        only_error(&client.diagnostics(&uri, Some(3)), "E-SPEC-UNKNOWN-TYPE");
        client.shutdown();
    }
}

#[test]
fn invalid_batches_and_notification_parameters_preserve_the_buffer() {
    let mut client = client();
    let uri = "untitled:atomic".parse().unwrap();
    client.open(&uri, 1, "let s=\"é😀\" @");
    client.diagnostics(&uri, Some(1));
    // The first edit is valid, but the second splits a UTF-16 surrogate pair.
    client.change(&uri, 2, vec![edit(12, 13, ""), edit(9, 10, "q")]);
    client.send(
        Notification::new(
            "textDocument/didChange".into(),
            json!({"textDocument": {"uri": uri, "version": "bad"}}),
        )
        .into(),
    );
    // Version 2 is still available, and @ is still in the unchanged buffer.
    client.change(&uri, 2, vec![edit(12, 13, "")]);
    assert!(client.diagnostics(&uri, Some(2)).diagnostics.is_empty());
    client.change(&uri, 3, vec![edit(0, 999, "x:int")]);
    assert!(client.diagnostics(&uri, Some(3)).diagnostics.is_empty());
    client.shutdown();
}

#[test]
fn document_versions_deadlines_and_reopens_are_independent() {
    let mut client = client();
    let a = "untitled:a".parse().unwrap();
    let b = "untitled:b".parse().unwrap();
    client.open(&a, 10, "x:int");
    client.open(&b, 1, "x:int");
    assert!(client.diagnostics(&a, Some(10)).diagnostics.is_empty());
    assert!(client.diagnostics(&b, Some(1)).diagnostics.is_empty());
    client.change(&a, 11, vec![replace("x:unknown")]);
    client.change(&a, 10, vec![replace("x:int")]);
    client.change(&a, 11, vec![replace("x:int")]);
    // Keep B changing past A's deadline: other documents must not reset it.
    let until = Instant::now() + Duration::from_millis(300);
    let mut b_version = 1;
    while Instant::now() < until {
        b_version += 1;
        client.change(&b, b_version, vec![replace("x:int")]);
        assert_eq!(
            client
                .request("unsupported", Value::Null)
                .response_result
                .unwrap_err()
                .code,
            -32601
        );
    }
    // One more round trip receives anything the server published before it,
    // so a slow client thread cannot miss A's diagnostics.
    client.request("unsupported", Value::Null);
    assert!(client.pending_diagnostic_count(&a, Some(11)) > 0);
    only_error(&client.diagnostics(&a, Some(11)), "E-SPEC-UNKNOWN-TYPE");
    client.change(&b, b_version + 1, vec![replace("x:unknown")]);
    client.change(&b, b_version + 2, vec![replace("x:int")]);
    assert!(
        client
            .diagnostics(&b, Some(b_version + 2))
            .diagnostics
            .is_empty()
    );
    client.change(&a, 12, vec![replace("x:int")]);
    client.close(&a);
    assert!(client.diagnostics(&a, None).diagnostics.is_empty());
    let publications_before_close = client.pending_diagnostic_count(&a, Some(12));
    client.open(&a, 1, "x:unknown");
    only_error(&client.diagnostics(&a, Some(1)), "E-SPEC-UNKNOWN-TYPE");
    // A subsequent debounce interval exercises removal of the old deadline.
    client.change(&b, b_version + 3, vec![replace("x:int")]);
    assert!(
        client
            .diagnostics(&b, Some(b_version + 3))
            .diagnostics
            .is_empty()
    );
    assert_eq!(
        client.pending_diagnostic_count(&a, Some(12)),
        publications_before_close
    );
    client.shutdown();
}

#[test]
fn analysis_keeps_error_order_and_physical_capture_ranges() {
    let mut client = client();
    let uri = "untitled:ordering".parse().unwrap();
    client.open(&uri, 1, "a[1]:rest bogus");
    only_error(&client.diagnostics(&uri, Some(1)), "E-SPEC-TYPE");
    client.open(&uri, 2, "line \"{x:\\nunknown}\"");
    let error = only_error(&client.diagnostics(&uri, Some(2)), "E-SPEC-UNKNOWN-TYPE");
    assert_eq!(
        error.range,
        Range::new(Position::new(0, 18), Position::new(0, 18))
    );
    client.shutdown();
}

#[test]
fn representative_repository_specs_use_the_same_protocol() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut client = client();
    let uri: Uri = "untitled:fixture".parse().unwrap();
    for (path, code) in [
        ("formats/dimacs-cnf/dimacs-cnf.iris", None),
        (
            "conformance/cases/0.1/smoke/parse-line-error/spec.iris",
            Some("E-SPEC-SYNTAX"),
        ),
        (
            "conformance/cases/0.1/pinned/type-self-reference/spec.iris",
            Some("E-SPEC-UNKNOWN-TYPE"),
        ),
    ] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        client.open(&uri, 1, &source);
        let diagnostics = client.diagnostics(&uri, Some(1));
        if let Some(code) = code {
            only_error(&diagnostics, code);
        } else {
            assert!(
                diagnostics.diagnostics.is_empty(),
                "{path}: {diagnostics:?}"
            );
        }
    }
    client.shutdown();
}

#[test]
fn lifecycle_errors_are_protocol_errors() {
    let mut client = client();
    assert_eq!(
        client
            .request("shutdown", json!({"unexpected": true}))
            .response_result
            .unwrap_err()
            .code,
        -32602
    );
    // A malformed shutdown must not prevent a subsequent orderly shutdown.
    client.shutdown();
    assert!(
        self::client()
            .exit_without_shutdown()
            .contains("without shutdown")
    );
}
