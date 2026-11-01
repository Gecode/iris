// Each integration suite uses a different subset of the shared protocol client.
#[allow(dead_code)]
mod support;

use lsp_types::{
    CompletionItem, CompletionTextEdit, Hover, HoverContents, InlayHint, InlayHintLabel,
    MarkupKind, Position, Range, TextDocumentContentChangeEvent, Uri,
};
use serde_json::{Value, json};
use support::Client;

fn position(source: &str, offset: usize, utf8: bool) -> Position {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let tail = prefix.rsplit('\n').next().unwrap();
    Position::new(
        line as u32,
        if utf8 {
            tail.len()
        } else {
            tail.encode_utf16().count()
        } as u32,
    )
}

fn params(uri: &Uri, position: Position) -> Value {
    json!({"textDocument": {"uri": uri}, "position": position})
}

fn hover(client: &mut Client, uri: &Uri, at: Position) -> Option<Hover> {
    serde_json::from_value(
        client
            .request("textDocument/hover", params(uri, at))
            .response_result
            .unwrap(),
    )
    .unwrap()
}

fn hover_text(hover: Hover, kind: MarkupKind) -> String {
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markup")
    };
    assert_eq!(markup.kind, kind);
    markup.value
}

fn completion(client: &mut Client, uri: &Uri, at: Position) -> Vec<CompletionItem> {
    serde_json::from_value(
        client
            .request("textDocument/completion", params(uri, at))
            .response_result
            .unwrap(),
    )
    .unwrap()
}

fn hints(client: &mut Client, uri: &Uri, range: Range) -> Vec<InlayHint> {
    serde_json::from_value(
        client
            .request(
                "textDocument/inlayHint",
                json!({"textDocument": {"uri": uri}, "range": range}),
            )
            .response_result
            .unwrap(),
    )
    .unwrap()
}

fn labels(items: &[CompletionItem]) -> Vec<&str> {
    items.iter().map(|item| item.label.as_str()).collect()
}

fn edit(item: &CompletionItem) -> &lsp_types::TextEdit {
    let Some(CompletionTextEdit::Edit(edit)) = &item.text_edit else {
        panic!("expected text edit")
    };
    edit
}

fn open(client: &mut Client, uri: &Uri, source: &str) {
    client.open(uri, 1, source);
    let diagnostics = client.diagnostics(uri, Some(1)).diagnostics;
    assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
}

#[test]
fn hover_documents_keywords_types_and_contextual_names_in_client_format() {
    for markdown in [false, true] {
        let format = if markdown { "markdown" } else { "plaintext" };
        let other = if markdown { "plaintext" } else { "markdown" };
        let mut client =
            Client::new(json!({"textDocument": {"hover": {"contentFormat": [format, other]}}}));
        let caps = serde_json::to_value(&client.initialization.capabilities).unwrap();
        assert_eq!(caps["hoverProvider"], true);
        assert!(caps["completionProvider"].is_object());
        assert_eq!(caps["inlayHintProvider"], true);
        let uri = "untitled:hover".parse().unwrap();
        let source = "let repeat=1\nvalue:int\nlet result=repeat\ntype natural=0..\nother:natural";
        open(&mut client, &uri, source);
        let kind = if markdown {
            MarkupKind::Markdown
        } else {
            MarkupKind::PlainText
        };
        for (offset, expected) in [
            (0, "07-structure.md#79-conditional-execution-and-let"),
            (
                source.find(":int").unwrap() + 1,
                "06-types-and-values.md#62-scalar-reading-and-numeric-promotion",
            ),
            (source.find("repeat").unwrap(), "int, depth 0"),
            (source.rfind("repeat").unwrap(), "int, depth 0"),
            (
                source.rfind("natural").unwrap(),
                "06-types-and-values.md#65-named-types-and-visibility",
            ),
        ] {
            let text = hover_text(
                hover(&mut client, &uri, position(source, offset, false)).unwrap(),
                kind.clone(),
            );
            assert!(text.contains(expected), "{text}");
        }
        assert!(hover(&mut client, &uri, Position::new(0, 3)).is_none());
        client.shutdown();
    }
}

#[test]
fn hover_and_hints_preserve_unknown_and_ambiguous_facts_and_requested_range() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:shapes".parse().unwrap();
    let source = "scalar:int\nmatrix[2][3]:int\nlet unknown=missing\nnumeric:num\nparam varying=matrix\nif condition {choice:int} else {choice:float}\nlet selected=choice";
    open(&mut client, &uri, source);
    for (name, expected) in [
        ("scalar", "int, depth 0"),
        ("matrix", "int, depth 2"),
        ("unknown", "unknown, depth unknown"),
        ("numeric", "int or float (ambiguous), depth 0"),
        ("varying", "depth 2 or 0 (ambiguous)"),
        ("selected", "int or float (ambiguous), depth 0"),
    ] {
        let at = source.find(name).unwrap();
        let text = hover_text(
            hover(&mut client, &uri, position(source, at, false)).unwrap(),
            MarkupKind::PlainText,
        );
        assert!(text.contains(expected), "{text}");
        let at = position(source, at + name.len(), false);
        let found = hints(&mut client, &uri, Range::new(at, at));
        assert_eq!(found.len(), 1, "{name}: {found:?}");
        assert_eq!(found[0].position, at);
        let InlayHintLabel::String(label) = &found[0].label else {
            panic!("expected text label")
        };
        assert!(label.contains(expected), "{label}");
    }
    // A reference uses its own facts, including the view after repeat collection.
    open(&mut client, &uri, "repeat 1 {field:int}\nlet column=field");
    let text = hover_text(
        hover(&mut client, &uri, Position::new(1, 11)).unwrap(),
        MarkupKind::PlainText,
    );
    assert_eq!(text, "field: int, depth 1");
    client.shutdown();
}

#[test]
fn completion_selects_statement_type_expression_and_layout_candidates() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:completion".parse().unwrap();
    for (source, expected, absent) in [
        (
            "",
            vec!["let", "repeat", "format", "type", "instances"],
            vec!["int", "sum", "each"],
        ),
        ("rep", vec!["repeat"], vec!["let"]),
        ("let previous=1\n", vec!["let", "repeat"], vec!["sum"]),
        ("ignore trailing\n", vec!["let"], vec!["trailing"]),
        ("skip line\n", vec!["let"], vec!["lines"]),
        (
            "list xs:int until 0\n",
            vec!["let", "offset"],
            vec!["sum", "eol"],
        ),
        (
            "if condition {\n ",
            vec!["let", "repeat"],
            vec!["format", "sum", "instances", "each"],
        ),
        (
            "repeat 1 {\n ",
            vec!["each", "let"],
            vec!["instances", "format"],
        ),
        (
            "type natural=0..\nvalue: ",
            vec!["int", "float", "num", "word", "rest", "alphabet", "natural"],
            vec!["sum", "string"],
        ),
        ("type natural=0..\nvalue: nat", vec!["natural"], vec!["int"]),
        (
            "let previous=1\nlet target= ",
            vec!["previous", "sum", "count", "all", "gecode.int_max"],
            vec!["target", "int", "let"],
        ),
        (
            "m[2][2]:int layout ",
            vec!["FULL_MATRIX", "UPPER_ROW", "LOWER_DIAG_COL"],
            vec!["int", "sum"],
        ),
    ] {
        client.open(&uri, 1, source);
        client.diagnostics(&uri, Some(1));
        let items = completion(&mut client, &uri, position(source, source.len(), false));
        let names = labels(&items);
        for expected in expected {
            assert!(
                names.contains(&expected),
                "{source}: {names:?} missing {expected}"
            );
        }
        for absent in absent {
            assert!(
                !names.contains(&absent),
                "{source}: {names:?} contains {absent}"
            );
        }
        assert!(
            items
                .iter()
                .all(|item| item.insert_text_format != Some(lsp_types::InsertTextFormat::SNIPPET))
        );
    }
    client.shutdown();
}

#[test]
fn completion_filters_visibility_and_supports_optional_snippets() {
    let mut client = Client::new(
        json!({"textDocument": {"completion": {"completionItem": {"snippetSupport": true, "documentationFormat": ["markdown"]}}}}),
    );
    let uri = "untitled:scope".parse().unwrap();
    let source = "type outer=0..\nif condition {type hidden=0..}\nrepeat 1 {field:int}\nlet previous=1\nlet result=sum(previous)\nlet future=2";
    open(&mut client, &uri, source);
    let items = completion(
        &mut client,
        &uri,
        position(source, source.find("sum").unwrap(), false),
    );
    let names = labels(&items);
    for name in ["previous", "field", "sum"] {
        assert!(names.contains(&name), "{names:?}");
    }
    for name in ["result", "future", "outer", "hidden"] {
        assert!(!names.contains(&name), "{names:?}");
    }
    let sum = items.iter().find(|item| item.label == "sum").unwrap();
    assert_eq!(
        sum.insert_text_format,
        Some(lsp_types::InsertTextFormat::SNIPPET)
    );
    assert_eq!(edit(sum).new_text, "sum(${1})$0");
    let Some(lsp_types::Documentation::MarkupContent(doc)) = &sum.documentation else {
        panic!("expected documentation")
    };
    assert_eq!(doc.kind, MarkupKind::Markdown);
    assert!(
        doc.value
            .contains("[Specification](https://github.com/Gecode/iris/blob/main/spec/0.1/")
    );
    client.open(&uri, 1, &format!("{source}\nx: "));
    client.diagnostics(&uri, Some(1));
    let items = completion(&mut client, &uri, Position::new(6, 3));
    assert!(labels(&items).contains(&"outer"));
    assert!(!labels(&items).contains(&"hidden"));
    let source = "a[1]:int check all(binding in a:binding>0)";
    open(&mut client, &uri, source);
    let items = completion(
        &mut client,
        &uri,
        position(source, source.rfind("binding").unwrap(), false),
    );
    assert!(labels(&items).contains(&"binding"));
    let items = completion(
        &mut client,
        &uri,
        position(source, source.find("in a").unwrap() + 3, false),
    );
    assert!(!labels(&items).contains(&"binding"));
    let source = "a[1]:int check all(binding in binding";
    client.open(&uri, 1, source);
    client.diagnostics(&uri, Some(1));
    let items = completion(&mut client, &uri, position(source, source.len(), false));
    assert!(!labels(&items).contains(&"binding"));
    client.shutdown();
}

#[test]
fn assistance_ranges_use_both_encodings_after_non_ascii_text() {
    for utf8 in [false, true] {
        let mut client = Client::new(
            json!({"general": {"positionEncodings": [if utf8 {"utf-8"} else {"utf-16"}]}}),
        );
        let uri = "untitled:unicode-assist".parse().unwrap();
        let source = "let text=\"é😀\" let amount=1 let result=amount";
        open(&mut client, &uri, source);
        let offset = source.rfind("amount").unwrap();
        let range = Range::new(
            position(source, offset, utf8),
            position(source, offset + 6, utf8),
        );
        let found = hover(&mut client, &uri, range.start).unwrap();
        assert_eq!(found.range, Some(range));
        assert_eq!(
            hover_text(found, MarkupKind::PlainText),
            "amount: int, depth 0"
        );
        let items = completion(&mut client, &uri, position(source, offset + 2, utf8));
        let amount = items.iter().find(|item| item.label == "amount").unwrap();
        assert_eq!(edit(amount).range, range);
        assert_eq!(edit(amount).new_text, "amount");
        let offset = source.find("amount").unwrap() + 6;
        let at = position(source, offset, utf8);
        assert_eq!(hints(&mut client, &uri, Range::new(at, at))[0].position, at);
        let source = "let text=\"é😀\" m[2][2]:int layout \"UP\"";
        open(&mut client, &uri, source);
        let start = source.find("\"UP").unwrap();
        let items = completion(&mut client, &uri, position(source, start + 3, utf8));
        assert_eq!(items.len(), 4);
        assert!(items.iter().all(|item| edit(item).range
            == Range::new(
                position(source, start, utf8),
                position(source, source.len(), utf8)
            )));
        assert_eq!(edit(&items[0]).new_text, "\"UPPER_ROW\"");
        client.shutdown();
    }
}

#[test]
fn assistance_uses_latest_buffer_and_validates_requests() {
    let mut client = Client::new(json!({}));
    let uri: Uri = "untitled:assist-changes".parse().unwrap();
    let at = Position::new(0, 4);
    let range = Range::new(Position::new(0, 0), Position::new(0, 99));
    for method in [
        "textDocument/hover",
        "textDocument/completion",
        "textDocument/inlayHint",
    ] {
        let mut request = params(&uri, at);
        request["range"] = json!(range);
        assert_eq!(
            client
                .request(method, request)
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
    }
    open(&mut client, &uri, "let item=1");
    client.change(
        &uri,
        2,
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "let item=\"string\"".into(),
        }],
    );
    assert_eq!(
        hover_text(hover(&mut client, &uri, at).unwrap(), MarkupKind::PlainText),
        "item: string, depth 0"
    );
    assert!(matches!(&hints(&mut client, &uri, range)[0].label,
        InlayHintLabel::String(label) if label == ": string, depth 0"));
    for method in [
        "textDocument/hover",
        "textDocument/completion",
        "textDocument/inlayHint",
    ] {
        assert_eq!(
            client
                .request(method, json!({}))
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
        let mut request = params(&uri, Position::new(99, 0));
        request["range"] = json!(Range::new(Position::new(99, 0), Position::new(99, 1)));
        assert_eq!(
            client
                .request(method, request)
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
    }
    client.close(&uri);
    assert_eq!(
        client
            .request("textDocument/hover", params(&uri, at))
            .response_result
            .unwrap_err()
            .code,
        -32602
    );
    client.shutdown();
}

fn completion_at_cursor(client: &mut Client, uri: &Uri, marked: &str) -> Vec<CompletionItem> {
    let offset = marked.find('|').expect("source has a cursor marker");
    let source = marked.replacen('|', "", 1);
    client.open(uri, 1, &source);
    client.diagnostics(uri, Some(1));
    completion(client, uri, position(&source, offset, false))
}

#[test]
fn completion_offers_contextual_option_keywords() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:continuations".parse().unwrap();
    for (source, expected) in [
        ("ignore t|", "trailing"),
        ("list xs:int u|", "until"),
        ("repeat until e|", "eof"),
    ] {
        let items = completion_at_cursor(&mut client, &uri, source);
        assert_eq!(labels(&items), vec![expected], "{source}");
    }
    client.shutdown();
}

#[test]
fn completion_recognizes_incomplete_counts_and_dimensions() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:incomplete-expressions".parse().unwrap();
    for source in ["let previous=1\nskip |", "let previous=1\nx[ |]:int"] {
        let items = completion_at_cursor(&mut client, &uri, source);
        let names = labels(&items);
        assert!(names.contains(&"previous"), "{source}: {names:?}");
        assert!(names.contains(&"sum"), "{source}: {names:?}");
        assert!(!names.contains(&"let"), "{source}: {names:?}");
    }
    client.shutdown();
}

#[test]
fn completion_excludes_sibling_branches_and_keeps_enclosing_values() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:exclusive-branches".parse().unwrap();
    for source in [
        "param condition=0\nif condition {let secret=2} else {let after=|0}",
        "param condition=0\nif condition {let secret=2} else if condition {let after=|0}",
    ] {
        let items = completion_at_cursor(&mut client, &uri, source);
        let names = labels(&items);
        assert!(!names.contains(&"secret"), "{source}: {names:?}");
        assert!(names.contains(&"condition"), "{source}: {names:?}");
    }
    let items = completion_at_cursor(
        &mut client,
        &uri,
        "let secret=1\nparam condition=0\nif condition {let secret=2} else {let after=|0}",
    );
    assert!(labels(&items).contains(&"secret"));
    client.shutdown();
}

#[test]
fn completion_offers_each_record_names_in_later_checks_only() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:each-completion".parse().unwrap();
    let items = completion_at_cursor(
        &mut client,
        &uri,
        "repeat 1 {each row x:int}\ncheck |row.x>0",
    );
    assert!(labels(&items).contains(&"row"));
    let items = completion_at_cursor(
        &mut client,
        &uri,
        "repeat 1 {each row x:int}\nlet result=|x",
    );
    assert!(!labels(&items).contains(&"row"));
    // A structural `_` names its records; a discarded read names nothing.
    let items = completion_at_cursor(&mut client, &uri, "repeat 1 {each _ x:int}\r\ncheck |_.x>0");
    assert!(labels(&items).contains(&"_"));
    let items = completion_at_cursor(&mut client, &uri, "_:int\ncheck |");
    assert!(!labels(&items).contains(&"_"));
    // An identifier ending at the cursor is completed even when punctuation
    // follows it directly.
    for (marked, name) in [
        ("repeat 1 {each row x:int}\ncheck ro|.x>0", "row"),
        ("repeat 1 {each _ x:int}\r\ncheck _|.x>0", "_"),
    ] {
        let items = completion_at_cursor(&mut client, &uri, marked);
        assert!(labels(&items).contains(&name), "{marked}");
    }
    let items = completion_at_cursor(&mut client, &uri, "repeat 1 as _ {x:int}\ncheck size(_|)>0");
    let item = items.iter().find(|item| item.label == "_").unwrap();
    assert_eq!(
        edit(item).range,
        Range::new(Position::new(1, 11), Position::new(1, 12))
    );
    client.shutdown();
}
