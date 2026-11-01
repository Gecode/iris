use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use iris_conformance::canonical::{Outcome, OutcomeKind};
use iris_conformance::case::{Case, Operation, OutputFormat, discover};
use iris_conformance::report::differences;
use iris_conformance::{CaseStatus, RunOptions, run};
use serde_json::{Value, json};
use tempfile::TempDir;

fn fixtures(group: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(group)
}

fn driver() -> String {
    format!(
        "python3 {}",
        shell_words::quote(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/canned-driver.py")
                .to_string_lossy()
        )
    )
}

fn options(root: PathBuf) -> RunOptions {
    RunOptions {
        driver: driver(),
        driver_id: None,
        roots: vec![root],
        filter: None,
        version: "0.1".into(),
        timeout: Duration::from_secs(3),
    }
}

fn copy_directory(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &target.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}

fn fixture_copy(name: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    copy_directory(&fixtures(name), dir.path());
    dir
}

fn change_control(path: &Path, change: impl FnOnce(&mut Value)) {
    let file = path.join("canned.json");
    let mut control: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    change(&mut control);
    fs::write(file, serde_json::to_vec(&control).unwrap()).unwrap();
}

fn decode(kind: OutcomeKind, text: &str) -> Result<Outcome, String> {
    Outcome::decode(kind, text.as_bytes(), "0.1")
}

fn data(value_type: &str, depth: usize, value: &str) -> String {
    format!(
        r#"{{"iris":"0.1","entries":[{{"name":"x","type":"{value_type}","depth":{depth},"value":{value}}}]}}"#
    )
}

fn error() -> Value {
    json!({"code":"E-EXPECTED-VALUE", "message":"first", "input_location":{"origin":"input.bin","line":2,"column":null}, "spec_location":{"origin":"spec.iris","line":9,"column":8}, "context":null, "record_path":null})
}

fn decode_error(value: &Value) -> Outcome {
    decode(OutcomeKind::Error, &value.to_string()).unwrap()
}

#[test]
fn canonical_data_is_exact_and_preserves_int64_and_empty_ragged_arrays() {
    let left = decode(
        OutcomeKind::Data,
        &data("int", 2, "[[],[9223372036854775807,-9223372036854775808]]"),
    )
    .unwrap();
    let right = decode(OutcomeKind::Data, r#"{ "entries": [{"value":[[],[9223372036854775807,-9223372036854775808]], "depth":2, "type":"int", "name":"x"}], "iris":"0.1" }"#).unwrap();
    assert_eq!(left, right);
    for (kind, depth, value) in [
        ("int", 1, "[]"),
        ("float", 300, "[]"),
        ("string", 2, "[[],[\"λ\",\"\\u0000\"]]"),
    ] {
        decode(OutcomeKind::Data, &data(kind, depth, value)).unwrap();
    }
    let depth = 300;
    let nested = format!("{}7{}", "[".repeat(depth), "]".repeat(depth));
    decode(OutcomeKind::Data, &data("int", depth, &nested)).unwrap();
}

#[test]
fn canonical_data_rejects_invalid_structure_and_leaf_spellings() {
    for document in [
        r#"{"iris":"0.1","iris":"0.1","entries":[]}"#.into(),
        r#"{"iris":"0.1"}"#.into(),
        r#"{"iris":"0.1","entries":[],"extra":0}"#.into(),
        r#"{"iris":"0.1","entries":[{"name":"x","name":"y","type":"int","depth":0,"value":1}]}"#.into(),
        r#"{"iris":"0.1","entries":[{"name":"x","type":"int","depth":0,"value":1},{"name":"x","type":"int","depth":0,"value":2}]}"#.into(),
        data("int", 0, "9223372036854775808"), data("int", 0, "-9223372036854775809"),
        data("int", 0, "-0"), data("int", 0, "1.0"), data("int", 0, "1e0"),
        data("int", 0, "true"), data("int", 0, "null"), data("int", 0, "{}"),
        data("int", 0, "[]"), data("int", 1, "1"), data("int", 2, "[1]"), data("int", 1, "[[1]]"),
        data("string", 0, r#""\ud800""#), data("int", 0, r#""1""#),
        data("int", 0, "7") + " {}", "\u{feff}{\"iris\":\"0.1\",\"entries\":[]}".into(),
    ] {
        assert!(decode(OutcomeKind::Data, &document).is_err(), "accepted {document}");
    }
    assert!(Outcome::decode(OutcomeKind::Data, b"\xff", "0.1").is_err());
}

#[test]
fn canonical_float_spelling_uses_shortest_digits_and_fixed_scientific_choice() {
    for spelling in [
        "0",
        "-0.0",
        "3",
        "100",
        "1e3",
        "0.01",
        "1e-3",
        "1.2345678901234567",
        "1.0000000000000002",
        "5e-324",
        "1.7976931348623157e308",
    ] {
        decode(OutcomeKind::Data, &data("float", 0, spelling)).unwrap();
    }
    for spelling in [
        "-0",
        "0.0",
        "3.0",
        "1000",
        "1E+03",
        "1e+3",
        "1e03",
        "0.001",
        "1e-2",
        "1e309",
        "NaN",
        "Infinity",
        "\"inf\"",
        "\"3\"",
        "1.00000000000000020",
    ] {
        assert!(
            decode(OutcomeKind::Data, &data("float", 0, spelling)).is_err(),
            "accepted {spelling}"
        );
    }
    assert_ne!(
        decode(OutcomeKind::Data, &data("float", 0, "0")).unwrap(),
        decode(OutcomeKind::Data, &data("float", 0, "-0.0")).unwrap()
    );
    assert_ne!(
        decode(OutcomeKind::Data, &data("float", 0, "3")).unwrap(),
        decode(OutcomeKind::Data, &data("int", 0, "3")).unwrap()
    );
}

#[test]
fn data_diffs_name_entry_and_nested_path_and_compare_order_type_depth_and_shape() {
    let expected = decode(OutcomeKind::Data, &data("int", 2, "[[1,2],[3]]")).unwrap();
    let actual = decode(OutcomeKind::Data, &data("int", 2, "[[1,2],[4]]")).unwrap();
    assert!(
        differences(&expected, &actual)
            .join(" ")
            .contains("entry[0] \"x\" value[1][0]: expected 3, actual 4")
    );
    for changed in [
        data("int", 2, "[[1],[3]]"),
        data("int", 1, "[1,2,3]"),
        data("float", 2, "[[1,2],[3]]"),
        data("int", 2, "[[1,2],[3]]").replace("\"x\"", "\"y\""),
    ] {
        let actual = decode(OutcomeKind::Data, &changed).unwrap();
        assert_ne!(expected, actual);
        assert!(!differences(&expected, &actual).is_empty());
    }
    let entries = r#"{"name":"a","type":"int","depth":0,"value":1},{"name":"b","type":"int","depth":0,"value":2}"#;
    let document = format!(r#"{{"iris":"0.1","entries":[{entries}]}}"#);
    let reversed = document
        .replace("\"a\"", "\"temp\"")
        .replace("\"b\"", "\"a\"")
        .replace("\"temp\"", "\"b\"");
    assert_ne!(
        decode(OutcomeKind::Data, &document).unwrap(),
        decode(OutcomeKind::Data, &reversed).unwrap()
    );
}

#[test]
fn errors_compare_primary_coordinates_and_path_only() {
    let baseline = error();
    let expected = decode_error(&baseline);
    let mut ignored = baseline.clone();
    ignored["message"] = json!("other wording");
    ignored["context"] = json!("repeat [3 of 4]");
    ignored["input_location"]["origin"] = json!("elsewhere");
    ignored["spec_location"]["line"] = json!(42);
    assert_eq!(expected, decode_error(&ignored));
    for (field, value) in [
        ("code", json!("E-CHECK-FAILED")),
        ("record_path", json!([])),
    ] {
        let mut changed = baseline.clone();
        changed[field] = value;
        assert!(!differences(&expected, &decode_error(&changed)).is_empty());
    }
    for (field, value) in [("line", json!(3)), ("column", json!(8))] {
        let mut changed = baseline.clone();
        changed["input_location"][field] = value;
        assert!(!differences(&expected, &decode_error(&changed)).is_empty());
    }
    let mut parse = baseline.clone();
    parse["code"] = json!("E-SPEC-SYNTAX");
    parse["input_location"] = Value::Null;
    parse["spec_location"]["column"] = Value::Null;
    let Outcome::Error(parse_key) = decode_error(&parse) else {
        panic!()
    };
    assert_eq!((parse_key.line, parse_key.column), (Some(9), None));
    parse["code"] = json!("E-PARAM-UNKNOWN");
    parse["spec_location"]["line"] = Value::Null;
    let Outcome::Error(key) = decode_error(&parse) else {
        panic!()
    };
    assert_eq!((key.line, key.column), (None, None));
    parse["spec_location"] = Value::Null;
    assert_eq!(decode_error(&parse), Outcome::Error(key));
}

#[test]
fn errors_require_nullable_fields_and_registered_codes_and_valid_locations() {
    for field in [
        "code",
        "message",
        "input_location",
        "spec_location",
        "context",
        "record_path",
    ] {
        let mut document = error();
        document.as_object_mut().unwrap().remove(field);
        assert!(
            decode(OutcomeKind::Error, &document.to_string()).is_err(),
            "missing {field}"
        );
    }
    for field in ["origin", "line", "column"] {
        let mut document = error();
        document["input_location"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(decode(OutcomeKind::Error, &document.to_string()).is_err());
    }
    for (field, value) in [
        ("code", json!("E-UNMAPPED")),
        ("code", json!("E-NOT-REGISTERED")),
        ("record_path", json!([-1])),
        ("extra", json!(null)),
    ] {
        let mut document = error();
        document[field] = value;
        assert!(decode(OutcomeKind::Error, &document.to_string()).is_err());
    }
    for field in ["line", "column"] {
        let mut document = error();
        document["input_location"][field] = json!(0);
        assert!(decode(OutcomeKind::Error, &document.to_string()).is_err());
    }
    assert!(
        decode(
            OutcomeKind::Error,
            &error().to_string().replacen(
                "\"code\":",
                "\"code\":\"E-EXPECTED-VALUE\",\"code\":",
                1
            )
        )
        .is_err()
    );
}

#[test]
fn parse_success_is_a_strict_protocol_document() {
    assert_eq!(
        decode(OutcomeKind::Parse, r#"{"parsed":true,"iris":"0.1"}"#).unwrap(),
        Outcome::Parse
    );
    for invalid in [
        r#"{"iris":"0.1","parsed":false}"#,
        r#"{"iris":"1.0","parsed":true}"#,
        r#"{"iris":"0.1","entries":[]}"#,
        r#"{"iris":"0.1","parsed":true,"extra":0}"#,
        r#"{"iris":"0.1","parsed":true,"parsed":true}"#,
    ] {
        assert!(decode(OutcomeKind::Parse, invalid).is_err());
    }
}

#[test]
fn dzn_is_opaque_and_byte_exact_with_readable_lf_cr_and_nul_diffs() {
    for bytes in [b"".as_slice(), b"x = 7;\n", b"\xff\0\r\n"] {
        assert_eq!(
            Outcome::decode(OutcomeKind::Dzn, bytes, "0.1").unwrap(),
            Outcome::Dzn(bytes.to_vec())
        );
    }
    let expected = Outcome::Dzn(b"x = 7;\n".to_vec());
    for bytes in [
        b"x = 7;".as_slice(),
        b"x = 7;\n\n",
        b"x = 7;\r\n",
        b"x = 7;\0\n",
        b"x=7;\n",
    ] {
        let details = differences(&expected, &Outcome::Dzn(bytes.to_vec())).join(" ");
        assert!(
            details.contains("dzn byte") && details.contains("length") && details.contains("line")
        );
    }
    assert!(differences(&expected, &Outcome::Dzn(b"x = 7;\r\n".to_vec()))[0].contains("\\r\\n"));
}

#[test]
fn case_loading_defaults_and_all_operation_outputs() {
    for name in ["read", "parse", "catalog", "dzn", "catalog-dzn", "error"] {
        let case = Case::load(&fixtures("passing").join(name), "0.1").unwrap();
        assert!(case.spec.is_absolute());
        if name == "parse" {
            assert_eq!(case.operation, Operation::Parse);
            assert!(case.input.is_none());
        }
        if name == "catalog-dzn" {
            assert_eq!(case.output, OutputFormat::Dzn);
        }
    }
    let read = fixture_copy("passing/read");
    fs::write(read.path().join("input.bin"), []).unwrap();
    let case = Case::load(read.path(), "0.1").unwrap();
    assert_eq!(case.operation, Operation::Read);
    assert_eq!(case.output, OutputFormat::Json);
    let parse = fixture_copy("passing/parse");
    // Optional parse input has arbitrary bytes and is never passed to the driver.
    fs::write(parse.path().join("input.bin"), b"\xff\0").unwrap();
    fs::write(parse.path().join("params.json"), "{}\n").unwrap();
    assert!(Case::load(parse.path(), "0.1").unwrap().input.is_none());
}

#[test]
fn driver_receives_spec_and_input_bytes_unchanged() {
    // Source stays UTF-8; arbitrary input bytes still reach the driver unchanged.
    let dir = fixture_copy("passing/read");
    fs::write(dir.path().join("spec.iris"), b"x: int\0\r\n").unwrap();
    fs::write(dir.path().join("input.bin"), b"\0\xff\r\n").unwrap();
    change_control(dir.path(), |control| {
        control["spec_hex"] = json!("783a20696e74000d0a");
        control["input_hex"] = json!("00ff0d0a");
    });
    let report = run(&options(dir.path().into())).unwrap();
    assert_eq!(report.exit_code(), 0, "{}", report.render());
}

#[test]
fn binary_specs_are_discovered_and_passed_unchanged() {
    let dir = fixture_copy("passing/read");
    fs::remove_file(dir.path().join("spec.iris")).unwrap();
    fs::write(dir.path().join("spec.bin"), b"x: int\0\xff\r\n").unwrap();
    change_control(dir.path(), |control| {
        control["spec_hex"] = json!("783a20696e7400ff0d0a");
    });
    assert_eq!(discover(&[dir.path().into()], None).unwrap().len(), 1);
    assert_eq!(
        Case::load(dir.path(), "0.1").unwrap().spec,
        dir.path().canonicalize().unwrap().join("spec.bin")
    );
    let report = run(&options(dir.path().into())).unwrap();
    assert_eq!(report.exit_code(), 0, "{}", report.render());
}

#[test]
fn specs_reject_ambiguous_files_and_invalid_utf8_text() {
    let dir = fixture_copy("passing/read");
    fs::write(dir.path().join("spec.bin"), b"\xff").unwrap();
    assert!(
        Case::load(dir.path(), "0.1")
            .unwrap_err()
            .contains("exactly one spec.iris or spec.bin")
    );
    fs::remove_file(dir.path().join("spec.bin")).unwrap();
    fs::write(dir.path().join("spec.iris"), b"# \xff").unwrap();
    assert!(
        Case::load(dir.path(), "0.1")
            .unwrap_err()
            .contains("spec.iris must be valid UTF-8")
    );
}

#[test]
fn case_loading_rejects_missing_ambiguous_and_unknown_configuration() {
    let changes = [
        ("spec.iris", None),
        ("input.bin", None),
        ("expected.json", None),
        ("input.txt", Some("")),
        ("expected.dzn", Some("")),
        ("case.toml", Some("unknown = true")),
        ("case.toml", Some("title = ''")),
        ("case.toml", Some("output = 'dzn'")),
        ("case.toml", Some("operation = 'dzn'\noutput = 'json'")),
    ];
    for (file, content) in changes {
        let dir = fixture_copy("passing/read");
        match content {
            Some(content) => fs::write(dir.path().join(file), content).unwrap(),
            None => fs::remove_file(dir.path().join(file)).unwrap(),
        }
        assert!(
            Case::load(dir.path(), "0.1").is_err(),
            "accepted change to {file}: {content:?}"
        );
    }
    let dir = fixture_copy("passing/dzn");
    fs::write(dir.path().join("case.toml"), "operation = 'read'").unwrap();
    assert!(Case::load(dir.path(), "0.1").is_err());
}

#[test]
fn parameters_are_strict_string_maps_with_exact_source_text() {
    for invalid in [
        "[]",
        "{\"n\":7}",
        "{\"n\":\"a\",\"n\":\"b\"}",
        "{} {}",
        "\u{feff}{}",
        "{\"n\":\"\\ud800\"}",
    ] {
        let dir = fixture_copy("passing/read");
        fs::write(dir.path().join("params.json"), invalid).unwrap();
        assert!(Case::load(dir.path(), "0.1").is_err(), "accepted {invalid}");
    }
    let case = Case::load(&fixtures("passing/read"), "0.1").unwrap();
    assert_eq!(
        case.params.unwrap(),
        fs::read_to_string(fixtures("passing/read/params.json")).unwrap()
    );
    let dir = fixture_copy("passing/parse");
    fs::write(dir.path().join("params.json"), "{\"n\":\"7\"}").unwrap();
    assert!(Case::load(dir.path(), "0.1").is_err());
}

#[test]
fn discovery_deduplicates_sorts_filters_and_rejects_zero_selection() {
    let root = fixtures("passing");
    let cases = discover(&[root.clone(), root.join("read")], None).unwrap();
    assert_eq!(cases.len(), 6);
    assert!(cases.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        discover(std::slice::from_ref(&root), Some("catalog"))
            .unwrap()
            .len(),
        2
    );
    assert!(discover(std::slice::from_ref(&root), Some("CATALOG")).is_err());
    assert!(discover(&[root], Some("*")).is_err());
    assert!(discover(&[fixtures("nonexistent")], None).is_err());
    let metadata_only = TempDir::new().unwrap();
    fs::write(metadata_only.path().join("case.toml"), "").unwrap();
    assert_eq!(
        discover(&[metadata_only.path().into()], None)
            .unwrap()
            .len(),
        1
    );
    assert!(Case::load(metadata_only.path(), "0.1").is_err());
}

#[cfg(unix)]
#[test]
fn discovery_skips_directory_symlinks_and_accepts_file_symlinks() {
    use std::os::unix::fs::symlink;
    let dir = TempDir::new().unwrap();
    symlink(fixtures("passing"), dir.path().join("linked-directory")).unwrap();
    assert!(discover(&[dir.path().into()], None).is_err());
    let case = fixture_copy("passing/read");
    fs::remove_file(case.path().join("spec.iris")).unwrap();
    symlink(
        fixtures("passing/read/spec.iris"),
        case.path().join("spec.iris"),
    )
    .unwrap();
    assert!(Case::load(case.path(), "0.1").is_ok());
}

#[test]
fn known_failure_configuration_requires_precise_different_compatible_alternates() {
    for replacement in [
        "driver = 'canned'\noutcome = 'data'\nexpected_file = 'expected.json'\nreason = 'noop'",
        "driver = 'canned'\noutcome = 'parse'\nexpected_file = 'expected.json'\nreason = 'wrong kind'",
        "driver = 'canned'\noutcome = 'data'\nexpected_file = '../outside'\nreason = 'outside'",
        "driver = 'canned'\noutcome = 'data'\nexpected_file = '/absolute'\nreason = 'outside'",
        "driver = 'canned'\noutcome = 'data'\nexpected_file = 'known/canned/alternate.json'\nreason = ''",
    ] {
        let dir = fixture_copy("failing/xpass");
        fs::write(
            dir.path().join("case.toml"),
            format!("[[known_failure]]\n{replacement}\n"),
        )
        .unwrap();
        assert!(
            Case::load(dir.path(), "0.1").is_err(),
            "accepted {replacement}"
        );
    }
    let dir = fixture_copy("failing/xpass");
    let metadata = fs::read_to_string(dir.path().join("case.toml")).unwrap();
    fs::write(dir.path().join("case.toml"), metadata.repeat(2)).unwrap();
    assert!(Case::load(dir.path(), "0.1").is_err());
    for difference in [
        "",
        "spec/0.1/appendix-c-differences.md",
        "spec/0.1/appendix-c-differences.md#c4-",
        "other.md#c4-thing",
    ] {
        fs::write(
            dir.path().join("case.toml"),
            format!(
                "{}\ndifference = {difference:?}\n",
                metadata.replace("\"canned\"", "\"gecode-test\"")
            ),
        )
        .unwrap();
        assert!(Case::load(dir.path(), "0.1").is_err());
    }
    fs::write(
        dir.path().join("case.toml"),
        metadata.replace("\"canned\"", "\"gecode-test\""),
    )
    .unwrap();
    assert!(Case::load(dir.path(), "0.1").is_err());
    fs::write(
        dir.path().join("case.toml"),
        format!(
            "{}\ndifference = 'spec/0.1/appendix-c-differences.md#c4-dzn-floats-just-below-1e17'\n",
            metadata.replace("\"canned\"", "\"gecode-test\"")
        ),
    )
    .unwrap();
    assert!(Case::load(dir.path(), "0.1").is_ok());
}

#[test]
fn canned_driver_observes_all_commands_original_bytes_and_parameters() {
    let report = run(&options(fixtures("passing"))).unwrap();
    assert_eq!(report.exit_code(), 0);
    assert!(
        report
            .cases
            .iter()
            .all(|case| case.status == CaseStatus::Pass),
        "{}",
        report.render()
    );
    assert!(
        report
            .render()
            .ends_with("Summary: total=6 pass=6 xfail=0 fail=0 xpass=0 unrepresentable=0\n")
    );
}

#[test]
fn known_failures_apply_only_to_selected_driver_and_complete_outcome() {
    let mut opts = options(fixtures("known"));
    assert_eq!(run(&opts).unwrap().exit_code(), 1);
    opts.driver_id = Some("other".into());
    assert_eq!(run(&opts).unwrap().exit_code(), 1);
    opts.driver_id = Some("canned".into());
    let report = run(&opts).unwrap();
    assert_eq!(report.exit_code(), 0);
    assert!(
        report
            .cases
            .iter()
            .all(|case| case.status == CaseStatus::KnownFailure)
    );
    assert!(
        report
            .render()
            .contains("Summary: total=2 pass=0 xfail=2 fail=0 xpass=0 unrepresentable=0")
    );
    opts.roots = vec![fixtures("failing")];
    let report = run(&opts).unwrap();
    assert_eq!(report.exit_code(), 1);
    assert_eq!(
        report
            .cases
            .iter()
            .filter(|case| case.status == CaseStatus::Fail)
            .count(),
        4
    );
    assert_eq!(
        report
            .cases
            .iter()
            .filter(|case| case.status == CaseStatus::UnexpectedPass)
            .count(),
        1
    );
    assert!(
        report
            .render()
            .contains("alternate for canned did not match")
    );
    assert!(report.render().contains("record_path"));
}

const UNREPRESENTABLE_ANNOTATION: &str = "\
[[known_failure]]
driver = 'gecode-test'
outcome = 'unrepresentable'
reason = 'The reference returns a non-UTF-8 string.'
difference = 'spec/0.1/appendix-c-differences.md#c1-unicode-strings'
";

fn unrepresentable_case() -> TempDir {
    let dir = fixture_copy("passing/read");
    fs::write(dir.path().join("case.toml"), UNREPRESENTABLE_ANNOTATION).unwrap();
    fs::write(dir.path().join("actual.bin"), b"").unwrap();
    change_control(dir.path(), |control| {
        control["output_file"] = json!("actual.bin");
        control["exit"] = json!(3);
        control["stderr"] =
            json!("iris-gecode-driver: cannot serialize non-UTF-8 reference string\n");
    });
    dir
}

#[test]
fn unrepresentable_requires_gecode_c1_and_no_alternate_file() {
    for annotation in [
        UNREPRESENTABLE_ANNOTATION.replace("gecode-test", "other"),
        UNREPRESENTABLE_ANNOTATION.replace("c1-unicode-strings", "c2-reader-spec-source"),
        UNREPRESENTABLE_ANNOTATION.replace("c1-unicode-strings", "c1-other"),
        UNREPRESENTABLE_ANNOTATION.replace("spec/0.1/appendix-c-differences.md#", "other.md#"),
        UNREPRESENTABLE_ANNOTATION.replace(
            "difference = 'spec/0.1/appendix-c-differences.md#c1-unicode-strings'\n",
            "",
        ),
        format!("{UNREPRESENTABLE_ANNOTATION}expected_file = 'expected.json'\n"),
    ] {
        let dir = unrepresentable_case();
        fs::write(dir.path().join("case.toml"), &annotation).unwrap();
        assert!(
            Case::load(dir.path(), "0.1").is_err(),
            "accepted {annotation}"
        );
    }
}

#[test]
fn unrepresentable_is_reported_separately_and_only_for_the_selected_driver() {
    let dir = unrepresentable_case();
    let mut opts = options(dir.path().into());
    assert_eq!(run(&opts).unwrap().cases[0].status, CaseStatus::Fail);
    opts.driver_id = Some("gecode-test".into());
    let report = run(&opts).unwrap();
    assert_eq!(report.exit_code(), 0);
    assert_eq!(report.cases[0].status, CaseStatus::Unrepresentable);
    assert!(report.render().starts_with("UNREPRESENTABLE "));
    assert!(report.render().contains("c1-unicode-strings"));
    assert!(
        report
            .render()
            .ends_with("Summary: total=1 pass=0 xfail=0 fail=0 xpass=0 unrepresentable=1\n")
    );
}

#[test]
fn unrepresentable_cannot_mask_other_driver_failures_or_outcomes() {
    let diagnostic = "iris-gecode-driver: cannot serialize non-UTF-8 reference string\n";
    let mut unmapped = error();
    unmapped["code"] = json!("E-UNMAPPED");
    for (exit, stdout, stderr) in [
        (3, "", "different host failure\n"),
        (3, "", "cannot serialize non-UTF-8 reference string\n"),
        (3, "", diagnostic.trim_end()),
        (3, "", &format!("{diagnostic}extra diagnostic\n")),
        (3, "partial output", diagnostic),
        (2, "", diagnostic),
        (7, "", diagnostic),
        (0, "not json", diagnostic),
        (0, "{\"iris\":\"0.1\",\"entries\":[]}", diagnostic),
        (1, &unmapped.to_string(), diagnostic),
    ] {
        let dir = unrepresentable_case();
        fs::write(dir.path().join("actual.bin"), stdout).unwrap();
        change_control(dir.path(), |control| {
            control["exit"] = json!(exit);
            control["stderr"] = json!(stderr);
        });
        let mut opts = options(dir.path().into());
        opts.driver_id = Some("gecode-test".into());
        let report = run(&opts).unwrap();
        assert_eq!(
            report.cases[0].status,
            CaseStatus::Fail,
            "{}",
            report.render()
        );
        assert_eq!(report.exit_code(), 1);
    }
}

#[test]
fn unrepresentable_primary_match_is_a_stale_annotation() {
    let dir = unrepresentable_case();
    change_control(dir.path(), |control| {
        control["output_file"] = json!("expected.json");
        control["exit"] = json!(0);
        control["stderr"] = json!("");
    });
    let mut opts = options(dir.path().into());
    opts.driver_id = Some("gecode-test".into());
    let report = run(&opts).unwrap();
    assert_eq!(report.cases[0].status, CaseStatus::UnexpectedPass);
    assert_eq!(report.exit_code(), 1);
    assert!(
        report
            .render()
            .contains("primary matched; stale known failure")
    );
}

#[test]
fn hard_driver_failures_cannot_be_known_failures() {
    for (status, bytes, description) in [
        (0, b"not json".as_slice(), "invalid"),
        (0, b"{} {}", "invalid"),
        (1, b"{\"iris\":\"0.1\",\"entries\":[]}", "invalid"),
        (2, b"", "driver misuse"),
        (3, b"", "host/adapter"),
        (7, b"", "unsupported driver exit"),
    ] {
        let dir = fixture_copy("failing/xpass");
        fs::write(dir.path().join("actual.bin"), bytes).unwrap();
        change_control(dir.path(), |control| {
            control["output_file"] = json!("actual.bin");
            control["exit"] = json!(status);
        });
        let mut opts = options(dir.path().into());
        opts.driver_id = Some("canned".into());
        let report = run(&opts).unwrap();
        assert_eq!(report.cases[0].status, CaseStatus::Fail);
        assert!(report.render().contains(description), "{}", report.render());
    }
    let dir = fixture_copy("failing/xpass");
    let mut document = error();
    document["code"] = json!("E-UNMAPPED");
    fs::write(dir.path().join("actual.bin"), document.to_string()).unwrap();
    change_control(dir.path(), |control| {
        control["output_file"] = json!("actual.bin");
        control["exit"] = json!(1);
    });
    let mut opts = options(dir.path().into());
    opts.driver_id = Some("canned".into());
    assert!(
        run(&opts)
            .unwrap()
            .render()
            .contains("E-UNMAPPED is a hard adapter failure")
    );
    opts.driver = "/a/nonexistent/driver".into();
    assert!(
        run(&opts)
            .unwrap()
            .render()
            .contains("could not spawn driver")
    );
}

#[test]
fn process_capture_drains_stderr_and_enforces_direct_child_deadline() {
    let dir = fixture_copy("failing/xpass");
    let pid = dir.path().join("pid");
    change_control(dir.path(), |control| {
        control["delay"] = json!(30);
        control["pid_file"] = json!(pid);
        control["stderr"] = json!("diagnostic".repeat(20_000));
    });
    let mut opts = options(dir.path().into());
    opts.driver_id = Some("canned".into());
    // The deadline leaves time for Python to start and write its stderr.
    opts.timeout = Duration::from_secs(2);
    let started = Instant::now();
    let report = run(&opts).unwrap();
    assert_eq!(report.cases[0].status, CaseStatus::Fail);
    assert!(report.render().contains("timeout") && report.render().contains("diagnostic"));
    assert!(started.elapsed() < Duration::from_secs(15));
    #[cfg(unix)]
    {
        let pid = fs::read_to_string(pid).unwrap();
        assert!(
            !Command::new("kill")
                .arg("-0")
                .arg(pid)
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    }
}

#[cfg(unix)]
#[test]
fn signal_termination_is_a_hard_failure() {
    let dir = fixture_copy("failing/xpass");
    change_control(dir.path(), |control| {
        control["signal"] = json!(true);
    });
    let mut opts = options(dir.path().into());
    opts.driver_id = Some("canned".into());
    assert!(
        run(&opts)
            .unwrap()
            .render()
            .contains("terminated by signal")
    );
}

#[test]
fn command_prefix_and_paths_with_spaces_are_separate_arguments() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("case with spaces");
    copy_directory(&fixtures("passing/read"), &path);
    let script = dir.path().join("driver with spaces.py");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/canned-driver.py"),
        &script,
    )
    .unwrap();
    change_control(&path, |control| {
        control["prefix"] = json!("a b;$literal");
    });
    let mut opts = options(path);
    opts.driver = format!(
        "python3 {} --prefix {}",
        shell_words::quote(&script.to_string_lossy()),
        shell_words::quote("a b;$literal")
    );
    assert_eq!(run(&opts).unwrap().exit_code(), 0);
}

#[test]
fn configuration_is_loaded_before_any_driver_execution() {
    let dir = TempDir::new().unwrap();
    let first = dir.path().join("a");
    copy_directory(&fixtures("passing/read"), &first);
    let pid = dir.path().join("pid");
    change_control(&first, |control| {
        control["pid_file"] = json!(pid);
    });
    fs::create_dir(dir.path().join("z")).unwrap();
    fs::write(dir.path().join("z/case.toml"), "").unwrap();
    assert!(run(&options(dir.path().into())).is_err());
    assert!(!pid.exists());
}

#[test]
fn cli_exit_codes_summary_and_configuration_diagnostics() {
    let binary = env!("CARGO_BIN_EXE_iris-conformance");
    for (group, expected, summary) in [
        (
            "passing",
            0,
            "Summary: total=6 pass=6 xfail=0 fail=0 xpass=0 unrepresentable=0",
        ),
        (
            "known",
            0,
            "Summary: total=2 pass=0 xfail=2 fail=0 xpass=0 unrepresentable=0",
        ),
        (
            "failing",
            1,
            "Summary: total=5 pass=0 xfail=0 fail=4 xpass=1 unrepresentable=0",
        ),
    ] {
        let output = Command::new(binary)
            .args(["run", "--driver", &driver(), "--driver-id", "canned"])
            .arg(fixtures(group))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(expected));
        assert!(String::from_utf8(output.stdout).unwrap().contains(summary));
    }
    for args in [
        vec!["--filter", "NO-MATCH"],
        vec!["--version", "1.0"],
        vec!["--timeout", "0"],
        vec!["--timeout", "NaN"],
    ] {
        let output = Command::new(binary)
            .args(["run", "--driver", &driver()])
            .args(args)
            .arg(fixtures("passing"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8(output.stderr).unwrap().contains("error"));
    }
    for flag in ["--help", "--version"] {
        assert!(
            Command::new(binary)
                .arg(flag)
                .stdout(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    }
}

#[test]
fn operation_errors_and_empty_dzn_use_their_protocol_outcomes() {
    for (operation, output) in [
        ("parse", "json"),
        ("read", "json"),
        ("catalog", "json"),
        ("catalog", "dzn"),
        ("dzn", "dzn"),
    ] {
        let dir = fixture_copy("passing/error");
        fs::write(
            dir.path().join("case.toml"),
            format!("operation = '{operation}'\noutput = '{output}'\n"),
        )
        .unwrap();
        change_control(dir.path(), |control| {
            control["operation"] = json!(operation);
            if operation == "catalog" {
                control["format"] = json!(output);
            }
            if operation == "parse" {
                control.as_object_mut().unwrap().remove("input_hex");
            }
        });
        let report = run(&options(dir.path().into())).unwrap();
        assert_eq!(report.exit_code(), 0, "{}", report.render());
    }
    let dir = fixture_copy("passing/dzn");
    fs::write(dir.path().join("expected.dzn"), []).unwrap();
    assert_eq!(run(&options(dir.path().into())).unwrap().exit_code(), 0);
}

#[test]
fn deadline_includes_pipe_completion_after_direct_child_exit() {
    let dir = fixture_copy("passing/read");
    change_control(dir.path(), |control| {
        control["hold_pipes"] = json!(30);
    });
    let mut opts = options(dir.path().into());
    // The deadline leaves time for Python to start and exit before it.
    opts.timeout = Duration::from_secs(2);
    let started = Instant::now();
    let report = run(&opts).unwrap();
    assert_eq!(report.cases[0].status, CaseStatus::Fail);
    assert!(report.render().contains("timeout"));
    assert!(started.elapsed() < Duration::from_secs(15));
}
