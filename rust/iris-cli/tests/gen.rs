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
fn the_jobshop_python_builder_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = iris(&["gen", "builder", "--lang", "python", spec.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-builder.py")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), recording);
}

#[test]
fn the_jobshop_cpp_builder_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = iris(&["gen", "builder", "--lang", "cpp", spec.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-builder.cpp")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), recording);
}

#[test]
fn the_jobshop_java_builder_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = iris(&["gen", "builder", "--lang", "java", spec.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-builder.java")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), recording);
}

#[test]
fn the_jobshop_kotlin_builder_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = iris(&["gen", "builder", "--lang", "kotlin", spec.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-builder.kt")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), recording);
}

#[test]
fn the_jobshop_typescript_builder_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = iris(&[
        "gen",
        "builder",
        "--lang",
        "typescript",
        spec.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-builder.ts")).unwrap();
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
    assert!(stderr(&output).contains("python"), "{}", stderr(&output));
    assert!(stderr(&output).contains("cpp"), "{}", stderr(&output));
    assert!(stderr(&output).contains("java"), "{}", stderr(&output));
    assert!(stderr(&output).contains("kotlin"), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("typescript"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_language_is_required() {
    let output = iris(&["gen", "builder", "spec.iris"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--lang"), "{}", stderr(&output));
}

fn generate_view(spec: &str) -> Output {
    iris(&["gen", "--lang", "rust", spec])
}

#[test]
fn the_jobshop_view_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = generate_view(spec.to_str().unwrap());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-view.rs")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), recording);
}

#[test]
fn a_view_of_a_rejected_spec_fails_as_the_check_does() {
    let directory = tempfile::tempdir().unwrap();
    let spec = write_spec(directory.path(), b"let x = nothing(1)\n");
    let generated = generate_view(&spec);
    assert_eq!(generated.status.code(), Some(1));
    assert_eq!(generated.stdout, b"");
    assert_eq!(stderr(&generated), stderr(&iris(&["check", &spec])));
}

#[test]
fn a_view_of_a_missing_file_fails_with_no_output() {
    let directory = tempfile::tempdir().unwrap();
    let spec = directory.path().join("missing.iris");
    let generated = generate_view(spec.to_str().unwrap());
    assert_eq!(generated.status.code(), Some(1));
    assert_eq!(generated.stdout, b"");
    assert!(stderr(&generated).contains("missing.iris"));
}

#[test]
fn a_view_needs_a_language_and_a_spec() {
    for arguments in [
        &["gen"][..],
        &["gen", "spec.iris"],
        &["gen", "--lang", "rust"],
        &["gen", "--lang", "cobol", "spec.iris"],
    ] {
        let output = iris(arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert_eq!(output.stdout, b"", "{arguments:?}");
    }
}

#[test]
fn the_python_jobshop_view_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = iris(&["gen", "--lang", "python", spec.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-view.py")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), recording);
}

#[test]
fn the_cpp_jobshop_view_matches_its_recording() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = root.join("../../formats/jobshop/jobshop.iris");
    let output = iris(&["gen", "--lang", "cpp", spec.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stderr(&output), "");
    let recording = fs::read_to_string(root.join("tests/fixtures/jobshop-view.hpp")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), recording);
}

#[test]
fn java_has_no_view_generator_yet() {
    let output = iris(&["gen", "--lang", "java", "spec.iris"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    assert!(
        stderr(&output).contains("java has no view generator yet"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("iris gen builder --lang java SPEC"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn kotlin_has_no_view_generator_yet() {
    let output = iris(&["gen", "--lang", "kotlin", "spec.iris"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    assert!(
        stderr(&output).contains("kotlin has no view generator yet"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("iris gen builder --lang kotlin SPEC"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn typescript_has_no_view_generator_yet() {
    let output = iris(&["gen", "--lang", "typescript", "spec.iris"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"");
    assert!(
        stderr(&output).contains("typescript has no view generator yet"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("iris gen builder --lang typescript SPEC"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_unknown_view_language_names_only_languages_with_views() {
    let output = iris(&["gen", "--lang", "cobol", "spec.iris"]);
    assert_eq!(output.status.code(), Some(2));
    let message = stderr(&output);
    assert!(
        message.contains("[possible values: rust, python, cpp]"),
        "{message}"
    );
}
