use serde_json::{Value, json};
fn spec_request(spec: &str) -> String {
    json!({"language":"0.1","spec":spec,"specOrigin":"spec.iris"}).to_string()
}
fn read(spec: &str, input: &str) -> Value {
    serde_json::from_str(&iris_wasm::read(&json!({"language":"0.1","spec":spec,"specOrigin":"spec.iris","input":input,"inputOrigin":"input.txt","params":[]}).to_string())).unwrap()
}
#[test]
fn exact_documents_and_decimal_table_values() {
    let source = "a: int\nb: int\nc: int\n";
    let input = "9007199254740993 9223372036854775807 -9223372036854775808";
    let output = read(source, input);
    assert_eq!(output["kind"], "success");
    let spec = iris_core::Spec::parse(source.as_bytes(), "spec.iris").unwrap();
    let data = spec
        .read(input.as_bytes(), "input.txt", &iris_core::Params::new())
        .unwrap();
    assert_eq!(
        output["json"],
        String::from_utf8(data.to_json().unwrap()).unwrap()
    );
    assert_eq!(
        output["dzn"],
        String::from_utf8(data.to_dzn().unwrap()).unwrap()
    );
    for (index, number) in input.split_whitespace().enumerate() {
        assert_eq!(output["table"]["entries"][index]["rows"][0]["text"], number);
    }
}
#[test]
fn errors_keep_cli_bytes_and_host_distinctions() {
    let result = read("x: int", "bad");
    assert_eq!(result["kind"], "language");
    assert!(result["diagnostic"].as_str().unwrap().contains('^'));
    let spec = iris_core::Spec::parse(b"x: int", "spec.iris").unwrap();
    let iris_core::Failure::Language(error) = spec
        .read(b"bad", "input.txt", &iris_core::Params::new())
        .unwrap_err()
    else {
        panic!()
    };
    assert_eq!(result["diagnostic"], format!("{error}\n"));
    assert_eq!(
        result["errorJson"],
        String::from_utf8(error.to_json().unwrap()).unwrap()
    );
    for request in [
        "{}",
        "broken",
        r#"{"language":"1.0","spec":"","specOrigin":"x"}"#,
    ] {
        assert_eq!(
            serde_json::from_str::<Value>(&iris_wasm::check(request)).unwrap()["kind"],
            "host"
        );
    }
    assert!(result.get("json").is_none());
}
#[test]
fn checking_and_formatting_have_separate_validity() {
    let source = "x:missing";
    let check: Value = serde_json::from_str(&iris_wasm::check(&spec_request(source))).unwrap();
    assert_eq!(check["code"], "E-SPEC-UNKNOWN-TYPE");
    assert!(check["diagnostic"].as_str().unwrap().contains('^'));
    let formatted: Value = serde_json::from_str(&iris_wasm::format(&spec_request(source))).unwrap();
    assert_eq!(formatted["spec"], "x: missing\n");
    let broken: Value = serde_json::from_str(&iris_wasm::format(&spec_request("x:"))).unwrap();
    assert_eq!(broken["kind"], "formatError");
    let unicode = "# 🌸\nx: int\n";
    let output: Value = serde_json::from_str(&iris_wasm::check(&spec_request(unicode))).unwrap();
    assert_eq!(
        output["colors"].as_array().unwrap().last().unwrap()["end"],
        unicode.encode_utf16().count()
    );
}
#[test]
fn preview_retains_ragged_empty_rows_and_limits_wide_arrays() {
    let output = read("repeat 3 { list x: int until 0 }", "0 1 0 2 3 0");
    assert_eq!(output["kind"], "success");
    let rows = output["table"]["entries"][0]["rows"].as_array().unwrap();
    assert!(
        rows.iter()
            .any(|row| row["path"] == json!([0]) && row["length"] == 0)
    );
    assert!(
        rows.iter()
            .any(|row| row["path"] == json!([2, 1]) && row["text"] == "3")
    );
    let wide = read("x[2000]: int", &"1 ".repeat(2000));
    assert_eq!(
        wide["table"]["entries"][0]["rows"]
            .as_array()
            .unwrap()
            .len(),
        1024
    );
    assert_eq!(wide["table"]["truncated"], true);
    let large = read("x: rest", &"a".repeat(600_000));
    assert_eq!(large["kind"], "success");
    assert_eq!(large["table"]["truncated"], true);
    let capped = read("x: rest", &"a\t".repeat(350_000));
    assert_eq!(capped["kind"], "host");
    assert!(capped.get("json").is_none());
}

#[test]
fn deep_preview_stops_before_omitted_subtrees() {
    let source = format!("{}x: int{}", "repeat 1 { ".repeat(20), " }".repeat(20));
    let output = read(&source, "9");
    assert_eq!(output["kind"], "success");
    assert_eq!(output["table"]["truncated"], true);
    let rows = output["table"]["entries"][0]["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 17);
    assert!(
        rows.iter()
            .all(|row| row["path"].as_array().unwrap().len() <= 16)
    );
}
