//! Requests on a valid spec whose tree is far deeper than any thread's stack
//! would allow a walk with one frame per level, with the server on a small
//! thread.

// Each integration suite uses a different subset of the shared protocol client.
#[allow(dead_code)]
mod support;

use lsp_types::{Position, Uri};
use serde_json::{Value, json};
use support::Client;

const SMALL_STACK: usize = 2 * 1024 * 1024;

/// A left-nested chain through 254 parentheses where each level adds
/// `suffix` up to `255 - depth` times, the most Specification §5.12 allows:
/// 32,640 suffixes.
fn left_spine(operand: &str, suffix: &str) -> String {
    let mut text = "(".repeat(254);
    text.push_str(operand);
    for depth in (0..=254).rev() {
        text.push_str(&suffix.repeat(255 - depth));
        if depth > 0 {
            text.push(')');
        }
    }
    text
}

fn params(uri: &Uri, extra: Value) -> Value {
    let mut params = json!({"textDocument": {"uri": uri}});
    params
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    params
}

#[test]
fn requests_on_deep_left_operands_fit_a_small_stack() {
    let mut client = Client::with_server(
        json!({"textDocument": {"documentSymbol": {"hierarchicalDocumentSymbolSupport": true}}}),
        |connection| {
            std::thread::Builder::new()
                .stack_size(SMALL_STACK)
                .spawn(move || gecode_iris_lsp::run(connection).map_err(|error| error.to_string()))
                .unwrap()
                .join()
                .unwrap()
        },
    );
    let uri: Uri = "file:///deep.iris".parse().unwrap();
    let source = format!(
        "n: int\nlet s = {}\ncheck {}\ncheck s > 0\n",
        left_spine("1", "+1"),
        left_spine("n", "&&n")
    );
    client.open(&uri, 1, &source);
    assert!(client.diagnostics(&uri, Some(1)).diagnostics.is_empty());
    // The innermost operands of each spine.
    let literal = Position::new(1, 8 + 254);
    let name = Position::new(2, 6 + 254);
    // References to `n` would take time quadratic in the line's length.
    let sum = Position::new(3, 6);
    let whole = json!({"start": {"line": 0, "character": 0}, "end": {"line": 4, "character": 0}});
    let options = json!({"tabSize": 4, "insertSpaces": true});
    for (method, extra) in [
        ("textDocument/documentSymbol", json!({})),
        ("textDocument/foldingRange", json!({})),
        ("textDocument/semanticTokens/full", json!({})),
        ("textDocument/hover", json!({"position": literal})),
        ("textDocument/hover", json!({"position": name})),
        ("textDocument/completion", json!({"position": literal})),
        ("textDocument/definition", json!({"position": name})),
        (
            "textDocument/references",
            json!({"position": sum, "context": {"includeDeclaration": true}}),
        ),
        ("textDocument/inlayHint", json!({"range": whole})),
        ("textDocument/formatting", json!({"options": options})),
        (
            "textDocument/rangeFormatting",
            json!({"range": whole, "options": options}),
        ),
    ] {
        let response = client.request(method, params(&uri, extra));
        assert!(response.response_result.is_ok(), "{method}: {response:?}");
    }
    client.shutdown();
}
