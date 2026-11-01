use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use iris_conformance::case::{Case, Operation, OutputFormat, discover};
use iris_conformance::examples::extract;
use tempfile::TempDir;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/examples")
}

#[test]
fn fixture_extracts_exact_bytes_and_replaces_the_owned_tree() {
    let temp = TempDir::new().unwrap();
    let output = temp.path().join("generated");
    assert_eq!(
        extract(&fixture(), &output, false).unwrap(),
        "Generated 6 cases (1 with spec.bin)"
    );
    let case = |id: &str| output.join(format!("02-fixture--fixture-{id}"));
    let bytes = |id: &str, name: &str| fs::read(case(id).join(name)).unwrap();
    assert_eq!(bytes("literal", "spec.iris"), b"x: word # literal \\n\n");
    assert_eq!(bytes("literal", "input.bin"), "λ\n".as_bytes());
    assert_eq!(
        bytes("literal", "params.json"),
        b"{\"n\":\"0007\",\"label\":\"a,b\"}\n"
    );
    assert_eq!(bytes("binary", "spec.bin"), b"#\xff\r\n\0");
    assert!(!case("binary").join("spec.iris").exists());
    assert_eq!(bytes("hex-text", "spec.iris"), b"#```\n");
    assert_eq!(bytes("hex-text", "expected.dzn"), b"x = 7;\r\n\0\xff");
    assert_eq!(bytes("catalog", "expected.dzn"), b"instances = 1;");
    assert_eq!(bytes("error", "spec.iris"), b"@");
    assert!(bytes("empty", "spec.iris").is_empty());
    assert!(bytes("empty", "input.bin").is_empty());
    let catalog = Case::load(&case("catalog"), "0.1").unwrap();
    assert_eq!(catalog.operation, Operation::Catalog);
    assert_eq!(catalog.output, OutputFormat::Dzn);
    let paths = discover(std::slice::from_ref(&output), None).unwrap();
    assert_eq!(paths.len(), 6);
    for path in paths {
        Case::load(&path, "0.1").unwrap();
    }
    extract(&fixture(), &output, true).unwrap();
    extract(&fixture(), &output, false).unwrap();
    extract(&fixture(), &output, true).unwrap();

    fs::write(case("literal").join("input.bin"), b"changed").unwrap();
    fs::create_dir_all(output.join("stale/empty")).unwrap();
    fs::write(output.join("stale/old.bin"), b"stale").unwrap();
    let error = extract(&fixture(), &output, true).unwrap_err();
    assert!(error.contains("changed 02-fixture--fixture-literal/input.bin"));
    assert!(error.contains("stale stale/old.bin"));
    assert!(error.contains("stale directory stale/empty"));
    assert_eq!(bytes("literal", "input.bin"), b"changed");
    extract(&fixture(), &output, false).unwrap();
    assert!(!output.join("stale").exists());
    fs::remove_file(case("literal").join("input.bin")).unwrap();
    assert!(
        extract(&fixture(), &output, true)
            .unwrap_err()
            .contains("missing")
    );
}

#[test]
fn malformed_examples_fail_before_changing_the_destination() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let output = temp.path().join("output");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep"), b"unchanged").unwrap();
    let valid = "```iris example\nid: fixture-valid\n--- spec\nx: int\n--- input\n7\n--- expected data\n{\"iris\":\"0.1\",\"entries\":[]}\n```\n";
    let invalid = [
        valid.replace("id: fixture-valid", "id: invalid"),
        valid.replace("--- spec", "id: fixture-repeated\n--- spec"),
        valid.replace("--- spec", "unknown: value\n--- spec"),
        valid.replace("--- input", "--- spec"),
        valid.replace("--- input", "--- unknown"),
        valid
            .replace("--- input", "--- input hex")
            .replace("\n7\n", "\nFF\n"),
        valid.replace("--- input", "--- input hex"),
        valid
            .replace("--- input", "--- input hex")
            .replace("\n7\n", "\n0b\u{a0}\n"),
        valid.replace("--- expected data", "--- expected data hex"),
        valid.replace("--- input\n7\n", ""),
        valid.replace("id: fixture-valid", "id: fixture-valid\noperation: parse"),
        valid.replace("--- expected data", "--- expected parse"),
        valid.replace(
            "--- expected data",
            "--- params\n{\"n\":\"1\",\"n\":\"2\"}\n--- expected data",
        ),
        valid.replace(
            "--- expected data",
            "--- params\n{\"n\":\"\\ud800\"}\n--- expected data",
        ),
        valid.replace("\"entries\":[]", "\"entries\":[],\"iris\":\"0.1\""),
        valid.replace("\"entries\":[]", "\"entries\":NaN"),
        valid.replace("x: int\n", "x: int\r\n"),
        valid.replacen("```iris example", "```iris example ", 1),
        valid.replacen("```iris example", " ```iris example", 1),
        valid.replacen("```iris example", "\t```iris example", 1),
        valid.replace("x: int", "# ``` in content"),
        valid.trim_end_matches("```\n").into(),
        format!("{valid}{valid}"),
    ];
    for text in invalid {
        fs::write(source.join("02-test.md"), &text).unwrap();
        assert!(extract(&source, &output, false).is_err(), "accepted {text}");
        assert_eq!(fs::read(output.join("keep")).unwrap(), b"unchanged");
    }
}

#[test]
fn cli_check_returns_nonzero_without_writing() {
    let temp = TempDir::new().unwrap();
    let output = temp.path().join("generated");
    let invoke = |check: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_iris-conformance"));
        command.arg("extract-examples").arg(fixture()).arg(&output);
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    };
    assert!(!invoke(true).status.success());
    assert!(!output.exists());
    assert!(invoke(false).status.success());
    assert!(invoke(true).status.success());
    fs::write(output.join("extra"), b"stale").unwrap();
    assert!(!invoke(true).status.success());
    assert!(output.join("extra").exists());
}
