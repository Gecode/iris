//! The hand-written builders in `examples/`, one per format in `formats/`. Each
//! case of a format is read through its builder and judged as the conformance
//! runner judges a driver: the same case loading, parameters, outcome decoding
//! and comparison. Each example also prints its `.out` recording, which the
//! Rust guide on the site shows.
use gecode_iris::{Failure, Spec};
use gecode_iris_cli::protocol::parse_params;
use gecode_iris_conformance::canonical::{Outcome, OutcomeKind};
use gecode_iris_conformance::case::{self, Case, Operation};
use gecode_iris_conformance::report::differences;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

// Each example is a program; its `main` is unused here.
#[allow(dead_code)]
#[path = "../examples/dimacs_cnf.rs"]
mod dimacs_cnf;
#[allow(dead_code)]
#[path = "../examples/jobshop.rs"]
mod jobshop;
#[allow(dead_code)]
#[path = "../examples/nonogram.rs"]
mod nonogram;
#[allow(dead_code)]
#[path = "../examples/sudoku.rs"]
mod sudoku;
#[allow(dead_code)]
#[path = "../examples/tsplib.rs"]
mod tsplib;
#[allow(dead_code)]
#[path = "../examples/tsptw.rs"]
mod tsptw;

/// Each format directory with the spec its builder builds.
fn built_specs() -> Vec<(&'static str, Spec)> {
    vec![
        ("dimacs-cnf", dimacs_cnf::dimacs_cnf().unwrap().spec),
        ("jobshop", jobshop::jobshop().unwrap().spec),
        ("nonogram", nonogram::nonogram().unwrap().spec),
        ("sudoku", sudoku::sudoku().unwrap().spec),
        ("tsplib", tsplib::tsplib().unwrap().spec),
        ("tsptw", tsptw::tsptw().unwrap().spec),
    ]
}

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn every_case_of_every_format_reads_through_its_builder() {
    let formats = repository().join("formats");
    let specs = built_specs();
    let names: Vec<&str> = specs.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        format_directories(&formats),
        "one builder per format"
    );

    let mut failures = Vec::new();
    let mut judged = 0;
    for (name, spec) in &specs {
        for path in case::discover(&[formats.join(name)], None).unwrap() {
            let case = Case::load(&path, "0.1").unwrap();
            let details = differences(&case.expected, &outcome(spec, &case));
            if !details.is_empty() {
                failures.push(format!("{}: {}", case.display_path, details.join("; ")));
            }
            judged += 1;
        }
    }
    println!("Format builders: {judged} cases");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(judged >= 2 * specs.len(), "too few cases: {judged}");
}

/// What a driver would report for `case`, with the built spec in place of the
/// case's own.
fn outcome(spec: &Spec, case: &Case) -> Outcome {
    let params = parse_params(case.params.as_deref().unwrap_or("{}")).unwrap();
    let read = |input: &PathBuf| {
        let bytes = fs::read(input).unwrap();
        let origin = input.to_string_lossy();
        match case.operation {
            Operation::Catalog => spec.catalog(&bytes, &origin, &params),
            _ => spec.read(&bytes, &origin, &params),
        }
    };
    let (kind, document) = match case.input.as_ref().map(read) {
        // `build()` has already parsed and checked the spec.
        None => return Outcome::Parse,
        Some(Ok(data)) if case.success_kind() == OutcomeKind::Dzn => {
            (OutcomeKind::Dzn, data.to_dzn().unwrap())
        }
        Some(Ok(data)) => (case.success_kind(), data.to_json().unwrap()),
        Some(Err(Failure::Language(error))) => (OutcomeKind::Error, error.to_json().unwrap()),
        Some(Err(other)) => panic!("{}: {other}", case.display_path),
    };
    Outcome::decode(kind, &document, "0.1").unwrap()
}

fn format_directories(formats: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(formats)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Builds the examples in a target directory of their own, since this test's
/// Cargo holds the workspace's, and runs each from the repository root, as the
/// guide says to.
#[test]
fn every_example_prints_its_recording() {
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("format-builders");
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args([
            "build",
            "--locked",
            "--offline",
            "--quiet",
            "-p",
            "gecode-iris",
        ])
        .arg("--examples")
        .env("CARGO_TARGET_DIR", &target)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join(".."))
        .status()
        .unwrap();
    assert!(status.success(), "building the examples failed");

    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut sources: Vec<PathBuf> = fs::read_dir(&examples)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    sources.sort();
    assert_eq!(sources.len(), built_specs().len(), "one example per format");
    for source in sources {
        let name = source.file_stem().unwrap().to_string_lossy().into_owned();
        let binary = target
            .join("debug/examples")
            .join(&name)
            .with_extension(std::env::consts::EXE_EXTENSION);
        let output = Command::new(binary)
            .current_dir(repository())
            .output()
            .unwrap();
        let stdout = with_forward_slashes(&String::from_utf8(output.stdout).unwrap());
        assert!(
            output.status.success(),
            "{name} failed:\n{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let recording = fs::read_to_string(source.with_extension("out")).unwrap();
        assert_eq!(stdout, recording, "{name} printed something else");
    }
}

/// `stdout` with each call site's path written with `/`, because the compiler
/// spells the path the way the host system does. Every other byte, line ends
/// included, stays as printed.
fn with_forward_slashes(stdout: &str) -> String {
    stdout
        .split_inclusive('\n')
        .map(|line| match line.strip_prefix("call site: ") {
            Some(site) => format!("call site: {}", site.replace('\\', "/")),
            None => line.to_owned(),
        })
        .collect()
}

#[test]
fn only_call_site_paths_change() {
    assert_eq!(
        with_forward_slashes("a\\b\r\ncall site: x\\y.rs:1:2\nend"),
        "a\\b\r\ncall site: x/y.rs:1:2\nend"
    );
}
