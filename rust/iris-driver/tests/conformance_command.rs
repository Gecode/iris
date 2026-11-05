use std::{
    path::Path,
    process::{Command, Output},
};

fn assert_status(output: &Output, status: i32) {
    assert_eq!(output.status.code(), Some(status), "{output:?}");
    if status == 0 {
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

fn run_conformance(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iris-cli-under-test"))
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
