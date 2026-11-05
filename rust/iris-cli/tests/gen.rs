use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn iris(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iris"))
        .args(arguments)
        .output()
        .unwrap()
}

fn generate(spec: &str) -> Output {
    iris(&["gen", "builder", "--lang", "rust", spec])
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_spec(directory: &Path, text: &[u8]) -> String {
    let path = directory.join("spec.iris");
    fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_owned()
}

#[test]
fn the_jobshop_builder_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = generate(spec.to_str().unwrap());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-builder.rs")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), recording);
}

#[test]
fn a_rejected_spec_fails_as_the_check_does() {
    let directory = tempfile::tempdir().unwrap();
    let spec = write_spec(directory.path(), b"let x = nothing(1)\n");
    let check = iris(&["check", &spec]);
    let generated = generate(&spec);
    assert_eq!(generated.status.code(), Some(1));
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(generated.stdout, b"");
    assert_eq!(stderr(&generated), stderr(&check));
    assert!(stderr(&generated).contains("nothing"));
}

#[test]
fn a_spec_that_is_not_utf8_fails_with_no_output() {
    let directory = tempfile::tempdir().unwrap();
    let spec = write_spec(directory.path(), b"let x = \"\xff\"\n");
    let generated = generate(&spec);
    assert_eq!(generated.status.code(), Some(1));
    assert_eq!(generated.stdout, b"");
    assert_eq!(stderr(&generated), stderr(&iris(&["check", &spec])));
}

#[test]
fn a_missing_file_fails_with_no_output() {
    let directory = tempfile::tempdir().unwrap();
    let spec = directory.path().join("missing.iris");
    let generated = generate(spec.to_str().unwrap());
    assert_eq!(generated.status.code(), Some(1));
    assert_eq!(generated.stdout, b"");
    assert!(stderr(&generated).contains("missing.iris"));
}

#[test]
fn an_unknown_language_is_a_usage_error_that_names_the_choices() {
    let output = iris(&["gen", "builder", "--lang", "cobol", "spec.iris"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    assert!(stderr(&output).contains("rust"), "{}", stderr(&output));
}

#[test]
fn the_language_is_required() {
    let output = iris(&["gen", "builder", "spec.iris"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--lang"), "{}", stderr(&output));
}
