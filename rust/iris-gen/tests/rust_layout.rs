//! Sweeps name and literal lengths across the widths where `rustfmt` changes
//! its layout, and requires it to leave every generated program as it is.
//!
//! The programs of a batch are modules of one root file, so a worker runs
//! `rustfmt` once. Each length is swept without gaps, because a width that
//! the backend gets wrong can be off by a single column.
use std::{
    fmt::Write as _,
    fs,
    num::NonZero,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
};

use gecode_iris_gen::{Language, builder_code};
use gecode_iris_sema::analyze;
use gecode_iris_syntax::{LanguageVersion, parse};

mod layout_forms;

use layout_forms::{FORMS, nests, spec};

/// The nesting depths to sweep, each with a length past the widest line there.
/// A deep line reaches 100 columns before its call reaches 60, which takes other
/// rules than a shallow one.
const SWEEPS: &[(usize, usize)] = &[(0, 104), (4, 88), (7, 76), (9, 68), (10, 64)];
/// Characters whose width in `rustfmt` is not their length in bytes: one column
/// for two bytes, two for three, and a letter with a mark of no width.
const NON_ASCII_TEXT: &[&str] = &["å", "漢", "e\u{301}"];
/// Lengths of that text, whose escapes take at least seven columns a character.
const NON_ASCII_LENGTHS: &[usize] = &[1, 7, 8, 13, 14, 15];

fn builder(source: &str) -> String {
    let parsed = parse(source.as_bytes(), "sweep.iris", LanguageVersion::V0_1)
        .unwrap_or_else(|error| panic!("{error}\n{source}"));
    let analysis = analyze(&parsed);
    builder_code(&parsed, &analysis, Language::Rust)
        .unwrap_or_else(|error| panic!("{error}\n{source}"))
}

/// Run `rustfmt --check` once over `programs`.
fn check(root: &Path, programs: &[PathBuf]) -> Output {
    let mut modules = String::new();
    for path in programs {
        let module = path.file_stem().unwrap().to_str().unwrap();
        writeln!(modules, "mod {module};").unwrap();
    }
    fs::write(root, modules).unwrap();
    Command::new("rustfmt")
        .args(["--check", "--edition", "2024"])
        .args(["--config", "reorder_modules=false"])
        .arg(root)
        .output()
        .expect("rustfmt runs")
}

fn fresh_directory(name: &str) -> PathBuf {
    let directory = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if directory.exists() {
        fs::remove_dir_all(&directory).unwrap();
    }
    fs::create_dir_all(&directory).unwrap();
    directory
}

/// Every program of the sweep, as a module name and its spec.
fn sweep() -> Vec<(String, String)> {
    let mut programs = Vec::new();
    for (title, form) in FORMS {
        let title = title.replace('-', "_");
        for &(depth, longest) in SWEEPS {
            if depth > 0 && !nests(form) {
                continue;
            }
            for length in 1..=longest {
                let source = spec(form, &"n".repeat(length), &"t".repeat(length), depth);
                programs.push((format!("{title}_{depth}_{length}"), source));
            }
        }
        if !form.contains("@t") {
            continue;
        }
        for (index, text) in NON_ASCII_TEXT.iter().enumerate() {
            for &length in NON_ASCII_LENGTHS {
                let source = spec(form, "n", &text.repeat(length), 0);
                programs.push((format!("{title}_text{index}_{length}"), source));
            }
        }
    }
    programs
}

/// Generate a batch of programs and check them with one run of `rustfmt`.
fn check_batch(directory: &Path, index: usize, batch: &[(String, String)]) -> Output {
    let mut paths = Vec::new();
    for (module, source) in batch {
        let code = builder(source);
        // Only ASCII has a length that is its width in `rustfmt`.
        assert!(code.is_ascii(), "{source}\n{code}");
        let path = directory.join(format!("{module}.rs"));
        fs::write(&path, code).unwrap();
        paths.push(path);
    }
    check(&directory.join(format!("batch_{index}.rs")), &paths)
}

#[test]
fn rustfmt_leaves_every_width_as_generated() {
    let directory = fresh_directory("layout-sweep");
    let programs = sweep();
    let workers = thread::available_parallelism().map_or(1, NonZero::get);
    let outputs: Vec<Output> = thread::scope(|scope| {
        let batches = programs.chunks(programs.len().div_ceil(workers));
        let handles: Vec<_> = batches
            .enumerate()
            .map(|(index, batch)| {
                let directory = &directory;
                scope.spawn(move || check_batch(directory, index, batch))
            })
            .collect();
        let joined = handles.into_iter().map(|handle| handle.join().unwrap());
        joined.collect()
    });

    let failed: Vec<&Output> = outputs
        .iter()
        .filter(|output| !output.status.success())
        .collect();
    let mut failures = String::new();
    for output in &failed {
        failures.push_str(&String::from_utf8_lossy(&output.stdout));
        failures.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    let different = failures.matches("Diff in ").count();
    println!(
        "Layout sweep: {} programs, {different} differences",
        programs.len()
    );
    let shown: String = failures.lines().take(60).collect::<Vec<_>>().join("\n");
    assert!(
        failed.is_empty(),
        "{} batches failed, {different} differences:\n{shown}",
        failed.len()
    );
}

/// A sweep that follows no module would pass whatever the backend does.
#[test]
fn a_batch_reports_a_program_that_rustfmt_would_change() {
    let directory = fresh_directory("layout-control");
    let (good, bad) = (directory.join("good.rs"), directory.join("bad.rs"));
    fs::write(&good, "pub fn f() {}\n").unwrap();
    fs::write(&bad, "pub fn  f() {}\n").unwrap();
    let root = directory.join("batch.rs");
    assert!(check(&root, std::slice::from_ref(&good)).status.success());
    assert!(!check(&root, &[good, bad]).status.success());
}
