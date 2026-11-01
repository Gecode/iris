// Each integration suite uses a different subset of the shared protocol client.
#[allow(dead_code)]
mod support;

use lsp_types::{
    DocumentSymbol, FoldingRange, FoldingRangeKind, OneOf, Position, Range, SemanticTokens,
    SymbolInformation, SymbolKind, TextDocumentContentChangeEvent, Uri,
};
use serde_json::{Value, json};
use support::Client;

fn request(client: &mut Client, uri: &Uri, method: &str) -> Value {
    client
        .request(method, json!({"textDocument": {"uri": uri}}))
        .response_result
        .unwrap()
}

fn open(client: &mut Client, uri: &Uri, source: &str) {
    client.open(uri, 1, source);
    assert!(
        client.diagnostics(uri, Some(1)).diagnostics.is_empty(),
        "{source}"
    );
}

#[test]
fn outline_preserves_nested_repeats_sections_arms_and_instances() {
    let mut client = Client::new(json!({"textDocument": {
        "documentSymbol": {
            "hierarchicalDocumentSymbolSupport": true,
            "symbolKind": {"valueSet": [2, 3, 15, 18, 20]}
        }
    }}));
    assert_eq!(
        client.initialization.capabilities.document_symbol_provider,
        Some(OneOf::Left(true))
    );
    let capabilities = serde_json::to_value(&client.initialization.capabilities).unwrap();
    assert_eq!(capabilities["foldingRangeProvider"], true);
    assert_eq!(capabilities["semanticTokensProvider"]["full"], true);
    assert!(
        capabilities["semanticTokensProvider"]
            .get("range")
            .is_none()
    );
    let uri = "untitled:outline".parse().unwrap();
    let source = "instances {\n  repeat 2 as rows {\n    sections {\n      \"é😀 {value:int}\" {\n        repeat 1 {\n          child:int\n        }\n      }\n      \"end\" optional last\n    }\n  }\n}";
    open(&mut client, &uri, source);
    let symbols: Vec<DocumentSymbol> =
        serde_json::from_value(request(&mut client, &uri, "textDocument/documentSymbol")).unwrap();
    assert_eq!(symbols.len(), 1);
    let instances = &symbols[0];
    assert_eq!(instances.name, "instances");
    assert_eq!(instances.kind, SymbolKind::MODULE);
    assert_eq!(
        instances.range,
        Range::new(Position::new(0, 0), Position::new(11, 1))
    );
    assert_eq!(
        instances.selection_range,
        Range::new(Position::new(0, 0), Position::new(0, 9))
    );
    let repeats = instances.children.as_ref().unwrap();
    assert_eq!(repeats.len(), 1);
    assert_eq!(repeats[0].name, "repeat 2 as rows");
    assert_eq!(repeats[0].kind, SymbolKind::ARRAY);
    let sections = &repeats[0].children.as_ref().unwrap()[0];
    assert_eq!(sections.name, "sections");
    assert_eq!(sections.kind, SymbolKind::NAMESPACE);
    let arms = sections.children.as_ref().unwrap();
    assert_eq!(arms.len(), 2);
    assert_eq!(arms[0].name, "\"é😀 {value:int}\"");
    assert_eq!(arms[0].kind, SymbolKind::KEY);
    assert_eq!(
        arms[0].selection_range,
        Range::new(Position::new(3, 6), Position::new(3, 23))
    );
    assert_eq!(arms[0].children.as_ref().unwrap()[0].name, "repeat 1");
    assert_eq!(arms[1].name, "\"end\" optional last");
    assert!(arms[1].children.is_none());
    client.shutdown();
}

#[test]
fn arms_use_a_baseline_symbol_kind_unless_the_client_lists_key() {
    // Without symbolKind.valueSet, LSP 3.17 clients need only accept File
    // through Array.
    for capabilities in [
        json!({}),
        json!({"textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": true}}}),
    ] {
        let mut client = Client::new(capabilities);
        let uri = "untitled:arms".parse().unwrap();
        open(&mut client, &uri, "sections {\n \"x\" optional\n}");
        let symbols = request(&mut client, &uri, "textDocument/documentSymbol");
        let kinds: Vec<_> = if symbols[0].get("children").is_some() {
            serde_json::from_value::<Vec<DocumentSymbol>>(symbols).unwrap()[0]
                .children
                .as_ref()
                .unwrap()
                .iter()
                .map(|symbol| symbol.kind)
                .collect()
        } else {
            serde_json::from_value::<Vec<SymbolInformation>>(symbols)
                .unwrap()
                .iter()
                .map(|symbol| symbol.kind)
                .collect()
        };
        assert!(kinds.contains(&SymbolKind::STRING), "{kinds:?}");
        assert!(!kinds.contains(&SymbolKind::KEY), "{kinds:?}");
        client.shutdown();
    }
}

#[test]
fn folding_covers_blocks_sections_and_separate_comment_runs() {
    for (line_only, utf8) in [(false, false), (false, true), (true, false)] {
        let mut client = Client::new(json!({
            "general": {"positionEncodings": [if utf8 {"utf-8"} else {"utf-16"}]},
            "textDocument": {"foldingRange": {"lineFoldingOnly": line_only}}
        }));
        let uri = "untitled:folds".parse().unwrap();
        let source = "# first\r\n# second\r\n\r\n# isolated\r\nrepeat 2 {\r\n  sections {\r\n    \"é😀\" {\r\n      x:int\r\n    }\r\n  }\r\n}\r\n# last\r\n# pair";
        open(&mut client, &uri, source);
        let folds: Vec<FoldingRange> =
            serde_json::from_value(request(&mut client, &uri, "textDocument/foldingRange"))
                .unwrap();
        assert_eq!(folds.len(), 5);
        let blocks: Vec<_> = folds.iter().filter(|f| f.kind.is_none()).collect();
        assert_eq!(
            blocks
                .iter()
                .map(|f| (f.start_line, f.end_line))
                .collect::<Vec<_>>(),
            if line_only {
                vec![(4, 9), (5, 8), (6, 7)]
            } else {
                vec![(4, 10), (5, 9), (6, 8)]
            }
        );
        let comments: Vec<_> = folds
            .iter()
            .filter(|f| f.kind == Some(FoldingRangeKind::Comment))
            .collect();
        assert_eq!(
            comments
                .iter()
                .map(|f| (f.start_line, f.end_line))
                .collect::<Vec<_>>(),
            vec![(0, 1), (11, 12)]
        );
        if line_only {
            assert!(
                folds
                    .iter()
                    .all(|f| f.start_character.is_none() && f.end_character.is_none())
            );
        } else {
            assert_eq!(
                (blocks[2].start_character, blocks[2].end_character),
                (Some(if utf8 { 14 } else { 11 }), Some(4))
            );
        }
        client.shutdown();
    }
}

#[derive(Debug)]
struct Token {
    line: u32,
    start: u32,
    length: u32,
    kind: String,
    modifiers: Vec<String>,
}

fn tokens(client: &mut Client, uri: &Uri) -> Vec<Token> {
    let legend = serde_json::to_value(&client.initialization.capabilities).unwrap()["semanticTokensProvider"]["legend"].clone();
    let response: SemanticTokens =
        serde_json::from_value(request(client, uri, "textDocument/semanticTokens/full")).unwrap();
    let (mut line, mut start) = (0, 0);
    response
        .data
        .into_iter()
        .map(|token| {
            line += token.delta_line;
            start = if token.delta_line == 0 {
                start + token.delta_start
            } else {
                token.delta_start
            };
            assert!(token.length > 0);
            Token {
                line,
                start,
                length: token.length,
                kind: legend["tokenTypes"][token.token_type as usize]
                    .as_str()
                    .unwrap()
                    .into(),
                modifiers: legend["tokenModifiers"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| token.token_modifiers_bitset & (1 << i) != 0)
                    .map(|(_, name)| name.as_str().unwrap().into())
                    .collect(),
            }
        })
        .collect()
}

fn assert_token(
    tokens: &[Token],
    line: u32,
    start: u32,
    length: u32,
    kind: &str,
    declaration: bool,
) {
    let token = tokens
        .iter()
        .find(|t| t.line == line && t.start == start)
        .unwrap_or_else(|| panic!("missing token at {line}:{start}: {tokens:?}"));
    assert_eq!((token.length, token.kind.as_str()), (length, kind));
    assert_eq!(
        token.modifiers.iter().any(|m| m == "declaration"),
        declaration
    );
}

#[test]
fn semantic_tokens_distinguish_contextual_names_and_pattern_names_in_both_encodings() {
    for utf8 in [false, true] {
        let mut client = Client::new(if utf8 {
            json!({"general": {"positionEncodings": ["utf-8"]}})
        } else {
            json!({})
        });
        let uri = "untitled:tokens".parse().unwrap();
        let source = "type natural=0..\nlet repeat=1\nline \"é😀{$repeat} {item:natural}\"\nlet result=item+repeat # tail\nparam amount=1\ncheck amount>0";
        open(&mut client, &uri, source);
        let tokens = tokens(&mut client, &uri);
        assert_token(&tokens, 0, 0, 4, "keyword", false);
        assert_token(&tokens, 0, 5, 7, "type", true);
        assert_token(&tokens, 1, 4, 6, "variable", true);
        assert_token(&tokens, 2, 0, 4, "keyword", false);
        assert_token(&tokens, 2, if utf8 { 14 } else { 11 }, 6, "variable", false);
        assert_token(&tokens, 2, if utf8 { 23 } else { 20 }, 4, "variable", true);
        assert_token(&tokens, 2, if utf8 { 28 } else { 25 }, 7, "type", false);
        assert_token(&tokens, 3, 11, 4, "variable", false);
        assert_token(&tokens, 3, 16, 6, "variable", false);
        assert_token(&tokens, 3, 23, 6, "comment", false);
        assert_token(&tokens, 4, 6, 6, "parameter", true);
        assert_token(&tokens, 5, 6, 6, "parameter", false);
        client.shutdown();
    }
}

#[test]
fn semantic_tokens_split_multiline_leaves_and_reset_delta_columns() {
    for utf8 in [false, true] {
        for multiline in [false, true] {
            let mut client = Client::new(json!({
                "general": {"positionEncodings": [if utf8 {"utf-8"} else {"utf-16"}]},
                "textDocument": {"semanticTokens": {
                    "requests": {"full": true}, "tokenTypes": [], "tokenModifiers": [], "formats": ["relative"],
                    "multilineTokenSupport": multiline
                }}
            }));
            let uri = "untitled:multiline".parse().unwrap();
            // A literal CR is accepted inside a string, and is a physical source line break.
            let source = "let text=\"é😀\rcontinued\"\nlet count=1";
            open(&mut client, &uri, source);
            let tokens = tokens(&mut client, &uri);
            assert_token(&tokens, 0, 9, if utf8 { 7 } else { 4 }, "string", false);
            assert_token(&tokens, 1, 0, 10, "string", false);
            assert_token(&tokens, 2, 0, 3, "keyword", false);
            assert_token(&tokens, 2, 4, 5, "variable", true);
            // Every wire token fits on its physical line and tokens never overlap.
            let lines: Vec<_> = source.split(['\r', '\n']).collect();
            let mut previous: Option<&Token> = None;
            for token in &tokens {
                let line = lines[token.line as usize];
                let width = if utf8 {
                    line.len()
                } else {
                    line.encode_utf16().count()
                } as u32;
                assert!(token.start + token.length <= width);
                if let Some(previous) = previous {
                    assert!(
                        token.line > previous.line
                            || token.start >= previous.start + previous.length
                    );
                }
                previous = Some(token);
            }
            client.shutdown();
        }
    }
}

#[test]
fn structure_requests_use_current_buffers_and_validate_document_identity() {
    let mut client = Client::new(json!({}));
    let uri: Uri = "untitled:changes".parse().unwrap();
    let methods = [
        "textDocument/documentSymbol",
        "textDocument/foldingRange",
        "textDocument/semanticTokens/full",
    ];
    for method in methods {
        assert_eq!(
            client
                .request(method, json!({"textDocument": {"uri": uri}}))
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
        assert_eq!(
            client
                .request(method, json!({}))
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
    }
    open(&mut client, &uri, "repeat 1 {\n x:int\n}");
    let symbols: Vec<SymbolInformation> =
        serde_json::from_value(request(&mut client, &uri, methods[0])).unwrap();
    assert_eq!(symbols[0].name, "repeat 1");
    assert_eq!(symbols[0].location.uri, uri);
    client.change(
        &uri,
        2,
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "let fresh=2".into(),
        }],
    );
    for method in &methods[..2] {
        assert_eq!(request(&mut client, &uri, method), json!([]));
    }
    let current = tokens(&mut client, &uri);
    assert_token(&current, 0, 4, 5, "variable", true);
    client.close(&uri);
    for method in methods {
        assert_eq!(
            client
                .request(method, json!({"textDocument": {"uri": uri}}))
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
    }
    client.shutdown();
}
