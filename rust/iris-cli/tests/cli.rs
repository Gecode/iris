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
    for operation in ["catalog", "dzn"] {
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

#[test]
fn read_json_and_instance_caret() {
    let (dir, path) = fixture(b"x:int let y=x^2");
    let input = dir.path().join("input.bin");
    let input = input.to_str().unwrap();
    std::fs::write(input, b"7\n").unwrap();
    for output in [
        driver(&["read", &path, input, "--params", "{}", "--version", "0.1"]),
        check(&["read", &path, input, "--json"]),
    ] {
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        Outcome::decode(OutcomeKind::Data, &output.stdout, "0.1").unwrap();
        let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(document["entries"][0]["value"], 7);
        assert_eq!(document["entries"][1]["value"], 49);
    }
    std::fs::write(input, b"oops\n").unwrap();
    let output = check(&["read", &path, input, "--json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        format!(
            "{input}:1:1: expected int, found 'oops'\n     1 | oops\n       | ^\nwhile reading: x\nspec: {path}:1\n"
        )
    );
    let output = driver(&["read", &path, input]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    Outcome::decode(OutcomeKind::Error, &output.stdout, "0.1").unwrap();
    for args in [
        vec!["read", &path],
        vec!["read", &path, input],
        vec!["read", &path, input, "--dzn"],
        vec!["read", &path, input, input, "--json"],
    ] {
        assert_eq!(check(&args).status.code(), Some(2));
    }
    assert_eq!(
        driver(&["read", &path, &format!("{input}.missing")])
            .status
            .code(),
        Some(3)
    );
    for options in [
        vec!["--params", "{\"x\":1}"],
        vec!["--params", "{\"x\":\"a\",\"x\":\"b\"}"],
        vec!["--params", "{} {}"],
        vec!["--version", "1.0"],
        vec!["--params", "{}", "--params", "{}"],
    ] {
        let mut args = vec!["read", &path, input];
        args.extend(options);
        let output = driver(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    for (source, bytes, code, column, spec_line, context, span_end) in [
        (
            "b:int\na[2]:int offset\n1",
            "7 9223372036854775807 0",
            "E-INTEGER-OVERFLOW",
            24usize,
            2,
            Some("a"),
            23,
        ),
        (
            "b:int\nlist a:int until eol offset\n1.0",
            "7 1 2  \n3",
            "E-OFFSET-TYPE",
            8usize,
            2,
            Some("a"),
            7,
        ),
        (
            "repeat 2 {\nk:int\nif k {a:int}\n}",
            "1 7 0",
            "E-REPEAT-MISSING",
            6,
            3,
            Some("repeat [2 of 2]"),
            5,
        ),
        (
            "repeat 1 {\nk:int\nif k {a:int}\n}",
            "0",
            "E-REPEAT-MISSING",
            2,
            1,
            None,
            1,
        ),
        (
            "repeat 1 {\nrepeat 2 {\nindex 1\n}\n}",
            "1 3",
            "E-INDEX-MISMATCH",
            3,
            3,
            Some("repeat [1 of 1] > repeat [2 of 2]"),
            3,
        ),
    ] {
        std::fs::write(&path, source).unwrap();
        std::fs::write(input, bytes).unwrap();
        let spec = iris_core::Spec::parse(source.as_bytes(), &path).unwrap();
        let iris_core::Failure::Language(error) = spec
            .read(bytes.as_bytes(), input, &iris_core::Params::new())
            .unwrap_err()
        else {
            panic!("expected a language error")
        };
        assert_eq!(error.code.as_str(), code);
        assert_eq!(error.context(), context);
        assert_eq!(error.spec_location().unwrap().line, Some(spec_line));
        let spec_span = error.spec_span().unwrap();
        let start = usize::from(spec_span.start());
        assert_eq!(
            source[..start].bytes().filter(|&b| b == b'\n').count() + 1,
            spec_line
        );
        assert_eq!(error.input_span().unwrap().bytes, column - 1..span_end);
        let output = check(&["read", &path, input, "--json"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let rendered = String::from_utf8(output.stderr).unwrap();
        assert!(
            rendered.starts_with(&format!("{input}:1:{column}:")),
            "{rendered}"
        );
        assert!(
            rendered.contains(&format!("       | {}^\n", " ".repeat(column - 1))),
            "{rendered}"
        );
        if let Some(context) = context {
            assert!(
                rendered.contains(&format!("while reading: {context}\n")),
                "{rendered}"
            );
        } else {
            assert!(!rendered.contains("while reading:"), "{rendered}");
        }
        assert!(
            rendered.ends_with(&format!("spec: {path}:{spec_line}\n")),
            "{rendered}"
        );
    }
}
#[test]
fn deeply_dimensional_arrays_serialize() {
    // §5.12 allows any number of dimensions; deep enough to overflow a
    // recursive writer.
    const DEPTH: usize = 16384;
    for (source, depth, value) in [
        (format!("a{}:int", "[1]".repeat(DEPTH)), DEPTH, "7"),
        (
            format!("repeat 1 {{a{}:int}}", "[1]".repeat(DEPTH)),
            DEPTH + 1,
            "7",
        ),
        (
            format!("repeat 1 {{a[0]{}:int}}", "[1]".repeat(DEPTH - 1)),
            DEPTH + 1,
            "",
        ),
    ] {
        let (dir, path) = fixture(source.as_bytes());
        let input = dir.path().join("input.bin");
        let input = input.to_str().unwrap();
        std::fs::write(input, value).unwrap();
        let expected_value = if value.is_empty() {
            "[[]]".into()
        } else {
            format!("{}7{}", "[".repeat(depth), "]".repeat(depth))
        };
        let expected = format!(
            "{{\"iris\":\"0.1\",\"entries\":[{{\"name\":\"a\",\"type\":\"int\",\"depth\":{depth},\"value\":{expected_value}}}]}}\n"
        );
        for output in [
            check(&["read", &path, input, "--json"]),
            driver(&["read", &path, input]),
        ] {
            assert_eq!(output.status.code(), Some(0), "{output:?}");
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        }
    }
}
#[test]
fn ordered_parameter_admission_precedes_input_validation() {
    let params = iris_cli::protocol::parse_params("{\"z\":\"  a=b\\n\",\"a\":\"2\"}").unwrap();
    assert_eq!(
        params.pairs(),
        &[("z".into(), "  a=b\n".into()), ("a".into(), "2".into())]
    );
    let (dir, path) = fixture(b"param n=1/0 x:int");
    let input = dir.path().join("input.bin");
    std::fs::write(&input, b"\xff").unwrap();
    let output = driver(&[
        "read",
        &path,
        input.to_str().unwrap(),
        "--params",
        "{\"z\":\"first\",\"a\":\"second\"}",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["code"], "E-PARAM-UNKNOWN");
    assert!(error["message"].as_str().unwrap().contains("'z'"));
    assert_eq!(error["input_location"], serde_json::Value::Null);
    let output = driver(&[
        "read",
        &path,
        input.to_str().unwrap(),
        "--params",
        "{\"n\":\"value\"}",
    ]);
    let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["code"], "E-UTF8");
}

#[test]
fn empty_branches_keep_the_enclosing_statement_line() {
    // Gecode's reported lines: an empty else block is not a statement.
    for (spec, input, code, line) in [
        (
            "repeat until eof {\nif 0 {}\nelse {}\n}",
            "x",
            "E-NO-PROGRESS",
            2,
        ),
        (
            "repeat 2 {\nk:int\nif k {a:int}\nelse {}\n}",
            "1 7 0",
            "E-REPEAT-MISSING",
            3,
        ),
    ] {
        let (dir, path) = fixture(spec.as_bytes());
        let input_path = dir.path().join("input.bin");
        std::fs::write(&input_path, input).unwrap();
        let output = driver(&["read", &path, input_path.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["code"], code, "{spec:?}");
        assert_eq!(error["spec_location"]["line"], line, "{spec:?}");
    }
}

#[test]
fn deeply_nested_repeats_respect_parser_admission() {
    // Execute at the existing 256-block limit, then reject thousands of
    // levels through the parser's established operational limit.
    for levels in [256, 4096] {
        let source = format!("{}x:int{}", "repeat 1 {".repeat(levels), "}".repeat(levels));
        let (dir, path) = fixture(source.as_bytes());
        let input = dir.path().join("input.bin");
        std::fs::write(&input, "7").unwrap();
        let input = input.to_str().unwrap();
        for output in [
            check(&["read", &path, input, "--json"]),
            driver(&["read", &path, input]),
        ] {
            if levels == 256 {
                assert_eq!(output.status.code(), Some(0), "{output:?}");
                let expected = format!(
                    "{{\"iris\":\"0.1\",\"entries\":[{{\"name\":\"x\",\"type\":\"int\",\"depth\":{levels},\"value\":{}7{}}}]}}\n",
                    "[".repeat(levels),
                    "]".repeat(levels)
                );
                assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
            } else {
                assert_eq!(output.status.code(), Some(1), "{output:?}");
                let diagnostic = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(
                    diagnostic.contains("blocks are nested too deeply"),
                    "{diagnostic}"
                );
            }
        }
    }
}
