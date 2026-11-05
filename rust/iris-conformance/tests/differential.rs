use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gecode_iris_conformance::case::Case;
use gecode_iris_conformance::differential;
use tempfile::TempDir;

fn fake_driver(options: &str) -> String {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/differential-driver.py");
    format!(
        "python3 {} {options}",
        shell_words::quote(&script.to_string_lossy())
    )
}

fn differential(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_iris-conformance"))
        .arg("differential")
        .args(arguments)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn generation_is_deterministic_for_each_seed_and_index() {
    for (seed, index) in [(0, 0), (1, 7), (u64::MAX, u64::MAX)] {
        assert_eq!(
            differential::generate(seed, index),
            differential::generate(seed, index)
        );
    }
    assert_ne!(differential::generate(5, 0), differential::generate(5, 1));
    assert_ne!(differential::generate(5, 0), differential::generate(6, 0));
}

#[test]
fn agreeing_drivers_pass() {
    let driver = fake_driver("");
    let output = differential(&[
        "--driver", &driver, "--driver", &driver, "--seed", "3", "--cases", "4", "--jobs", "2",
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let text = stdout(&output);
    assert!(
        text.contains("Differential: seed=3 cases=4 comparisons=20 disagreements=0"),
        "{text}"
    );
    assert!(text.contains("strategies: generate=4"), "{text}");
    assert!(
        text.contains("requests: catalog=4 catalog-dzn=4 dzn=4 parse=4 read=4"),
        "{text}"
    );
}

#[test]
fn case_roots_seed_the_mutate_strategy() {
    let driver = fake_driver("");
    let roots = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/passing");
    let output = differential(&[
        "--driver",
        &driver,
        "--driver",
        &driver,
        "--seed",
        "1",
        "--cases",
        "12",
        &roots.to_string_lossy(),
    ]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("mutate="), "{}", stdout(&output));
}

#[test]
fn differing_drivers_fail_with_a_reduced_reproducer() {
    let keep = TempDir::new().unwrap();
    let plain = fake_driver("");
    let skewed = fake_driver("--skew");
    let arguments = [
        "--driver", &plain, "--driver", &skewed, "--seed", "11", "--cases", "50",
    ];
    let keep_path = keep.path().join("reproducer");
    let output =
        differential(&[&arguments[..], &["--keep", &keep_path.to_string_lossy()]].concat());
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = stdout(&output);
    assert!(text.contains("disagreements=1"), "{text}");
    assert!(text.contains("request=read"), "{text}");

    let original = fs::read_to_string(keep_path.join("original/spec.iris")).unwrap();
    let reduced = fs::read_to_string(keep_path.join("reduced/spec.iris")).unwrap();
    assert!(original.lines().count() > 1, "{original}");
    assert_eq!(reduced.lines().count(), 1, "{reduced}");
    assert!(reduced.contains(':'), "{reduced}");
    assert_eq!(fs::read(keep_path.join("reduced/input.txt")).unwrap(), b"");
    let drivers = fs::read_to_string(keep_path.join("reduced/drivers.txt")).unwrap();
    assert!(drivers.contains("--skew"), "{drivers}");
    assert!(drivers.contains("\"value\": 2"), "{drivers}");

    // The reproducer is a conformance case once an expectation is chosen.
    let reduced_case = keep_path.join("reduced");
    fs::write(
        reduced_case.join("expected.json"),
        r#"{"iris": "0.1", "entries": [{"name": "lines", "type": "int", "depth": 0, "value": 1}]}"#,
    )
    .unwrap();
    let case = Case::load(&reduced_case, "0.1").unwrap();
    assert_eq!(case.operation.as_str(), "read");
    let metadata = fs::read_to_string(reduced_case.join("case.toml")).unwrap();
    assert!(metadata.contains("seed=11 index="), "{metadata}");

    // The printed replay reproduces the failure alone.
    let start = replay_start(&text);
    let replay = differential(&[&arguments[..6], &["--start", &start, "--cases", "1"]].concat());
    assert_eq!(replay.status.code(), Some(1), "{replay:?}");
    assert!(
        stdout(&replay).contains(&format!("index={start} ")),
        "{}",
        stdout(&replay)
    );
}

fn replay_start(report: &str) -> String {
    let replay = report
        .lines()
        .find_map(|line| line.trim().strip_prefix("replay: "))
        .expect("a failing run prints its replay command");
    let words = shell_words::split(replay).unwrap();
    let position = words.iter().position(|word| word == "--start").unwrap();
    assert_eq!(words[position + 2..position + 4], ["--cases", "1"]);
    words[position + 1].clone()
}

#[test]
fn a_hard_failure_fails_the_run() {
    let plain = fake_driver("");
    let crashing = fake_driver("--exit 3");
    let output = differential(&[
        "--driver", &plain, "--driver", &crashing, "--seed", "2", "--cases", "1",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = stdout(&output);
    assert!(text.contains("hard failure: exit 3"), "{text}");
    assert!(text.contains("reduced spec:"), "{text}");
}

#[test]
fn usage_errors_exit_2() {
    let driver = fake_driver("");
    let one_driver = differential(&["--driver", &driver, "--cases", "1"]);
    assert_eq!(one_driver.status.code(), Some(2), "{one_driver:?}");
    let no_limit = differential(&["--driver", &driver, "--driver", &driver]);
    assert_eq!(no_limit.status.code(), Some(2), "{no_limit:?}");

    let keep = TempDir::new().unwrap();
    fs::write(keep.path().join("file"), "").unwrap();
    let full_keep = differential(&[
        "--driver",
        &driver,
        "--driver",
        &driver,
        "--cases",
        "1",
        "--keep",
        &PathBuf::from(keep.path()).to_string_lossy(),
    ]);
    assert_eq!(full_keep.status.code(), Some(2), "{full_keep:?}");
}

#[test]
fn the_replay_keeps_the_timeout() {
    let plain = fake_driver("");
    let slow = fake_driver("--sleep 0.5");
    let output = differential(&[
        "--driver",
        &plain,
        "--driver",
        &slow,
        "--seed",
        "4",
        "--cases",
        "1",
        "--timeout",
        "0.1",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let text = stdout(&output);
    let replay = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("replay: "))
        .expect("a failing run prints its replay command");
    let words = shell_words::split(replay).unwrap();
    assert_eq!(words[..2], ["iris-conformance", "differential"]);
    let arguments: Vec<&str> = words[2..].iter().map(String::as_str).collect();
    let replayed = differential(&arguments);
    assert_eq!(replayed.status.code(), Some(1), "{replayed:?}");
    assert!(
        stdout(&replayed).contains("disagreements=1"),
        "{}",
        stdout(&replayed)
    );
}
