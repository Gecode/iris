use std::{
    path::Path,
    process::{Command, Output},
};

fn iris(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iris"))
        .args(args)
        .output()
        .unwrap()
}

fn assert_status(output: &Output, status: i32) {
    assert_eq!(output.status.code(), Some(status), "{output:?}");
    if status == 0 {
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

#[test]
fn summary_scalars_shapes_and_multiple_files() {
    let dir = tempfile::tempdir().unwrap();
    let spec = dir.path().join("spec.iris");
    let input = dir.path().join("input.txt");
    let other = dir.path().join("other.txt");
    std::fs::write(
        &spec,
        br#"format "sample format"
let scalar=7 let floating=1.5 let text="a\n\"b"
empty[0][2]:int cube[1][2][0]:float
repeat 3 {n:int row[n]:int}"#,
    )
    .unwrap();
    std::fs::write(&input, "0 2 10 20 1 30").unwrap();
    std::fs::write(&other, "1 9 1 8 1 7").unwrap();
    let spec = spec.to_str().unwrap();
    let input = input.to_str().unwrap();
    let other = other.to_str().unwrap();
    let prefix = "  scalar: int 7\n  floating: float 1.5\n  text: string \"a\\n\\\"b\"\n  empty: int array 0 x 0\n  cube: float array 1 x 2 x ...\n  n: int array 3\n";
    let expected = format!(
        "{input} [sample format]\n{prefix}  row: int array 3 x 0..2\n{other} [sample format]\n{prefix}  row: int array 3 x 1\n"
    );
    for options in [vec![], vec!["--summary"]] {
        let mut args = vec!["read", spec, input, other];
        args.extend(options);
        let output = iris(&args);
        assert_status(&output, 0);
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
    for (mode, operation) in [("--json", "read"), ("--dzn", "dzn")] {
        let mut expected = Vec::new();
        for file in [input, other] {
            let output = Command::new(env!("CARGO_BIN_EXE_iris-driver"))
                .args([operation, spec, file])
                .output()
                .unwrap();
            assert_status(&output, 0);
            expected.extend(output.stdout);
        }
        let output = iris(&["read", spec, input, other, mode]);
        assert_status(&output, 0);
        assert_eq!(output.stdout, expected);
    }
    // Preserve the existing all-files-successful output boundary for every mode.
    std::fs::write(other, "bad").unwrap();
    for mode in ["--summary", "--json", "--dzn"] {
        let output = iris(&["read", spec, input, other, mode]);
        assert_status(&output, 1);
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn summary_empty_data_and_instance_parameters() {
    let dir = tempfile::tempdir().unwrap();
    let spec = dir.path().join("spec.iris");
    let input = dir.path().join("input.txt");
    std::fs::write(&spec, "").unwrap();
    std::fs::write(&input, "").unwrap();
    let spec = spec.to_str().unwrap();
    let input = input.to_str().unwrap();
    let output = iris(&["read", spec, input]);
    assert_status(&output, 0);
    assert_eq!(output.stdout, format!("{input}\n").as_bytes());

    std::fs::write(spec, "param p=1 instances {name:word x:int}").unwrap();
    std::fs::write(input, "a 7 b 8").unwrap();
    for selector in ["instance=b", "instance=2"] {
        let output = iris(&[
            "read", spec, input, "--param", "p=3", "--param", "p=9", "--param", selector,
        ]);
        assert_status(&output, 0);
        assert_eq!(
            output.stdout,
            format!("{input}\n  p: int 9\n  name: string \"b\"\n  x: int 8\n").as_bytes()
        );
    }
    for subcommand in ["read", "list"] {
        let output = iris(&[subcommand, spec, input, "--param", "unknown=1"]);
        assert_status(&output, 1);
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn summary_deep_dimensions_are_iterative() {
    const DEPTH: usize = 65_536;
    let dir = tempfile::tempdir().unwrap();
    let spec = dir.path().join("deep.iris");
    let input = dir.path().join("input.txt");
    for empty in [false, true] {
        std::fs::write(
            &spec,
            format!(
                "a[{}]{}:int",
                if empty { 0 } else { 1 },
                "[1]".repeat(DEPTH - 1)
            ),
        )
        .unwrap();
        std::fs::write(&input, if empty { "" } else { "7" }).unwrap();
        let output = iris(&["read", spec.to_str().unwrap(), input.to_str().unwrap()]);
        assert_status(&output, 0);
        let shape = if empty { "0 x 0" } else { "1 x 1" };
        assert_eq!(
            output.stdout,
            format!(
                "{}\n  a: int array {shape}{}\n",
                input.display(),
                " x ...".repeat(DEPTH - 2)
            )
            .as_bytes()
        );
    }
}

#[test]
fn output_modes_and_subcommand_usage_exit_two() {
    for args in [
        vec!["read"],
        vec!["read", "spec"],
        vec!["read", "spec", "input", "--summary", "--json"],
        vec!["read", "spec", "input", "--summary", "--dzn"],
        vec!["read", "spec", "input", "--json", "--dzn"],
        vec!["read", "spec", "input", "--param", "bad"],
        vec!["list"],
        vec!["check"],
        vec!["fmt"],
        vec!["lsp", "extra"],
    ] {
        let output = iris(&args);
        assert_status(&output, 2);
        assert!(output.stdout.is_empty());
    }
    let output = iris(&["--version"]);
    assert_status(&output, 0);
    assert_eq!(
        output.stdout,
        format!("iris {} (languages: 0.1)\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
}

fn run_conformance(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iris"))
        .current_dir(cwd)
        .arg("conformance")
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn conformance_defaults_options_exemptions_and_exit_statuses() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("conformance/cases/0.1/tiny");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("spec.iris"), "x:int").unwrap();
    std::fs::write(root.join("input.txt"), "7").unwrap();
    let metadata = "operation = \"read\"\n";
    std::fs::write(root.join("case.toml"), metadata).unwrap();
    let actual = r#"{"iris":"0.1","entries":[{"name":"x","type":"int","depth":0,"value":7}]}"#;
    std::fs::write(root.join("expected.json"), actual).unwrap();
    let driver = format!("\"{}\"", env!("CARGO_BIN_EXE_iris-driver"));
    let root_text = root.to_str().unwrap();
    for args in [
        vec![driver.as_str()],
        vec![
            &driver,
            root_text,
            root_text,
            "--filter",
            "tiny",
            "--version",
            "0.1",
            "--timeout",
            "5",
        ],
    ] {
        let output = run_conformance(dir.path(), &args);
        assert_status(&output, 0);
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .ends_with("Summary: total=1 pass=1 xfail=0 fail=0 xpass=0 unrepresentable=0\n")
        );
    }
    std::fs::write(root.join("expected.json"), actual.replace(":7", ":9")).unwrap();
    assert_status(&run_conformance(dir.path(), &[&driver, root_text]), 1);
    std::fs::create_dir_all(root.join("known")).unwrap();
    std::fs::write(root.join("known/alternate.json"), actual).unwrap();
    std::fs::write(root.join("case.toml"), format!("{metadata}\n[[known_failure]]\ndriver = \"test-driver\"\noutcome = \"data\"\nexpected_file = \"known/alternate.json\"\nreason = \"Controlled alternate for CLI test\"\n")).unwrap();
    assert_status(&run_conformance(dir.path(), &[&driver, root_text]), 1);
    let output = run_conformance(
        dir.path(),
        &[&driver, root_text, "--driver-id", "test-driver"],
    );
    assert_status(&output, 0);
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("xfail=1 fail=0")
    );
    std::fs::write(root.join("expected.json"), actual.replace(":7", ":9")).unwrap();
    std::fs::write(root.join("spec.iris"), "let x=9").unwrap();
    std::fs::write(root.join("input.txt"), "").unwrap();
    let output = run_conformance(
        dir.path(),
        &[&driver, root_text, "--driver-id", "test-driver"],
    );
    assert_status(&output, 1);
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("xpass=1")
    );
    for args in [
        vec![],
        vec![""],
        vec![&driver, "missing-root"],
        vec![&driver, root_text, "--filter", "no-such-case"],
        vec![&driver, root_text, "--driver-id", " "],
        vec![&driver, root_text, "--version", "1.0"],
        vec![&driver, root_text, "--timeout", "0"],
        vec![&driver, root_text, "--timeout", "NaN"],
        vec![&driver, root_text, "--timeout", "inf"],
        vec![&driver, root_text, "--timeout", "-1"],
        vec![&driver, root_text, "--unknown"],
    ] {
        let output = run_conformance(dir.path(), &args);
        assert_status(&output, 2);
        assert!(output.stdout.is_empty());
    }
}
