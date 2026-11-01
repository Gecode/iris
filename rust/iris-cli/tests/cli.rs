use iris_conformance::canonical::{Outcome, OutcomeKind};
use std::process::{Command, Output};
use tempfile::TempDir;
fn fixture(bytes: &[u8]) -> (TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("spec.iris");
    std::fs::write(&path, bytes).unwrap();
    (dir, path.to_str().unwrap().into())
}
fn driver(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iris-driver"))
        .args(args)
        .output()
        .unwrap()
}
fn check(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iris"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn check_exit_and_caret() {
    let (_dir, path) = fixture(b"format test x:int");
    let output = check(&["check", &path]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{path}: ok (format test)\n")
    );
    assert!(output.stderr.is_empty());
    std::fs::write(&path, b"x:int\nlet y=@").unwrap();
    let output = check(&["check", &path]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with(&format!("{path}:2: unexpected character '@'\n")));
    assert!(stderr.contains("2 | let y=@\n  |       ^"));
    for args in [vec![], vec!["check"], vec!["unknown"]] {
        let output = check(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let output = check(&["check", &format!("{path}.missing")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}
#[test]
fn driver_canonical_documents() {
    let (_dir, path) = fixture(b"param n=1/0");
    let output = driver(&["parse", &path, "--version", "0.1", "--params", "{}"]);
    assert_eq!(output.status.code(), Some(0));
    Outcome::decode(OutcomeKind::Parse, &output.stdout, "0.1").unwrap();
    for (bytes, code, line, column) in [
        (b"n:int\nlet x=(1).bad".as_slice(), "E-SPEC-FIELD", 2, None),
        (b"# \xc3\xa9\n# \xff".as_slice(), "E-SPEC-UTF8", 2, Some(3)),
    ] {
        std::fs::write(&path, bytes).unwrap();
        let output = driver(&["parse", &path]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        Outcome::decode(OutcomeKind::Error, &output.stdout, "0.1").unwrap();
        let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["code"], code);
        assert_eq!(error["spec_location"]["line"], line);
        assert_eq!(error["spec_location"]["column"], serde_json::json!(column));
        for key in ["input_location", "context", "record_path"] {
            assert_eq!(error[key], serde_json::Value::Null);
        }
        assert!(!error["message"].as_str().unwrap().contains(&path));
    }
}
#[test]
fn driver_misuse_and_host_failures() {
    let (_dir, path) = fixture(b"x:int");
    for options in [
        vec!["--version", "1.0"],
        vec!["--version", "0.1", "--version", "0.1"],
        vec!["--params", "{}", "--params", "{}"],
        vec!["--unknown"],
        vec!["extra"],
        vec!["--params"],
        vec!["--params", "{\"x\":\"a\"}"],
        vec!["--params", "{\"x\":\"a\",\"x\":\"b\"}"],
        vec!["--params", "{\"x\":1}"],
        vec!["--params", "\u{feff}{}"],
        vec!["--params", "{} {}"],
        vec!["--params", "{\"x\":\"\\ud800\"}"],
    ] {
        let mut args = vec!["parse", &path];
        args.extend(options);
        let output = driver(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    for operation in ["read", "catalog", "dzn"] {
        let output = driver(&[operation, &path, &path]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let output = driver(&["parse", &format!("{path}.missing")]);
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}
#[test]
fn excerpt_escapes_controls_and_uses_physical_pattern_line() {
    let (_dir, path) = fixture(b"\tline \"{x:\\nunknown}\"");
    let output = check(&["check", &path]);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with(&format!("{path}:2: unknown type 'unknown'")));
    assert!(stderr.contains("1 |         line"));
    assert!(!stderr.contains('\t'));
    let error = match iris_cli::check_spec(b"let x=\"a\r\0b\" @", "test").unwrap_err() {
        iris_cli::CheckFailure::Language(error) => error,
        _ => unreachable!(),
    };
    let rendered = iris_cli::render_error(&error, b"let x=\"a\r\0b\" @");
    assert!(rendered.contains("a\\r\\u{0}b"));
    assert!(!rendered.contains('\r'));
    assert!(!rendered.contains('\0'));
    let error = match iris_cli::check_spec(b"\0", "test").unwrap_err() {
        iris_cli::CheckFailure::Language(error) => error,
        _ => unreachable!(),
    };
    assert!(!iris_cli::render_error(&error, b"\0").contains('\0'));
}
