// Each integration suite uses a different subset of the shared protocol client.
#[allow(dead_code)]
mod support;

use lsp_types::{
    Location, OneOf, Position, Range, TextDocumentContentChangeEvent, TextEdit, Uri, WorkspaceEdit,
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

fn range(source: &str, start: usize, length: usize, utf8: bool) -> Range {
    Range::new(
        position(source, start, utf8),
        position(source, start + length, utf8),
    )
}

fn params(uri: &Uri, position: Position) -> Value {
    json!({"textDocument": {"uri": uri}, "position": position})
}

fn definitions(client: &mut Client, uri: &Uri, position: Position) -> Vec<Range> {
    let response = client.request("textDocument/definition", params(uri, position));
    let locations: Vec<Location> =
        serde_json::from_value(response.response_result.unwrap()).unwrap();
    locations
        .into_iter()
        .map(|location| {
            assert_eq!(location.uri, *uri);
            location.range
        })
        .collect()
}

fn references(
    client: &mut Client,
    uri: &Uri,
    position: Position,
    include_declaration: bool,
) -> Vec<Range> {
    let mut params = params(uri, position);
    params["context"] = json!({"includeDeclaration": include_declaration});
    let locations: Vec<Location> = serde_json::from_value(
        client
            .request("textDocument/references", params)
            .response_result
            .unwrap(),
    )
    .unwrap();
    locations
        .into_iter()
        .map(|location| {
            assert_eq!(location.uri, *uri);
            location.range
        })
        .collect()
}

fn rename_params(uri: &Uri, position: Position, new_name: &str) -> Value {
    let mut params = params(uri, position);
    params["newName"] = json!(new_name);
    params
}

fn rename(client: &mut Client, uri: &Uri, position: Position, new_name: &str) -> Vec<TextEdit> {
    let edit: WorkspaceEdit = serde_json::from_value(
        client
            .request(
                "textDocument/rename",
                rename_params(uri, position, new_name),
            )
            .response_result
            .unwrap(),
    )
    .unwrap();
    assert_eq!(edit.changes.as_ref().unwrap().len(), 1);
    edit.changes.unwrap().remove(uri).unwrap()
}

fn assert_edits(edits: &[TextEdit], expected: Vec<Range>, new_name: &str) {
    assert_eq!(
        edits.iter().map(|edit| edit.range).collect::<Vec<_>>(),
        expected
    );
    assert!(edits.iter().all(|edit| edit.new_text == new_name));
}

fn open(client: &mut Client, uri: &Uri, source: &str) {
    client.open(uri, 1, source);
    assert!(
        client.diagnostics(uri, Some(1)).diagnostics.is_empty(),
        "{source}"
    );
}

#[test]
fn capabilities_and_navigation_cover_values_types_and_records() {
    let mut client = Client::new(json!({}));
    let capabilities = &client.initialization.capabilities;
    assert_eq!(capabilities.definition_provider, Some(OneOf::Left(true)));
    assert_eq!(capabilities.references_provider, Some(OneOf::Left(true)));
    assert!(
        matches!(&capabilities.rename_provider, Some(OneOf::Right(options)) if options.prepare_provider == Some(true))
    );
    let uri = "untitled:navigation".parse().unwrap();
    let source = "type natural=0..\nlet amount=1\nvalue:natural\nlet twice=amount+amount\nrepeat 1 as rows {each row field:int}\ncheck row.field == rows[0].field\nline \"{item:natural}\"";
    open(&mut client, &uri, source);
    for (name, declaration, usage) in [
        (
            "amount",
            source.find("amount").unwrap(),
            source.rfind("amount").unwrap(),
        ),
        (
            "natural",
            source.find("natural").unwrap(),
            source.rfind("natural").unwrap(),
        ),
        (
            "rows",
            source.find("rows").unwrap(),
            source.rfind("rows").unwrap(),
        ),
        (
            "row",
            source.find("row field").unwrap(),
            source.find("row.field").unwrap(),
        ),
        (
            "field",
            source.find("field").unwrap(),
            source.rfind("field").unwrap(),
        ),
    ] {
        let definition = range(source, declaration, name.len(), false);
        let at = position(source, usage, false);
        assert_eq!(
            definitions(&mut client, &uri, at),
            vec![definition],
            "{name}"
        );
        assert_eq!(
            definitions(&mut client, &uri, definition.start),
            vec![definition]
        );
        let uses = references(&mut client, &uri, at, false);
        assert!(!uses.is_empty());
        assert!(uses.contains(&range(source, usage, name.len(), false)));
        let mut with_declaration = vec![definition];
        with_declaration.extend(&uses);
        assert_eq!(references(&mut client, &uri, at, true), with_declaration);
        let edits = rename(&mut client, &uri, at, &format!("renamed_{name}"));
        assert_edits(&edits, with_declaration, &format!("renamed_{name}"));
    }
    client.shutdown();
}

#[test]
fn rename_closes_transitively_over_shared_references() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:group".parse().unwrap();
    let source = "let x=1\nif first {let x=2}\nlet middle=x\nif second {let x=3}\nlet result=x";
    open(&mut client, &uri, source);
    let xs: Vec<_> = source
        .match_indices('x')
        .map(|(offset, _)| range(source, offset, 1, false))
        .collect();
    assert_eq!(
        definitions(&mut client, &uri, xs[2].start),
        vec![xs[0], xs[1]]
    );
    assert_eq!(
        definitions(&mut client, &uri, xs[4].start),
        vec![xs[0], xs[1], xs[3]]
    );
    // References follow the selected definitions; rename alone closes transitively.
    assert_eq!(
        references(&mut client, &uri, xs[0].start, false),
        vec![xs[2], xs[4]]
    );
    assert_eq!(
        references(&mut client, &uri, xs[2].start, false),
        vec![xs[2], xs[4]]
    );
    for selected in &xs {
        let prepared = client
            .request("textDocument/prepareRename", params(&uri, selected.start))
            .response_result
            .unwrap();
        assert_eq!(
            serde_json::from_value::<Range>(prepared).unwrap(),
            *selected
        );
        assert_edits(
            &rename(&mut client, &uri, selected.start, "longer_name"),
            xs.clone(),
            "longer_name",
        );
    }
    client.shutdown();
}

#[test]
fn both_if_branches_are_possible_definitions() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:branches".parse().unwrap();
    let source = "if condition {value:int} else {value:float}\nlet result=value";
    open(&mut client, &uri, source);
    let values: Vec<_> = source
        .match_indices("value")
        .map(|(offset, _)| range(source, offset, 5, false))
        .collect();
    assert_eq!(definitions(&mut client, &uri, values[2].start), values[..2]);
    assert_eq!(references(&mut client, &uri, values[2].start, true), values);
    assert_edits(
        &rename(&mut client, &uri, values[1].start, "v"),
        values,
        "v",
    );
    client.shutdown();
}

#[test]
fn captures_use_raw_ranges_and_pre_match_values_in_both_encodings() {
    for utf8 in [false, true] {
        let mut client = Client::new(if utf8 {
            json!({"general": {"positionEncodings": ["utf-8"]}})
        } else {
            json!({})
        });
        let uri = "untitled:captures".parse().unwrap();
        let source = "let x=1\nline \"é😀{$x} {x:\\n0..x}\"\nlet result=x";
        open(&mut client, &uri, source);
        let inside_scalar = Position::new(1, if utf8 { 7 } else { 8 });
        for at in [inside_scalar, Position::new(99, 0)] {
            assert_eq!(
                client
                    .request("textDocument/definition", params(&uri, at))
                    .response_result
                    .unwrap_err()
                    .code,
                -32602
            );
        }
        let xs: Vec<_> = source
            .match_indices('x')
            .map(|(offset, _)| range(source, offset, 1, utf8))
            .collect();
        assert_eq!(definitions(&mut client, &uri, xs[1].start), vec![xs[0]]);
        assert_eq!(definitions(&mut client, &uri, xs[3].start), vec![xs[2]]);
        assert_eq!(references(&mut client, &uri, xs[2].start, true), xs[2..]);
        assert_edits(
            &rename(&mut client, &uri, xs[1].start, "previous"),
            xs[..2].to_vec(),
            "previous",
        );
        assert_edits(
            &rename(&mut client, &uri, xs[2].start, "captured"),
            xs[2..].to_vec(),
            "captured",
        );
        // The earlier definition has the same spelling but is outside this group.
        assert!(rename(&mut client, &uri, xs[2].start, "x").is_empty());
        client.shutdown();
    }
}

#[test]
fn clashing_renames_are_refused_without_changing_the_document() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:clashes".parse().unwrap();
    for (source, selected, new_name) in [
        ("x:int taken:int", "x", "taken"),
        ("taken:int repeat 1 {x:int}", "x", "taken"),
        ("let x=1 let y=target", "x", "target"),
        ("type t=0.. x:int", "x", "t"),
        ("type first=0.. type second=1..", "first", "second"),
        (
            "repeat 1 as rows {x:int} repeat 1 as other {y:int}",
            "rows",
            "other",
        ),
        ("a[1]:int check all(x in a:all(y in a:x==y))", "x", "y"),
    ] {
        open(&mut client, &uri, source);
        let start = position(source, source.find(selected).unwrap(), false);
        let response = client.request("textDocument/rename", rename_params(&uri, start, new_name));
        assert_eq!(
            response.response_result.unwrap_err().code,
            -32803,
            "{source}"
        );
        assert_eq!(
            definitions(&mut client, &uri, start),
            vec![range(
                source,
                source.find(selected).unwrap(),
                selected.len(),
                false
            )]
        );
    }
    client.shutdown();
}

#[test]
fn sibling_type_and_binding_scopes_can_reuse_names() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:scopes".parse().unwrap();
    for (source, selected, new_name) in [
        (
            "if a {type first=0.. x:first} else {type second=0.. y:second}",
            "first",
            "second",
        ),
        ("a[1]:int check all(x in a:x) && all(y in a:y)", "x", "y"),
    ] {
        open(&mut client, &uri, source);
        let edits = rename(
            &mut client,
            &uri,
            position(source, source.find(selected).unwrap(), false),
            new_name,
        );
        assert_edits(
            &edits,
            source
                .match_indices(selected)
                .map(|(offset, _)| range(source, offset, selected.len(), false))
                .collect(),
            new_name,
        );
    }
    client.shutdown();
}

#[test]
fn invalid_identifiers_and_forbidden_keywords_are_refused_contextually() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:identifiers".parse().unwrap();
    open(&mut client, &uri, "let x=1");
    for new_name in ["", "two words", "2x", "é", "gecode.int_max", "_"] {
        assert!(
            client
                .request(
                    "textDocument/rename",
                    rename_params(&uri, Position::new(0, 4), new_name)
                )
                .response_result
                .is_err(),
            "{new_name}"
        );
    }
    for (source, at, new_name) in [
        ("x:int", Position::new(0, 0), "format"),
        ("type t=0..", Position::new(0, 5), "int"),
        ("type t=0..", Position::new(0, 5), "alphabet"),
        ("x:int let y=x", Position::new(0, 0), "_"),
    ] {
        open(&mut client, &uri, source);
        assert!(
            client
                .request("textDocument/rename", rename_params(&uri, at, new_name))
                .response_result
                .is_err()
        );
    }
    let source = "x:int let y=x";
    open(&mut client, &uri, source);
    assert_eq!(
        rename(&mut client, &uri, Position::new(0, 0), "line").len(),
        2
    );
    let source = "if condition {x:int}";
    open(&mut client, &uri, source);
    assert_eq!(
        rename(&mut client, &uri, Position::new(0, 14), "format").len(),
        1
    );
    client.shutdown();
}

#[test]
fn non_names_unknown_documents_and_bad_parameters_have_no_edits() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:invalid".parse().unwrap();
    let source = "let x=gecode.int_max # x\nlet y=missing";
    open(&mut client, &uri, source);
    for offset in [
        0,
        source.find('=').unwrap(),
        source.find("gecode").unwrap(),
        source.find("# x").unwrap() + 2,
        source.find("missing").unwrap(),
    ] {
        let at = position(source, offset, false);
        assert!(definitions(&mut client, &uri, at).is_empty());
        assert!(references(&mut client, &uri, at, true).is_empty());
        assert_eq!(
            client
                .request("textDocument/prepareRename", params(&uri, at))
                .response_result
                .unwrap(),
            Value::Null
        );
        assert!(
            client
                .request("textDocument/rename", rename_params(&uri, at, "fresh"))
                .response_result
                .is_err()
        );
    }
    for method in [
        "textDocument/definition",
        "textDocument/references",
        "textDocument/prepareRename",
        "textDocument/rename",
    ] {
        assert_eq!(
            client
                .request(method, Value::Null)
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
    }
    client.close(&uri);
    for method in [
        "textDocument/definition",
        "textDocument/references",
        "textDocument/prepareRename",
        "textDocument/rename",
    ] {
        let mut request = rename_params(&uri, Position::new(0, 4), "fresh");
        request["context"] = json!({"includeDeclaration": true});
        assert_eq!(
            client
                .request(method, request)
                .response_result
                .unwrap_err()
                .code,
            -32602
        );
    }
    client.shutdown();
}

#[test]
fn requests_use_latest_text_before_debounce_and_ignore_stale_changes() {
    let mut client = Client::new(json!({}));
    let uri = "untitled:versions".parse().unwrap();
    open(&mut client, &uri, "x:int let y=x");
    client.change(
        &uri,
        3,
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "long_name:int let y=long_name".into(),
        }],
    );
    client.change(
        &uri,
        2,
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "stale:int".into(),
        }],
    );
    assert_eq!(
        definitions(&mut client, &uri, Position::new(0, 22)),
        vec![Range::new(Position::new(0, 0), Position::new(0, 9))]
    );
    let edits = rename(&mut client, &uri, Position::new(0, 22), "fresh");
    assert_eq!(edits.len(), 2);
    assert!(client.diagnostics(&uri, Some(3)).diagnostics.is_empty());
    client.open(&uri, 4, "x:");
    client.diagnostics(&uri, Some(4));
    assert!(
        client
            .request(
                "textDocument/rename",
                rename_params(&uri, Position::new(0, 0), "fresh")
            )
            .response_result
            .is_err()
    );
    client.shutdown();
}
