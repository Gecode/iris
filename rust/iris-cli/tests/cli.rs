use gecode_iris_conformance::canonical::{Outcome, OutcomeKind};
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
fn dzn_cli_matches_driver_for_every_dzn_case() {
    use gecode_iris_conformance::case::{self, Case, Operation, OutputFormat};
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let paths = case::discover(
        &[repo.join("conformance/cases/0.1"), repo.join("formats")],
        None,
    )
    .unwrap();
    let mut count = 0;
    for path in paths {
        let case = Case::load(&path, "0.1").unwrap();
        if case.output != OutputFormat::Dzn {
            continue;
        }
        count += 1;
        let spec = case.spec.to_str().unwrap();
        let input = case.input.as_ref().unwrap().to_str().unwrap();
        let catalog = case.operation == Operation::Catalog;
        let mut driver_args = vec![if catalog { "catalog" } else { "dzn" }, spec, input];
        if catalog {
            driver_args.extend(["--format", "dzn"]);
        }
        let mut cli_args = vec![if catalog { "list" } else { "read" }, spec, input, "--dzn"];
        let mut assignments = vec![];
        if let Some(params) = &case.params {
            driver_args.extend(["--params", params]);
            let values: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(params).unwrap();
            assignments.extend(
                values
                    .iter()
                    .map(|(key, value)| format!("{key}={}", value.as_str().unwrap())),
            );
        }
        for assignment in &assignments {
            cli_args.extend(["--param", assignment]);
        }
        let expected = driver(&driver_args);
        let actual = check(&cli_args);
        assert_eq!(
            actual.status.code(),
            expected.status.code(),
            "{}",
            path.display()
        );
        if expected.status.success() {
            assert_eq!(actual.stdout, expected.stdout, "{}", path.display());
            assert!(actual.stderr.is_empty());
            // Multiple input operands concatenate the exact documents.
            cli_args.insert(3, input);
            let twice = check(&cli_args);
            assert!(twice.status.success());
            assert_eq!(twice.stdout, expected.stdout.repeat(2));
        } else {
            assert_eq!(expected.status.code(), Some(1));
            assert!(actual.stdout.is_empty());
        }
    }
    assert!(count >= 21);
    println!("CLI/driver dzn equality: {count} cases (including read errors and catalog)");
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
    let output = driver(&["dzn", &path, &path, "--format", "dzn"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
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
    let error = match gecode_iris_cli::check_spec(b"let x=\"a\r\0b\" @", "test").unwrap_err() {
        gecode_iris_cli::CheckFailure::Language(error) => error,
        _ => unreachable!(),
    };
    let rendered = gecode_iris_cli::render_error(&error, b"let x=\"a\r\0b\" @");
    assert!(rendered.contains("a\\r\\u{0}b"));
    assert!(!rendered.contains('\r'));
    assert!(!rendered.contains('\0'));
    let error = match gecode_iris_cli::check_spec(b"\0", "test").unwrap_err() {
        gecode_iris_cli::CheckFailure::Language(error) => error,
        _ => unreachable!(),
    };
    assert!(!gecode_iris_cli::render_error(&error, b"\0").contains('\0'));
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
        vec!["read", &path, input, "--json", "--dzn"],
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
    for (source, bytes, code, input_line, column, spec_line, context, span_end) in [
        (
            "b:int\na[2]:int offset\n1",
            "7 9223372036854775807 0",
            "E-INTEGER-OVERFLOW",
            1,
            24usize,
            2,
            Some("a"),
            23,
        ),
        (
            "b:int\nlist a:int until eol offset\n1.0",
            "7 1 2  \n3",
            "E-OFFSET-TYPE",
            1,
            8usize,
            2,
            Some("a"),
            7,
        ),
        (
            "repeat 2 {\nk:int\nif k {a:int}\n}",
            "1 7 0",
            "E-REPEAT-MISSING",
            1,
            6,
            3,
            Some("repeat [2 of 2]"),
            5,
        ),
        (
            "repeat 1 {\nk:int\nif k {a:int}\n}",
            "0",
            "E-REPEAT-MISSING",
            1,
            2,
            1,
            None,
            1,
        ),
        (
            "repeat 1 {\nrepeat 2 {\nindex 1\n}\n}",
            "1 3",
            "E-INDEX-MISMATCH",
            1,
            3,
            3,
            Some("repeat [1 of 1] > repeat [2 of 2]"),
            3,
        ),
        (
            "line \"A {x:0..1}\"",
            "A 123\n",
            "E-EXPECTED-VALUE",
            1,
            3,
            1,
            None,
            5,
        ),
        (
            "sections {\n\"A {x:0..1}\" optional {expect \"OK\"}\n}",
            "A 1\nOK\nA 9",
            "E-SECTIONS-REPEATED",
            3,
            1,
            1,
            None,
            7,
        ),
        (
            "sections {\n\"NEED\"\n\"END\" last {\nx:int\n}\n}",
            "END\n7\n",
            "E-LINE-MATCH",
            1,
            1,
            4,
            None,
            0,
        ),
        (
            "sections {\n\"A\" last {\nrepeat 1 {\nsections {\"B\" last {\nx:int\n}}}}}",
            "A\nB\nbad",
            "E-EXPECTED-VALUE",
            3,
            1,
            5,
            Some("section \"A\" > repeat [1 of 1] > section \"B\" > x"),
            7,
        ),
    ] {
        std::fs::write(&path, source).unwrap();
        std::fs::write(input, bytes).unwrap();
        let spec = gecode_iris_core::Spec::parse(source.as_bytes(), &path).unwrap();
        let gecode_iris_core::Failure::Language(error) = spec
            .read(bytes.as_bytes(), input, &gecode_iris_core::Params::new())
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
        let line_start = bytes
            .split_inclusive('\n')
            .take(input_line - 1)
            .map(str::len)
            .sum::<usize>();
        assert_eq!(
            error.input_span().unwrap().bytes,
            line_start + column - 1..span_end
        );
        let output = driver(&["read", &path, input]);
        assert_eq!(output.status.code(), Some(1));
        let diagnostic: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(diagnostic["code"], code);
        assert_eq!(diagnostic["input_location"]["line"], input_line);
        assert_eq!(diagnostic["input_location"]["column"], column);
        assert_eq!(diagnostic["spec_location"]["line"], spec_line);
        assert_eq!(diagnostic["context"], serde_json::json!(context));
        let output = check(&["read", &path, input, "--json"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let rendered = String::from_utf8(output.stderr).unwrap();
        assert!(
            rendered.starts_with(&format!("{input}:{input_line}:{column}:")),
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
            format!("sections {{\"A\" last {{a{}:int}}}}", "[1]".repeat(DEPTH)),
            DEPTH,
            "A\n7",
        ),
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
    let params =
        gecode_iris_cli::protocol::parse_params("{\"z\":\"  a=b\\n\",\"a\":\"2\"}").unwrap();
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
        (
            "sections {\n\"NEED\"\n\"END\" last {\n}\n}",
            "END\n",
            "E-LINE-MATCH",
            1,
        ),
        (
            "sections {\n\"NEED\"\n\"END\" last {\nif 0 {} else {}\n}\n}",
            "END\n",
            "E-LINE-MATCH",
            4,
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
    for (route, levels) in [0, 1, 2]
        .into_iter()
        .flat_map(|route| [256, 4096].map(|levels| (route, levels)))
    {
        let mut source = String::new();
        let mut bytes = String::new();
        let mut suffix = String::new();
        let mut depth = 0;
        for level in 0..levels {
            if route == 0 || (route == 2 && level % 3 == 0) {
                source.push_str("repeat 1 {");
                suffix.push('}');
                depth += 1;
            } else {
                source.push_str("sections {\"A\" last {");
                suffix.push_str("}}");
                bytes.push_str("A\n");
            }
        }
        source.push_str("x:int");
        source.extend(suffix.chars().rev());
        bytes.push('7');
        let (dir, path) = fixture(source.as_bytes());
        let input = dir.path().join("input.bin");
        std::fs::write(&input, bytes).unwrap();
        let input = input.to_str().unwrap();
        for output in [
            check(&["read", &path, input, "--json"]),
            driver(&["read", &path, input]),
        ] {
            if levels == 256 {
                assert_eq!(output.status.code(), Some(0), "{output:?}");
                let expected = format!(
                    "{{\"iris\":\"0.1\",\"entries\":[{{\"name\":\"x\",\"type\":\"int\",\"depth\":{depth},\"value\":{}7{}}}]}}\n",
                    "[".repeat(depth),
                    "]".repeat(depth)
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

#[test]
fn check_locations_contexts_paths_and_spans() {
    // Pinned location probes also check the fields omitted by differential equality.
    let probes = [
        (
            r####"bare-check"####,
            r####"check 0 "plain""####,
            r####""####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed: plain", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 1, "column": null}, "context": null, "record_path": []}"####,
        ),
        (
            r####"empty-block-check"####,
            r####"if 1 {}
check 0
if 1 {}"####,
            r####""####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": []}"####,
        ),
        (
            r####"bare-block-check"####,
            r####"if 1 {
check 0
}"####,
            r####""####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": []}"####,
        ),
        (
            r####"local-after-empty"####,
            r####"repeat 2 {
x:int
if 1 {}
check x>0 "positive"
}"####,
            r####"1 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed: positive", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 4, "column": null}, "context": "repeat [2 of 2]", "record_path": []}"####,
        ),
        (
            r####"lifted-named"####,
            r####"repeat 2 as rows {x:int}
if 1 {}
check x>0"####,
            r####"1 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'rows'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 3}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 3, "column": null}, "context": null, "record_path": [1]}"####,
        ),
        (
            r####"lifted-unnamed-asymmetric"####,
            r####"repeat 3 {repeat 2 {x:int}}
check x>0"####,
            r####"1 2 3 4 5 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of record 3 of 'x'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 11}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [2, 1]}"####,
        ),
        (
            r####"mixed-witness"####,
            r####"lim[2]:int repeat 2 {x:int}
check all(l in lim:x<l)"####,
            r####"5 3 1 4"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for element 2 of 'lim' in record 2 of 'x'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 7}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [1, 1]}"####,
        ),
        (
            r####"array-cursor"####,
            r####"a[2][3]:int
check all(r in a:all(x in r:x>0))"####,
            r####"1 2 3
4 5 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for element 3 of 'r' in element 2 of 'a'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 2, "column": 6}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [1, 2]}"####,
        ),
        (
            r####"explicit-index-direct"####,
            r####"repeat 2 as rows {x:int}
check rows[1].x>0"####,
            r####"1 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": []}"####,
        ),
        (
            r####"explicit-index-all"####,
            r####"repeat 2 as jobs {repeat 2 as steps {x:int}}
check all(s in jobs[1].steps:s.x>0)"####,
            r####"1 2 3 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'steps'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 7}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [1]}"####,
        ),
        (
            r####"hard-inside-check"####,
            r####"repeat 2 {x:int}
check 1/x>0"####,
            r####"1 0"####,
            r####"{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"all-recognized-field"####,
            r####"repeat 2 as jobs {repeat 2 as steps {x:int}}
check all(j in jobs:all(v in j.x:v>0))"####,
            r####"1 2 3 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'steps' in record 2 of 'jobs'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 7}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [1, 1]}"####,
        ),
        (
            r####"section-check"####,
            r####"sections {
"X {x:int}" {
if 1 {}
check x>0 "positive"
}
}"####,
            r####"X 0
"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed: positive", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 4, "column": null}, "context": "section \"X {x:int}\"", "record_path": []}"####,
        ),
        (
            r####"section-repeat-check"####,
            r####"sections {
"GO" {
repeat 2 {
x:int
if 1 {}
check x>0
}
}
}"####,
            r####"GO
1 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 2, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 6, "column": null}, "context": "section \"GO\" > repeat [2 of 2]", "record_path": []}"####,
        ),
        (
            r####"repeat-section-check"####,
            r####"repeat 1 {
sections {
"X {x:int}" last {
if 1 {}
check x>0
}
}
}"####,
            r####"X 0
"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 5, "column": null}, "context": "repeat [1 of 1] > section \"X {x:int}\"", "record_path": []}"####,
        ),
        (
            r####"section-check-hard"####,
            r####"sections {
"X {x:int}" last {
if 1 {}
check 1/x
}
}"####,
            r####"X 0
"####,
            r####"{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 4, "column": null}, "context": "section \"X {x:int}\"", "record_path": null}"####,
        ),
        (
            r####"empty-last-after-check"####,
            r####"sections {
"NEED"
"END" last {
check 1
if 1 {}
}
}"####,
            r####"END
"####,
            r####"{"code": "E-LINE-MATCH", "message": "expected a line matching \"NEED\" before the line matching \"END\"", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 5, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"bare-last-after-check"####,
            r####"sections {
"NEED"
"END" last {
if 1 {check 1}
if 1 {}
}
}"####,
            r####"END
"####,
            r####"{"code": "E-LINE-MATCH", "message": "expected a line matching \"NEED\" before the line matching \"END\"", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 5, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"empty-body-after-check"####,
            r####"repeat 1 {
if 0 {x:int}
check 1
if 1 {}
}"####,
            r####""####,
            r####"{"code": "E-REPEAT-MISSING", "message": "'x' is defined in no repetition", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 1, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"unicode-crlf-record"####,
            r####"comment "#" repeat 2 as rows {a:word x:int}
check x>0"####,
            r####"# c
é 1
😀 0"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'rows'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 3, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [1]}"####,
        ),
        (
            r####"absent-nested-list"####,
            r####"repeat 2 as jobs {k:int if k {repeat 1 as steps {x:int}}}
let n=sum(j in jobs:size(j.steps))"####,
            r####"0 1 7"####,
            r####"{"iris": "0.1", "entries": [{"name": "k", "type": "int", "depth": 1, "value": [0, 1]}, {"name": "x", "type": "int", "depth": 2, "value": [[], [7]]}, {"name": "n", "type": "int", "depth": 0, "value": 1}]}"####,
        ),
        (
            r####"never-nested-list"####,
            r####"repeat 2 as jobs {k:int if k {repeat 1 as steps {x:int}}}
let n=sum(j in jobs:size(j.steps))"####,
            r####"0 0"####,
            r####"{"iris": "0.1", "entries": [{"name": "k", "type": "int", "depth": 1, "value": [0, 0]}, {"name": "x", "type": "int", "depth": 2, "value": [[], []]}, {"name": "n", "type": "int", "depth": 0, "value": 0}]}"####,
        ),
        (
            r####"absent-nested-each"####,
            r####"repeat 2 as jobs {k:int if k {repeat 1 as steps {each step let z=1}}}
check step.z>0"####,
            r####"0 0"####,
            r####"{"iris": "0.1", "entries": [{"name": "k", "type": "int", "depth": 1, "value": [0, 0]}, {"name": "z", "type": "int", "depth": 2, "value": [[], []]}]}"####,
        ),
        (
            r####"empty-nested-records"####,
            r####"repeat 2 as jobs {each job repeat 3 as steps {each step}}
check all(j in jobs:all(s in j.steps:0))"####,
            r####""####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for record 1 of 'steps' in record 1 of 'jobs'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [0, 0]}"####,
        ),
        (
            r####"current-outer-before-inner"####,
            r####"repeat 1 as jobs {each job let before=job.x repeat 1 {x:int}}"####,
            r####"7"####,
            r####"{"code": "E-NAME-UNDEFINED", "message": "'x' is not defined yet", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 1, "column": null}, "context": "repeat [1 of 1]", "record_path": null}"####,
        ),
        (
            r####"all-error-clears-path"####,
            r####"repeat 2 as rows {x:int}
check all(r in rows:1/r.x)"####,
            r####"1 0"####,
            r####"{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"and-first"####,
            r####"a[2]:int
check all(x in a:x>1)&&all(y in a:y>2)"####,
            r####"1 2"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for element 1 of 'a'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [0]}"####,
        ),
        (
            r####"or-last"####,
            r####"a[2]:int
check all(x in a:x>2)||all(y in a:y>1)"####,
            r####"2 1"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for element 2 of 'a'", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 2, "column": null}, "context": null, "record_path": [1]}"####,
        ),
        (
            r####"list-index-before-availability"####,
            r####"let v=rows[missing].x repeat 0 as rows {x:int}"####,
            r####""####,
            r####"{"code": "E-NAME-UNDEFINED", "message": "'rows' is not defined yet", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 1, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"list-index-type-before-availability"####,
            r####"let v=rows[0.0].x repeat 0 as rows {x:int}"####,
            r####""####,
            r####"{"code": "E-NAME-UNDEFINED", "message": "'rows' is not defined yet", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 1, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"list-index-unavailable"####,
            r####"let v=rows[0].x repeat 0 as rows {x:int}"####,
            r####""####,
            r####"{"code": "E-NAME-UNDEFINED", "message": "'rows' is not defined yet", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 1, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"current-outer-fallback"####,
            r####"let x=9 repeat 1 as jobs {each job let v=job.x x:int}"####,
            r####"7"####,
            r####"{"code": "E-SPEC-FIELD", "message": "'x' is defined inside and outside the repeat of 'jobs', so it cannot be a field of its records", "input_location": null, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 1, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"before-nested-list"####,
            r####"repeat 1 as jobs {each job let v=size(job.steps) repeat 1 as steps {x:int}}"####,
            r####"7"####,
            r####"{"code": "E-NAME-UNDEFINED", "message": "'steps' is not defined yet", "input_location": {"origin": "/tmp/iris-zdev/rust-007-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-007-probe.iris", "line": 1, "column": null}, "context": "repeat [1 of 1]", "record_path": null}"####,
        ),
    ];
    for (name, source, bytes, expected) in probes {
        let (dir, path) = fixture(source.as_bytes());
        let input = dir.path().join("input.bin");
        std::fs::write(&input, bytes).unwrap();
        let input = input.to_str().unwrap();
        let expected: serde_json::Value = serde_json::from_str(expected).unwrap();
        if expected.get("code").is_some() && expected["input_location"].is_null() {
            let gecode_iris_core::Failure::Language(error) =
                gecode_iris_core::Spec::parse(source.as_bytes(), &path).unwrap_err()
            else {
                panic!("{name}: language error")
            };
            assert_eq!(error.code.as_str(), expected["code"].as_str().unwrap());
            let output = driver(&["read", &path, input]);
            let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(actual["code"], expected["code"], "{name}");
            assert_eq!(actual["message"], expected["message"], "{name}");
            assert_eq!(
                actual["input_location"], expected["input_location"],
                "{name}"
            );
            assert_eq!(actual["context"], expected["context"], "{name}");
            assert_eq!(actual["record_path"], expected["record_path"], "{name}");
            assert_eq!(
                actual["spec_location"]["line"], expected["spec_location"]["line"],
                "{name}"
            );
            continue;
        }
        let spec = gecode_iris_core::Spec::parse(source.as_bytes(), &path).unwrap();
        let result = spec.read(bytes.as_bytes(), input, &gecode_iris_core::Params::new());
        let output = driver(&["read", &path, input]);
        assert!(output.stderr.is_empty(), "{name}: {output:?}");
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        if expected.get("code").is_none() {
            assert_eq!(output.status.code(), Some(0), "{name}");
            assert_eq!(actual, expected, "{name}");
            assert!(result.is_ok(), "{name}");
            continue;
        }
        assert_eq!(output.status.code(), Some(1), "{name}");
        for field in ["code", "message", "context", "record_path"] {
            assert_eq!(actual[field], expected[field], "{name}: {field}");
        }
        for location in ["spec_location", "input_location"] {
            for field in ["line", "column"] {
                assert_eq!(
                    actual[location][field], expected[location][field],
                    "{name}: {location}.{field}"
                );
            }
        }
        let gecode_iris_core::Failure::Language(error) = result.unwrap_err() else {
            panic!("{name}: language error")
        };
        assert_eq!(
            error.record_path().map(|p| p.to_vec()),
            serde_json::from_value::<Option<Vec<usize>>>(expected["record_path"].clone()).unwrap(),
            "{name}"
        );
        let spec_line = expected["spec_location"]["line"].as_u64().unwrap() as usize;
        let start = usize::from(error.spec_span().unwrap().start());
        assert_eq!(
            source[..start].bytes().filter(|&b| b == b'\n').count() + 1,
            spec_line,
            "{name}"
        );
        if matches!(
            error.code,
            gecode_iris_core::ErrorCode::CheckFailed | gecode_iris_core::ErrorCode::DivisionZero
        ) {
            let parsed = gecode_iris_syntax::parse(
                source.as_bytes(),
                &path,
                gecode_iris_syntax::LanguageVersion::V0_1,
            )
            .unwrap();
            let node = parsed
                .syntax()
                .descendants()
                .find(|node| {
                    node.kind() == gecode_iris_syntax::SyntaxKind::Check
                        && source[..usize::from(node.text_range().start())]
                            .bytes()
                            .filter(|&b| b == b'\n')
                            .count()
                            + 1
                            == spec_line
                })
                .unwrap();
            assert_eq!(
                error.spec_span(),
                Some(node.text_range()),
                "{name}: complete check statement span"
            );
        }
        let line = expected["input_location"]["line"].as_u64().unwrap() as usize;
        let column = expected["input_location"]["column"].as_u64().unwrap() as usize;
        let offset = bytes
            .split_inclusive('\n')
            .take(line - 1)
            .map(str::len)
            .sum::<usize>()
            + column
            - 1;
        assert_eq!(error.input_span().unwrap().bytes, offset..offset, "{name}");
        let output = check(&["read", &path, input, "--json"]);
        assert_eq!(output.status.code(), Some(1), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains('^'), "{name}: {stderr}");
        assert!(
            stderr.contains(&format!("spec: {path}:{spec_line}")),
            "{name}: {stderr}"
        );
    }
    let spec = gecode_iris_core::Spec::parse(
        b"repeat 2 as rows {x:int} check all(r in rows:r.x>0)",
        "spec",
    )
    .unwrap();
    for input in [b"1 0", b"1 2", b"0 1", b"3 4"] {
        let result = spec.read(input, "input", &gecode_iris_core::Params::new());
        assert_eq!(result.is_ok(), input == b"1 2" || input == b"3 4");
    }
}

#[test]
fn deeply_nested_checks_respect_parser_admission() {
    for mixed in [false, true] {
        for levels in [256, 4096] {
            for truth in [0, 1] {
                let mut source = String::new();
                let mut suffix = String::new();
                let mut input = String::new();
                for level in 0..levels {
                    if mixed && level % 2 == 1 {
                        source.push_str("sections {\"A\" last {");
                        suffix.push_str("}}");
                        input.push_str("A\n");
                    } else {
                        source.push_str("repeat 1 {");
                        suffix.push('}');
                    }
                }
                source.push_str(&format!("check {truth}"));
                source.extend(suffix.chars().rev());
                let (dir, path) = fixture(source.as_bytes());
                let input_path = dir.path().join("input.bin");
                std::fs::write(&input_path, input).unwrap();
                let input = input_path.to_str().unwrap();
                for output in [
                    check(&["read", &path, input, "--json"]),
                    driver(&["read", &path, input]),
                ] {
                    assert_eq!(
                        output.status.code(),
                        Some(i32::from(levels == 4096 || truth == 0)),
                        "{output:?}"
                    );
                    let diagnostic = format!(
                        "{}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                    if levels == 4096 {
                        assert!(
                            diagnostic.contains("blocks are nested too deeply"),
                            "{diagnostic}"
                        );
                    } else if truth == 0 {
                        assert!(diagnostic.contains("check failed"), "{diagnostic}");
                    } else {
                        assert_eq!(output.stdout, b"{\"iris\":\"0.1\",\"entries\":[]}\n");
                    }
                }
            }
        }
    }
    println!(
        "Checks execute at 256 admitted bodies, including mixed repeat/sections; 4096 bodies reject cleanly"
    );
    let mut largest = 0;
    for depth in [16, 32, 64, 128, 254, 255, 256, 4096] {
        let mut source = (0..depth)
            .map(|i| format!("repeat 1 as rows{i} {{"))
            .collect::<String>();
        source.push_str(&"}".repeat(depth));
        source.push_str("check ");
        for i in 0..depth {
            source.push_str(&format!(
                "all(r{i} in {}: ",
                if i == 0 {
                    "rows0".into()
                } else {
                    format!("r{}.rows{i}", i - 1)
                }
            ));
        }
        source.push('0');
        source.push_str(&")".repeat(depth));
        let (dir, path) = fixture(source.as_bytes());
        let input = dir.path().join("input.bin");
        std::fs::write(&input, []).unwrap();
        let input = input.to_str().unwrap();
        let parsed = check(&["check", &path]);
        if parsed.status.success() {
            largest = depth;
        }
        for output in [
            check(&["read", &path, input, "--json"]),
            driver(&["read", &path, input]),
        ] {
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if parsed.status.success() {
                assert!(diagnostic.contains("check failed"), "{diagnostic}");
            } else {
                assert!(diagnostic.contains("nested too deeply"), "{diagnostic}");
            }
        }
    }
    assert!(largest >= 64);
    println!("Largest tested admitted nested record comprehension: {largest}");
    for depth in [255, 4096] {
        let source = format!("check {}0{}", "(".repeat(depth), ")".repeat(depth));
        let (dir, path) = fixture(source.as_bytes());
        let input = dir.path().join("input.bin");
        std::fs::write(&input, []).unwrap();
        let input = input.to_str().unwrap();
        for output in [
            check(&["read", &path, input, "--json"]),
            driver(&["read", &path, input]),
        ] {
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                diagnostic.contains(if depth == 4096 {
                    "expressions are nested too deeply"
                } else {
                    "check failed"
                }),
                "{diagnostic}"
            );
        }
    }
    for repeats in [0, 256] {
        for (dimensions, value, condition) in [
            (
                "[1]".repeat(16384),
                "7",
                "sum(a)==7 && sum(a[0])==7 && all(r in a:sum(r)==7)",
            ),
            (
                format!("[0]{}", "[1]".repeat(16383)),
                "",
                "sum(a)==0 && all(r in a:sum(r)==0)",
            ),
        ] {
            let source = format!(
                "{}a{dimensions}:int{} check {condition}",
                "repeat 1 {".repeat(repeats),
                "}".repeat(repeats)
            );
            let (dir, path) = fixture(source.as_bytes());
            let input = dir.path().join("input.bin");
            std::fs::write(&input, value).unwrap();
            let input = input.to_str().unwrap();
            for output in [
                check(&["read", &path, input, "--json"]),
                driver(&["read", &path, input]),
            ] {
                assert_eq!(output.status.code(), Some(0), "{output:?}");
                let depth = 16384 + repeats;
                let array = if value.is_empty() {
                    format!("{}[]{}", "[".repeat(repeats), "]".repeat(repeats))
                } else {
                    format!("{}7{}", "[".repeat(depth), "]".repeat(depth))
                };
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap(),
                    format!(
                        "{{\"iris\":\"0.1\",\"entries\":[{{\"name\":\"a\",\"type\":\"int\",\"depth\":{depth},\"value\":{array}}}]}}\n"
                    )
                );
            }
        }
        let source = format!(
            "{}a{}:int{} check all(r in a:0)",
            "repeat 1 {".repeat(repeats),
            "[1]".repeat(16384),
            "}".repeat(repeats)
        );
        let (dir, path) = fixture(source.as_bytes());
        let input = dir.path().join("input.bin");
        std::fs::write(&input, "7").unwrap();
        let input = input.to_str().unwrap();
        let output = driver(&["read", &path, input]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let error: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(error["record_path"], serde_json::json!([0]));
        let output = check(&["read", &path, input, "--json"]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn instance_and_param_locations_contexts_and_paths() {
    let probes = [
        (
            r####"empty-instance-block"####,
            r####"read"####,
            r####"
instances {}"####,
            r####"1"####,
            r####"{}"####,
            r####"{"code": "E-NO-PROGRESS", "message": "an instance must read some input", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"bare-instance-block"####,
            r####"read"####,
            r####"instances {
 {}
}"####,
            r####"
1"####,
            r####"{}"####,
            r####"{"code": "E-SPEC-SYNTAX", "message": "expected a statement, found '{'", "input_location": null, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"invalid-number"####,
            r####"read"####,
            r####"
instances {n:int}"####,
            r####""####,
            r####"{"instance": "0"}"####,
            r####"{"code": "E-INSTANCE-NUMBER", "message": "instance numbers start at 1", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"not-found"####,
            r####"read"####,
            r####"
instances {n:int}"####,
            r####"1
"####,
            r####"{"instance": "2"}"####,
            r####"{"code": "E-INSTANCE-NOT-FOUND", "message": "there is no instance '2' (the file has 1 instance)", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 2, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"empty"####,
            r####"read"####,
            r####"
instances {n:int}"####,
            r####""####,
            r####"{}"####,
            r####"{"code": "E-INSTANCE-EMPTY", "message": "the file contains no instance", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"selection-required"####,
            r####"read"####,
            r####"
instances {n:int}"####,
            r####"1
2
"####,
            r####"{}"####,
            r####"{"code": "E-INSTANCE-SELECTION", "message": "the file has 2 instances; select one with the parameter instance (a number from 1, or a name)", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 3, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"selected-read"####,
            r####"read"####,
            r####"instances {
n:int
}"####,
            r####"1
bad"####,
            r####"{"instance": "2"}"####,
            r####"{"code": "E-EXPECTED-VALUE", "message": "expected int, found 'bad'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 2, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [2] > n", "record_path": null}"####,
        ),
        (
            r####"skipped-read"####,
            r####"read"####,
            r####"instances {
n:int name:word
}"####,
            r####"bad a 2 b"####,
            r####"{"instance": "2"}"####,
            r####"{"code": "E-EXPECTED-VALUE", "message": "expected int, found 'bad'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1] > n", "record_path": null}"####,
        ),
        (
            r####"deferred-record"####,
            r####"read"####,
            r####"instances {
n:int repeat n {x:int}
check x>0 "positive"
name:word
}"####,
            r####"2 1 0 a
1 5 b"####,
            r####"{"instance": "a"}"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed for record 2 of 'x': positive", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 5}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 3, "column": null}, "context": "instance [1]", "record_path": [1]}"####,
        ),
        (
            r####"deferred-then-hard"####,
            r####"read"####,
            r####"instances {
n:int check n>0 "first"
x:int name:word}"####,
            r####"0 bad a"####,
            r####"{"instance": "a"}"####,
            r####"{"code": "E-EXPECTED-VALUE", "message": "expected int, found 'bad'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 3}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 3, "column": null}, "context": "instance [1] > x", "record_path": null}"####,
        ),
        (
            r####"deferred-then-immediate"####,
            r####"read"####,
            r####"instances {
n:int check n>0 "first"
name:word check 0 "second"}"####,
            r####"0 a"####,
            r####"{"instance": "a"}"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed: second", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 4}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 3, "column": null}, "context": "instance [1]", "record_path": []}"####,
        ),
        (
            r####"empty-name-immediate"####,
            r####"read"####,
            r####"instances {
n:int check 0
name:word}"####,
            r####"1 a"####,
            r####"{"instance": ""}"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 2}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1]", "record_path": []}"####,
        ),
        (
            r####"line-is-hard"####,
            r####"read"####,
            r####"instances {
line "A"
}"####,
            r####"B
A"####,
            r####"{"instance": "2"}"####,
            r####"{"code": "E-LINE-MATCH", "message": "expected a line matching \"A\"", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"check-expression-is-hard"####,
            r####"read"####,
            r####"instances {
n:int check n/0
name:word}"####,
            r####"1 a"####,
            r####"{"instance": "a"}"####,
            r####"{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 2}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"read-repeated-hard"####,
            r####"read"####,
            r####"instances {
sections until "END" {
"A {n:int}" {check n>0}
}
name:word
}"####,
            r####"A 1
A bad
END
a
"####,
            r####"{"instance": "b"}"####,
            r####"{"code": "E-SECTIONS-UNEXPECTED", "message": "unexpected line; expected one of \"A {n:int}\"", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 2, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"read-repeated-rethrow"####,
            r####"read"####,
            r####"instances {
sections until "END" {
"A {n:int}" {}
}
name:word
}"####,
            r####"A 1
A 2
END
a
"####,
            r####"{"instance": "a"}"####,
            r####"{"code": "E-SECTIONS-REPEATED", "message": "a second line matching \"A {n:int}\" (the first is on line 1)", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 2, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"read-missing-eof"####,
            r####"read"####,
            r####"instances {
sections until "END" {
"A" {}
}
}"####,
            r####"END
"####,
            r####"{"instance": "b"}"####,
            r####"{"code": "E-LINE-MATCH", "message": "expected a line matching \"A\" before 'END'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"read-missing-comment-eof"####,
            r####"read"####,
            r####"comment "#"
instances {
sections until "END" {"A"}
}"####,
            r####"END
# tail
"####,
            r####"{"instance": "b"}"####,
            r####"{"code": "E-LINE-MATCH", "message": "expected a line matching \"A\" before 'END'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 3, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"read-missing-last-body"####,
            r####"read"####,
            r####"instances {
sections {
"A"
"B" last {
let x=1
}
}
name:word
}"####,
            r####"B
a"####,
            r####"{"instance": "a"}"####,
            r####"{"code": "E-LINE-MATCH", "message": "expected a line matching \"A\" before the line matching \"B\"", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 5, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"read-param-nested"####,
            r####"read"####,
            r####"instances {
n:int repeat n {
param p
}
}"####,
            r####"1"####,
            r####"{"instance": "b"}"####,
            r####"{"code": "E-PARAM-MISSING", "message": "parameter 'p' must be given", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 2}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 3, "column": null}, "context": "instance [1] > repeat [1 of 1]", "record_path": null}"####,
        ),
        (
            r####"catalog-repeated-hard"####,
            r####"catalog"####,
            r####"instances {
sections until "END" {
"A {n:int}" {check n>0}
}
name:word
}"####,
            r####"A 1
A bad
END
a
"####,
            r####"{}"####,
            r####"{"code": "E-SECTIONS-UNEXPECTED", "message": "unexpected line; expected one of \"A {n:int}\"", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 2, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"catalog-repeated-rethrow"####,
            r####"catalog"####,
            r####"instances {
sections until "END" {
"A {n:int}" {}
}
name:word
}"####,
            r####"A 1
A 2
END
a
"####,
            r####"{}"####,
            r####"{"iris": "0.1", "entries": [{"name": "instances", "type": "int", "depth": 0, "value": 1}, {"name": "name", "type": "string", "depth": 1, "value": ["a"]}, {"name": "failed", "type": "string", "depth": 1, "value": ["a second line matching \"A {n:int}\""]}]}"####,
        ),
        (
            r####"catalog-missing-eof"####,
            r####"catalog"####,
            r####"instances {
sections until "END" {
"A" {}
}
}"####,
            r####"END
"####,
            r####"{}"####,
            r####"{"code": "E-LINE-MATCH", "message": "expected a line matching \"A\" before 'END'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"catalog-missing-comment-eof"####,
            r####"catalog"####,
            r####"comment "#"
instances {
sections until "END" {"A"}
}"####,
            r####"END
# tail
"####,
            r####"{}"####,
            r####"{"code": "E-LINE-MATCH", "message": "expected a line matching \"A\" before 'END'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 3, "column": null}, "context": "instance [1]", "record_path": null}"####,
        ),
        (
            r####"catalog-missing-last-body"####,
            r####"catalog"####,
            r####"instances {
sections {
"A"
"B" last {
let x=1
}
}
name:word
}"####,
            r####"B
a"####,
            r####"{}"####,
            r####"{"iris": "0.1", "entries": [{"name": "instances", "type": "int", "depth": 0, "value": 1}, {"name": "name", "type": "string", "depth": 1, "value": ["a"]}, {"name": "failed", "type": "string", "depth": 1, "value": ["no line matching \"A\""]}]}"####,
        ),
        (
            r####"catalog-param-nested"####,
            r####"catalog"####,
            r####"instances {
n:int repeat n {
param p
}
}"####,
            r####"1"####,
            r####"{}"####,
            r####"{"code": "E-PARAM-MISSING", "message": "parameter 'p' must be given", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 2}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 3, "column": null}, "context": "instance [1] > repeat [1 of 1]", "record_path": null}"####,
        ),
        (
            r####"param-missing"####,
            r####"read"####,
            r####"
param p"####,
            r####""####,
            r####"{}"####,
            r####"{"code": "E-PARAM-MISSING", "message": "parameter 'p' must be given", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"param-type"####,
            r####"read"####,
            r####"n:int
param p=1"####,
            r####"5"####,
            r####"{"p": "1.5"}"####,
            r####"{"code": "E-PARAM-TYPE", "message": "parameter 'p' must be an integer, not '1.5'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 2}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"param-default-error"####,
            r####"read"####,
            r####"
param p=1/0"####,
            r####""####,
            r####"{"p": "1"}"####,
            r####"{"code": "E-DIVISION-ZERO", "message": "division by zero", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"param-unknown-order"####,
            r####"read"####,
            r####"param a"####,
            r####""####,
            r####"{"z": "x", "b": "2"}"####,
            r####"{"code": "E-PARAM-UNKNOWN", "message": "the spec has no parameter 'z'", "input_location": null, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": null, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"param-nested-block"####,
            r####"read"####,
            r####"if 1 {
{}
param p
}"####,
            r####""####,
            r####"{}"####,
            r####"{"code": "E-SPEC-SYNTAX", "message": "expected a statement, found '{'", "input_location": null, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": null}"####,
        ),
        (
            r####"catalog-no-block"####,
            r####"catalog"####,
            r####"
check 0"####,
            r####""####,
            r####"{}"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 2, "column": null}, "context": null, "record_path": []}"####,
        ),
        (
            r####"catalog-prefix-check"####,
            r####"catalog"####,
            r####"check 0
instances {n:int}"####,
            r####"1"####,
            r####"{}"####,
            r####"{"code": "E-CHECK-FAILED", "message": "check failed", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 1}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 1, "column": null}, "context": null, "record_path": []}"####,
        ),
        (
            r####"catalog-hard-after-deferred"####,
            r####"catalog"####,
            r####"instances {
n:int check n>0
x:int
}"####,
            r####"0 bad"####,
            r####"{}"####,
            r####"{"code": "E-EXPECTED-VALUE", "message": "expected int, found 'bad'", "input_location": {"origin": "/tmp/iris-zdev/rust-008-probe-input.txt", "line": 1, "column": 3}, "spec_location": {"origin": "/tmp/iris-zdev/rust-008-probe.iris", "line": 3, "column": null}, "context": "instance [1] > x", "record_path": null}"####,
        ),
    ];
    for (name, operation, source, bytes, params, expected) in probes {
        let (dir, path) = fixture(source.as_bytes());
        let input = dir.path().join("input.bin");
        std::fs::write(&input, bytes).unwrap();
        let output = driver(&[
            operation,
            &path,
            input.to_str().unwrap(),
            "--params",
            params,
        ]);
        assert!(output.stderr.is_empty(), "{name}: {output:?}");
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let expected: serde_json::Value = serde_json::from_str(expected).unwrap();
        assert_eq!(
            output.status.code(),
            Some(if expected.get("code").is_some() { 1 } else { 0 }),
            "{name}"
        );
        if expected.get("code").is_none() {
            assert_eq!(actual, expected, "{name}");
            continue;
        }
        for field in ["code", "message", "context", "record_path"] {
            assert_eq!(actual[field], expected[field], "{name}: {field}");
        }
        for location in ["spec_location", "input_location"] {
            if expected[location].is_null() {
                assert!(actual[location].is_null(), "{name}: {location}");
            } else {
                for field in ["line", "column"] {
                    assert_eq!(
                        actual[location][field], expected[location][field],
                        "{name}: {location}.{field}"
                    );
                }
            }
        }
    }
}

#[test]
fn list_and_read_params() {
    let (dir, path) = fixture(b"param p=1 instances {x:int check x>0 \"positive\" name:word}");
    let input = dir.path().join("input.bin");
    let input = input.to_str().unwrap();
    std::fs::write(input, "0 a\n2 b\n").unwrap();
    let output = check(&["list", &path, input, input, "--param", "instance=unmatched"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{input}: 2 instances\n  1 a (failed: positive)\n  2 b\n").repeat(2)
    );
    let output = check(&[
        "read",
        &path,
        input,
        "--json",
        "--param",
        "instance=2",
        "--param",
        "p=3",
        "--param",
        "p=7",
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["entries"][0]["value"], 7);
    assert_eq!(document["entries"][1]["value"], 2);
    for args in [
        vec!["list", &path],
        vec!["read", &path, input, "--json", "--param", "bad"],
        vec!["list", &path, input, "--param", "=value"],
    ] {
        assert_eq!(check(&args).status.code(), Some(2));
    }
    for args in [
        vec!["catalog", &path],
        vec!["catalog", &path, input, "--format", "bad"],
        vec![
            "catalog", &path, input, "--format", "json", "--format", "json",
        ],
        vec!["read", &path, input, "--format", "json"],
    ] {
        let output = driver(&args);
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
    }
    std::fs::write(&path, "instances {x:int check x>0}").unwrap();
    std::fs::write(input, "1 0 1").unwrap();
    let output = check(&["list", &path, input]);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{input}: 3 instances\n  2 (failed: check on line 1 of the spec)\n")
    );
    std::fs::write(input, "").unwrap();
    assert_eq!(
        String::from_utf8(check(&["list", &path, input]).stdout).unwrap(),
        format!("{input}: 0 instances\n")
    );
    std::fs::write(&path, "param p=\"\"").unwrap();
    let output = check(&["read", &path, input, "--json", "--param", "p= a,b=c "]);
    assert_eq!(output.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["entries"][0]["value"], " a,b=c ");
    assert_eq!(
        String::from_utf8(check(&["list", &path, input]).stdout).unwrap(),
        format!("{input}: 1 instance\n")
    );
}

#[test]
fn deeply_nested_instance_bodies_use_the_work_stack() {
    let source = format!(
        "instances {{{}x:int{} name:word}}",
        "repeat 1 {".repeat(255),
        "}".repeat(255)
    );
    let (dir, path) = fixture(source.as_bytes());
    let input = dir.path().join("input.bin");
    let input = input.to_str().unwrap();
    std::fs::write(input, "7 a 8 b").unwrap();
    for args in [
        vec!["read", &path, input, "--params", "{\"instance\":\"b\"}"],
        vec!["catalog", &path, input],
    ] {
        let output = driver(&args);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    }
}
