#[allow(dead_code)]
mod support;
use lsp_types::{
    DiagnosticSeverity, Hover, InlayHint, NumberOrString, Position, Range,
    TextDocumentContentChangeEvent,
};
use serde_json::json;
use support::{Client, file_uri};

fn hover(client: &mut Client, uri: &lsp_types::Uri, line: u32, character: u32) -> Option<Hover> {
    serde_json::from_value(
        client
            .request(
                "textDocument/hover",
                json!({"textDocument":{"uri":uri},"position":{"line":line,"character":character}}),
            )
            .response_result
            .unwrap(),
    )
    .unwrap()
}
fn hover_text(client: &mut Client, uri: &lsp_types::Uri, line: u32) -> String {
    let value = hover(client, uri, line, 4).unwrap();
    let lsp_types::HoverContents::Markup(contents) = value.contents else {
        panic!("markup")
    };
    contents.value
}
fn hints(client: &mut Client, uri: &lsp_types::Uri, range: Range) -> Vec<InlayHint> {
    serde_json::from_value(
        client
            .request(
                "textDocument/inlayHint",
                json!({"textDocument":{"uri":uri},"range":range}),
            )
            .response_result
            .unwrap(),
    )
    .unwrap()
}
fn all_hints(client: &mut Client, uri: &lsp_types::Uri, last_line: u32) -> Vec<InlayHint> {
    hints(
        client,
        uri,
        Range::new(Position::new(0, 0), Position::new(last_line, u32::MAX)),
    )
}
fn replace(client: &Client, uri: &lsp_types::Uri, version: i32, text: &str) {
    client.change(
        uri,
        version,
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.into(),
        }],
    );
}
#[test]
fn clean_sample_summary_hint_range_and_disk_reread() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("with spaces.txt"), "7").unwrap();
    let uri = file_uri(&dir.path().join("reader with spaces.iris"));
    let mut client = Client::new(json!({}));
    let source = "# iris: sample \"with spaces.txt\"\r\nx:int\r\n";
    client.open(&uri, 1, source);
    assert!(client.diagnostics(&uri, Some(1)).diagnostics.is_empty());
    let text = hover_text(&mut client, &uri, 0);
    assert!(text.contains("sample with spaces.txt"));
    assert!(text.contains("x: int 7"));
    let all = all_hints(&mut client, &uri, 1);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].position, Position::new(0, 32));
    assert!(all[0].kind.is_none());
    assert!(all[0].tooltip.is_some());
    assert_eq!(
        hints(
            &mut client,
            &uri,
            Range::new(Position::new(1, 0), Position::new(1, 5))
        )
        .len(),
        1
    );
    std::fs::write(dir.path().join("with spaces.txt"), "9").unwrap();
    replace(&client, &uri, 2, source);
    assert!(client.diagnostics(&uri, Some(2)).diagnostics.is_empty());
    assert!(hover_text(&mut client, &uri, 0).contains("x: int 9"));
    assert_eq!(client.pending_diagnostic_count(&uri, Some(2)), 0);
    client.shutdown();
}
#[test]
fn failures_use_exact_spec_ranges_in_both_encodings_and_physical_lines() {
    for utf8 in [false, true] {
        for newline in ["\n", "\r\n"] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("bad"), "oops").unwrap();
            let uri = file_uri(&dir.path().join("reader.iris"));
            let mut client = Client::new(if utf8 {
                json!({"general":{"positionEncodings":["utf-8"]}})
            } else {
                json!({})
            });
            let prefix = "let label=\"é😀\" ";
            client.open(
                &uri,
                1,
                &format!("# iris: sample bad{newline}{prefix}x:int{newline}"),
            );
            let diagnostics = client.diagnostics(&uri, Some(1)).diagnostics;
            assert_eq!(diagnostics.len(), 1);
            let diagnostic = &diagnostics[0];
            let start = if utf8 {
                prefix.len()
            } else {
                prefix.encode_utf16().count()
            } as u32;
            assert_eq!(
                diagnostic.range,
                Range::new(Position::new(1, start), Position::new(1, start + 5))
            );
            assert_eq!(
                diagnostic.code,
                Some(NumberOrString::String("E-EXPECTED-VALUE".into()))
            );
            assert!(diagnostic.message.contains("sample bad"));
            assert!(diagnostic.message.contains("line 1, column 1"));
            assert!(diagnostic.message.contains("oops"));
            assert_eq!(
                diagnostic.code_description.as_ref().unwrap().href.as_str(),
                "https://iris.gecode.dev/specification/0.1/errors/#e-expected-value"
            );
            client.shutdown();
        }
    }
}
#[test]
fn instances_params_repeated_keys_and_quoted_values_are_independent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("named"), "first 2\nsecond 4").unwrap();
    let uri = file_uri(&dir.path().join("reader.iris"));
    let mut client = Client::new(json!({"textDocument":{"hover":{"contentFormat":["markdown"]}}}));
    client.open(&uri,1,"# iris: sample named instance=2 param scale=1 param scale=3\n# iris: sample named instance=first param instance=\"second\" param scale=2\nparam scale=1 instances { name:word x:int let answer=x*scale }\n");
    assert!(client.diagnostics(&uri, Some(1)).diagnostics.is_empty());
    assert!(hover_text(&mut client, &uri, 0).contains("answer: int 12"));
    assert!(hover_text(&mut client, &uri, 1).contains("answer: int 8"));
    assert_eq!(
        all_hints(&mut client, &uri, 2)
            .iter()
            .filter(|h| h.kind.is_none())
            .count(),
        2
    );
    client.shutdown();
}
#[test]
fn multiple_samples_publish_one_merged_batch_in_directive_order() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("good"), "7").unwrap();
    std::fs::write(dir.path().join("bad"), "oops").unwrap();
    let uri = file_uri(&dir.path().join("reader.iris"));
    let mut client = Client::new(json!({}));
    let source = "# iris: sample good unknown\n# iris: sample bad\nx:int\n";
    client.open(&uri, 1, source);
    let diagnostics = client.diagnostics(&uri, Some(1)).diagnostics;
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::WARNING));
    assert!(diagnostics[1].message.contains("sample bad"));
    assert!(hover_text(&mut client, &uri, 0).contains("x: int 7"));
    assert!(hover_text(&mut client, &uri, 1).contains("oops"));
    assert_eq!(
        all_hints(&mut client, &uri, 2)
            .iter()
            .filter(|h| h.kind.is_none())
            .count(),
        1
    );
    assert_eq!(client.pending_diagnostic_count(&uri, Some(1)), 0);
    replace(
        &client,
        &uri,
        2,
        "# iris: sample bad\n# iris: sample bad\nx:int",
    );
    let diagnostics = client.diagnostics(&uri, Some(2)).diagnostics;
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].range, diagnostics[1].range);
    assert!(diagnostics[0].message.contains("directive line 1"));
    assert!(diagnostics[1].message.contains("directive line 2"));
    client.shutdown();
}
#[test]
fn acquisition_and_locationless_errors_fall_back_to_directive() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("extra"), "7 extra").unwrap();
    std::fs::write(dir.path().join("utf8"), b"\xff").unwrap();
    std::fs::write(dir.path().join("ok"), "7").unwrap();
    std::fs::create_dir(dir.path().join("directory")).unwrap();
    let uri = file_uri(&dir.path().join("reader.iris"));
    let mut client = Client::new(json!({}));
    for (i, directive, code) in [
        (1, "missing", "sample-file"),
        (2, "directory", "sample-file"),
        (3, "extra", "E-TRAILING-INPUT"),
        (4, "ok param unknown=1", "E-PARAM-UNKNOWN"),
        (5, "utf8", "E-UTF8"),
    ] {
        let line = format!("# iris: sample {directive}");
        client.open(&uri, i, &format!("{line}\nx:int"));
        let diagnostics = client.diagnostics(&uri, Some(i)).diagnostics;
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(code.into()))
        );
        assert_eq!(
            diagnostics[0].range,
            Range::new(Position::new(0, 0), Position::new(0, line.len() as u32))
        );
        assert_eq!(
            all_hints(&mut client, &uri, 1)
                .iter()
                .filter(|h| h.kind.is_none())
                .count(),
            0
        );
    }
    client.shutdown();
}
#[test]
fn directives_are_comments_in_any_block_and_strings_are_silent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("ok"), "7").unwrap();
    let uri = file_uri(&dir.path().join("reader.iris"));
    let mut client = Client::new(json!({}));
    client.open(&uri,1,"let label=\"# iris: sample missing\"\nif 0 { # iris: sample ok\n}\nx:int # iris: sample ok\n");
    assert!(client.diagnostics(&uri, Some(1)).diagnostics.is_empty());
    assert_eq!(
        all_hints(&mut client, &uri, 3)
            .iter()
            .filter(|h| h.kind.is_none())
            .count(),
        2
    );
    for (version, line) in [
        (2, "# iris: sample"),
        (3, "# iris: sample ok instance="),
        (4, "# iris: sample ok param"),
        (5, "# iris: sample \"ok"),
        (6, "# iris: sample /absolute"),
    ] {
        replace(&client, &uri, version, &format!("{line}\nx:int"));
        let diagnostics = client.diagnostics(&uri, Some(version)).diagnostics;
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
    }
    let untitled = "untitled:sample".parse().unwrap();
    client.open(&untitled, 1, "# iris: sample ok\nx:int");
    assert!(
        client.diagnostics(&untitled, Some(1)).diagnostics[0]
            .message
            .contains("file URI")
    );
    replace(&client, &uri, 7, "# iris: sampled ok\nx:unknown");
    let diagnostics = client.diagnostics(&uri, Some(7)).diagnostics;
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[1].severity, Some(DiagnosticSeverity::WARNING));
    client.shutdown();
}
#[test]
fn budget_error_is_mapped_to_the_active_statement() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("empty"), "").unwrap();
    let uri = file_uri(&dir.path().join("reader.iris"));
    let mut client = Client::new(json!({}));
    client.open(&uri, 1, "# iris: sample empty\nrepeat 1048577 {}\n");
    let diagnostics = client.diagnostics(&uri, Some(1)).diagnostics;
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String("E-BUDGET".into()))
    );
    assert_eq!(diagnostics[0].range.start.line, 1);
    client.shutdown();
}

#[test]
fn quoted_named_selection_and_string_parameter_preserve_spaces() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("named"), "2 named item\n4 other item\n").unwrap();
    let uri = file_uri(&dir.path().join("reader.iris"));
    let mut client = Client::new(json!({}));
    client.open(&uri,1,"# iris: sample named instance=\"named item\" param label=\"hello world\"\nparam label=\"default\" instances {x:int name:rest}\n");
    assert!(client.diagnostics(&uri, Some(1)).diagnostics.is_empty());
    let text = hover_text(&mut client, &uri, 0);
    assert!(text.contains("x: int 2"));
    assert!(text.contains("label: string \"hello world\""));
    client.shutdown();
}
