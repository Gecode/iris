use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use gecode_iris_conformance::grammar::check_grammar;
use serde_json::json;
use tempfile::TempDir;

fn case(root: &Path, name: &str, source: &[u8], code: Option<&str>) -> std::path::PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    let extension = if std::str::from_utf8(source).is_ok() {
        "iris"
    } else {
        "bin"
    };
    fs::write(path.join(format!("spec.{extension}")), source).unwrap();
    fs::write(path.join("case.toml"), "operation = \"parse\"\n").unwrap();
    if let Some(code) = code {
        let error = json!({"code": code, "message": "fixture", "input_location": null,
            "spec_location": {"origin": "spec.iris", "line": 1, "column": null},
            "context": null, "record_path": null});
        fs::write(path.join("error.json"), error.to_string()).unwrap();
    } else {
        fs::write(
            path.join("expected.json"),
            r#"{"iris":"0.1","parsed":true}"#,
        )
        .unwrap();
    }
    path
}

fn command(paths: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iris-conformance"))
        .arg("check-grammar")
        .args(paths)
        .output()
        .unwrap()
}

#[test]
fn command_classifies_primary_outcomes_and_deduplicates_roots() {
    let dir = TempDir::new().unwrap();
    let valid = case(dir.path(), "valid", b"a:int", None);
    case(
        dir.path(),
        "semantic",
        b"let x=unknown()",
        Some("E-SPEC-FUNCTION"),
    );
    let invalid = case(dir.path(), "invalid", b"a:", Some("E-SPEC-SYNTAX"));
    case(dir.path(), "bytes", b"#\xff", Some("E-SPEC-UTF8"));
    // A driver-specific success alternate cannot change a primary negative.
    fs::write(
        invalid.join("alternate.json"),
        r#"{"iris":"0.1","parsed":true}"#,
    )
    .unwrap();
    fs::write(invalid.join("case.toml"), "operation = \"parse\"\n[[known_failure]]\ndriver = \"fixture\"\noutcome = \"parse\"\nexpected_file = \"alternate.json\"\nreason = \"fixture alternate\"\n").unwrap();
    let output = command(&[dir.path(), &valid]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Grammar summary: total=4 valid=2 invalid=2 accepted=2 rejected=2 fail=0\n"
    );
}

#[test]
fn disagreements_report_both_directions_and_do_not_stop_later_cases() {
    let dir = TempDir::new().unwrap();
    case(dir.path(), "unexpected-rejection", b"a:", None);
    case(
        dir.path(),
        "unexpected-acceptance",
        b"a:int",
        Some("E-SPEC-SYNTAX"),
    );
    case(dir.path(), "valid", b"", None);
    let output = command(&[dir.path()]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("unexpected-rejection: expected acceptance (success); tokens /"),
        "{stdout}"
    );
    assert!(
        stdout.contains("source line 1, byte 2: expected"),
        "{stdout}"
    );
    assert!(
        stdout.contains("unexpected-acceptance: expected rejection (E-SPEC-SYNTAX)"),
        "{stdout}"
    );
    assert!(
        stdout
            .ends_with("Grammar summary: total=3 valid=2 invalid=1 accepted=2 rejected=1 fail=2\n"),
        "{stdout}"
    );
}

#[test]
fn loading_io_zero_selection_and_cli_errors_exit_two() {
    let dir = TempDir::new().unwrap();
    let output = command(&[dir.path()]);
    assert_eq!(output.status.code(), Some(2));
    let path = case(dir.path(), "bad-metadata", b"a:int", None);
    fs::write(path.join("case.toml"), "unknown = true").unwrap();
    let output = command(&[dir.path()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unknown field")
    );
    assert_eq!(
        command(&[&dir.path().join("missing")]).status.code(),
        Some(2)
    );
    let output = Command::new(env!("CARGO_BIN_EXE_iris-conformance"))
        .args(["check-grammar", "--driver", "unused"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn repository_cases_and_formats_agree_with_the_formal_grammar() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let report = check_grammar(&[
        repository.join("conformance/cases"),
        repository.join("formats"),
    ])
    .unwrap();
    assert_eq!(report.exit_code(), 0, "{}", report.render());
    assert!(report.valid > 0 && report.invalid > 0);
    assert_eq!(report.valid, report.accepted);
    assert_eq!(report.invalid, report.rejected);
}
