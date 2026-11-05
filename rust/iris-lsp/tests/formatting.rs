// Each integration suite uses a different subset of the shared protocol client.
#[allow(dead_code)]
mod support;

use gecode_iris_syntax::{FormatOptions, LanguageVersion, TextRange, TextSize};
use lsp_types::{Position, Range, TextDocumentContentChangeEvent, TextEdit, Uri};
use serde_json::{Value, json};
use std::fs;
use support::{Client, file_uri};

fn params(uri: &Uri) -> Value {
    // Iris has one style; these deliberately disagree with its indentation.
    json!({"textDocument": {"uri": uri}, "options": {"tabSize": 8, "insertSpaces": false}})
}

fn document_edits(client: &mut Client, uri: &Uri) -> Vec<TextEdit> {
    serde_json::from_value(
        client
            .request("textDocument/formatting", params(uri))
            .response_result
            .unwrap(),
    )
    .unwrap()
}

fn range_edits(client: &mut Client, uri: &Uri, range: Range) -> Vec<TextEdit> {
    let mut params = params(uri);
    params["range"] = serde_json::to_value(range).unwrap();
    serde_json::from_value(
        client
            .request("textDocument/rangeFormatting", params)
            .response_result
            .unwrap(),
    )
    .unwrap()
}

fn replace(client: &Client, uri: &Uri, version: i32, source: &str) {
    client.change(
        uri,
        version,
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: source.into(),
        }],
    );
}

/// Independent conversion of protocol positions when applying server edits.
fn byte_offset(source: &str, position: Position, utf8: bool) -> usize {
    let mut line = 0;
    let mut character = 0;
    let mut scalars = source.char_indices().peekable();
    while let Some((offset, scalar)) = scalars.next() {
        if Position::new(line, character) == position {
            return offset;
        }
        if matches!(scalar, '\r' | '\n') {
            if scalar == '\r' && scalars.peek().is_some_and(|(_, next)| *next == '\n') {
                scalars.next();
            }
            line += 1;
            character = 0;
        } else {
            character += if utf8 {
                scalar.len_utf8()
            } else {
                scalar.len_utf16()
            } as u32;
        }
    }
    assert_eq!(position, Position::new(line, character));
    source.len()
}

fn apply(source: &str, edits: &[TextEdit], utf8: bool) -> String {
    let mut result = source.to_owned();
    let mut previous_start = source.len();
    for edit in edits.iter().rev() {
        let start = byte_offset(source, edit.range.start, utf8);
        let end = byte_offset(source, edit.range.end, utf8);
        assert!(start <= end && end <= previous_start, "overlapping edits");
        result.replace_range(start..end, &edit.new_text);
        previous_start = start;
    }
    result
}

#[test]
fn formats_latest_buffer_and_returns_no_edits_when_unchanged() {
    let mut client = Client::new(json!({}));
    let caps = serde_json::to_value(&client.initialization.capabilities).unwrap();
    assert_eq!(caps["documentFormattingProvider"], true);
    assert_eq!(caps["documentRangeFormattingProvider"], true);
    assert!(caps.get("documentOnTypeFormattingProvider").is_none());
    let uri = "untitled:formatting".parse().unwrap();
    client.open(&uri, 1, "old:int");
    let source = "repeat 2 {line \"é😀\" x:int}";
    replace(&client, &uri, 2, source);
    let edits = document_edits(&mut client, &uri);
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].range.start, Position::new(0, 0));
    let output = apply(source, &edits, false);
    assert_eq!(output, "repeat 2 {\n  line \"é😀\"\n  x: int\n}\n");
    replace(&client, &uri, 3, &output);
    assert!(document_edits(&mut client, &uri).is_empty());
    assert!(
        range_edits(
            &mut client,
            &uri,
            Range::new(Position::new(2, 2), Position::new(2, 3)),
        )
        .is_empty()
    );
    client.shutdown();
}

#[test]
fn refuses_document_errors_but_formats_a_clean_recovered_statement() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:broken-formatting".parse().unwrap();
    let source = "let bad=+\ngood:int\nlast:int";
    client.open(&uri, 1, source);
    assert!(document_edits(&mut client, &uri).is_empty());
    assert!(
        range_edits(
            &mut client,
            &uri,
            Range::new(Position::new(0, 0), Position::new(0, 7)),
        )
        .is_empty()
    );
    let edits = range_edits(
        &mut client,
        &uri,
        Range::new(Position::new(1, 0), Position::new(1, 4)),
    );
    assert_eq!(edits.len(), 1);
    assert_eq!(
        apply(source, &edits, false),
        "let bad=+\ngood: int\nlast:int"
    );
    client.shutdown();
}

#[test]
fn uses_editorconfig_for_the_file_uri_and_formats_the_buffer() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("reader specs é");
    fs::create_dir(&directory).unwrap();
    let config = root.path().join(".editorconfig");
    fs::write(&config, "root = true\n[*.iris]\nmax_line_length = 40\n").unwrap();
    let path = directory.join("test file.iris");
    fs::write(&path, "on_disk:int").unwrap();
    let uri = file_uri(&path);
    let mut client = Client::new(json!({}));
    let source = "let total=sum(first_value,second_value,third_value)";
    let wrapped = "let total = sum(\n  first_value,\n  second_value,\n  third_value\n)\n";
    client.open(&uri, 1, source);
    assert_eq!(
        apply(source, &document_edits(&mut client, &uri), false),
        wrapped
    );
    assert_eq!(
        apply(
            source,
            &range_edits(
                &mut client,
                &uri,
                Range::new(Position::new(0, 0), Position::new(0, 3)),
            ),
            false
        ),
        wrapped
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "on_disk:int");

    // Both authorities name the same local path. Encoded slashes also decode
    // to path separators, preserving the former Unix URL conversion behavior.
    for uri in [
        uri.as_str().replacen("file://", "file://localhost", 1),
        uri.as_str().replace("/reader", "%2Freader"),
    ] {
        let uri = uri.parse().unwrap();
        client.open(&uri, 1, source);
        assert_eq!(
            apply(source, &document_edits(&mut client, &uri), false),
            wrapped
        );
    }

    fs::write(&config, "root = true\n[*.iris]\nmax_line_length = off\n").unwrap();
    let flat = "let total = sum(first_value, second_value, third_value)\n";
    assert_eq!(
        apply(source, &document_edits(&mut client, &uri), false),
        flat
    );
    // Non-file documents have no EditorConfig path.
    for uri in [
        "untitled:formatting-default",
        "https://example.test/test.iris",
    ] {
        let uri = uri.parse().unwrap();
        client.open(&uri, 1, source);
        assert_eq!(
            apply(source, &document_edits(&mut client, &uri), false),
            flat
        );
    }
    fs::write(
        &config,
        "root = true\n[*.iris]\nmax_line_length = invalid\n",
    )
    .unwrap();
    let error = client
        .request("textDocument/formatting", params(&uri))
        .response_result
        .unwrap_err();
    assert_eq!(error.code, -32803);
    assert!(
        error
            .message
            .contains("invalid .editorconfig max_line_length")
    );
    client.shutdown();
}

#[test]
fn edits_preserve_formatter_output_with_physical_lines_and_both_encodings() {
    for utf8 in [false, true] {
        let mut client = Client::new(if utf8 {
            json!({"general": {"positionEncodings": ["utf-8"]}})
        } else {
            json!({})
        });
        for (index, newline) in ["\r\n", "\r"].into_iter().enumerate() {
            let uri = format!("untitled:line-endings-{index}").parse().unwrap();
            let source = format!("format demo{newline}x:int # é😀");
            client.open(&uri, 1, &source);
            let edits = document_edits(&mut client, &uri);
            assert_eq!(edits.len(), 1);
            assert_eq!(
                edits[0].range.end,
                Position::new(1, if utf8 { 14 } else { 11 })
            );
            let expected = gecode_iris_syntax::format(
                source.as_bytes(),
                "test",
                LanguageVersion::V0_1,
                FormatOptions::default(),
            )
            .unwrap();
            assert_eq!(apply(&source, &edits, utf8), expected);

            let source = format!("comment \"é😀\" good:int{newline}last:int{newline}let bad=+");
            replace(&client, &uri, 2, &source);
            let start = if utf8 { 17 } else { 14 };
            let edits = range_edits(
                &mut client,
                &uri,
                Range::new(Position::new(0, start), Position::new(0, start + 4)),
            );
            let start = source.find("good").unwrap();
            let formatted = gecode_iris_syntax::format_range(
                source.as_bytes(),
                "test",
                LanguageVersion::V0_1,
                TextRange::new(
                    TextSize::new(start as u32),
                    TextSize::new((start + 4) as u32),
                ),
                FormatOptions::default(),
            )
            .unwrap();
            assert_eq!(edits.len(), 1);
            assert_eq!(
                edits[0].range.start,
                Position::new(0, if utf8 { 16 } else { 13 })
            );
            let extent = std::ops::Range::<usize>::from(formatted.range);
            assert_eq!(
                byte_offset(&source, edits[0].range.start, utf8),
                extent.start
            );
            assert_eq!(byte_offset(&source, edits[0].range.end, utf8), extent.end);
            let mut expected = source.clone();
            for edit in formatted.edits.iter().rev() {
                expected.replace_range(
                    std::ops::Range::<usize>::from(edit.range),
                    &edit.replacement,
                );
            }
            let output = apply(&source, &edits, utf8);
            assert_eq!(output, expected);
            assert_eq!(&output[..extent.start], &source[..extent.start]);
            assert!(output.ends_with(&source[extent.end..]));
        }
        client.shutdown();
    }
}

#[test]
fn range_edits_never_split_a_crlf_pair() {
    for utf8 in [false, true] {
        let mut client = Client::new(if utf8 {
            json!({"general": {"positionEncodings": ["utf-8"]}})
        } else {
            json!({})
        });
        let uri = "untitled:crlf-boundary".parse().unwrap();
        // The formatter's range starts between this CR and LF, which no
        // protocol position can address.
        let source = "# keep\r\nx:int";
        client.open(&uri, 1, source);
        let cursor = Position::new(1, 0);
        let edits = range_edits(&mut client, &uri, Range::new(cursor, cursor));
        let formatted = gecode_iris_syntax::format_range(
            source.as_bytes(),
            "test",
            LanguageVersion::V0_1,
            TextRange::empty(TextSize::new(8)),
            FormatOptions::default(),
        )
        .unwrap();
        assert_eq!(usize::from(formatted.range.start()), 7);
        let mut expected = source.to_owned();
        for edit in formatted.edits.iter().rev() {
            expected.replace_range(
                std::ops::Range::<usize>::from(edit.range),
                &edit.replacement,
            );
        }
        assert_eq!(apply(source, &edits, utf8), expected);
        assert!(expected.starts_with("# keep\r\n"));
        client.shutdown();
    }
}

#[test]
fn refuses_foreign_authorities_and_invalid_file_paths() {
    let mut client = Client::new(json!({}));
    for uri in [
        "file://elsewhere.test/tmp/test.iris",
        "file://user@localhost/tmp/test.iris",
        "file://localhost:80/tmp/test.iris",
        "file:///tmp/test%00.iris",
        "file:relative.iris",
    ] {
        let uri = uri.parse().unwrap();
        client.open(&uri, 1, "x:int");
        for method in ["textDocument/formatting", "textDocument/rangeFormatting"] {
            let mut params = params(&uri);
            params["range"] = serde_json::to_value(Range::default()).unwrap();
            assert_eq!(
                client
                    .request(method, params)
                    .response_result
                    .unwrap_err()
                    .code,
                -32602
            );
        }
    }
    client.shutdown();
}

#[test]
fn unknown_documents_and_invalid_ranges_are_parameter_errors() {
    let mut client = Client::new(json!({}));
    let uri: Uri = "untitled:formatting-errors".parse().unwrap();
    let range = Range::new(Position::new(0, 0), Position::new(0, 1));
    let mut range_params = params(&uri);
    range_params["range"] = serde_json::to_value(range).unwrap();
    for (method, params) in [
        ("textDocument/formatting", params(&uri)),
        ("textDocument/rangeFormatting", range_params),
    ] {
        assert_eq!(
            client
                .request(method, params)
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
    }
    client.open(&uri, 1, "line \"é😀\"\nx:int");
    for range in [
        Range::new(Position::new(1, 0), Position::new(0, 0)),
        Range::new(Position::new(2, 0), Position::new(2, 0)),
        Range::new(Position::new(0, 8), Position::new(0, 9)), // inside the emoji's surrogate pair
    ] {
        let mut params = params(&uri);
        params["range"] = serde_json::to_value(range).unwrap();
        assert_eq!(
            client
                .request("textDocument/rangeFormatting", params)
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
    }
    client.close(&uri);
    assert_eq!(
        client
            .request("textDocument/formatting", params(&uri))
            .response_result
            .unwrap_err()
            .code,
        -32602
    );
    client.shutdown();
}
