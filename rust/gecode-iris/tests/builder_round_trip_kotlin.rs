//! Round-trips every corpus spec that `iris check` accepts through the Kotlin
//! builder: generate the builder program, check that ktfmt would leave it as
//! it is, compile it with the flags of `java/`, warnings as errors among them,
//! run it, and compare the spec it prints with the original. The layout forms
//! and the limit specs of the Rust round trip go the same way.
//!
//! The programs go to `java/iris-kotlin-round-trip` through Gradle
//! properties, so the Gradle wrapper of `java/` runs everything that is
//! Kotlin: Spotless with the build's ktfmt, detekt with the build's rules,
//! one compilation of every program with the runner, and the runner, which
//! writes what each program prints. Each program is the generated text in a
//! package of its own, `rt.specN`, which detekt's package naming accepts.
//! Gradle needs Java 27, on `PATH` or in `JAVA_HOME`.
mod round_trip;

use gecode_iris_cli::check_spec;
use gecode_iris_gen::{Language, builder_code};
use round_trip::{Tally, corpus, extra_specs, limit_specs};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

#[test]
#[ignore = "needs Java 27 for the Gradle build of java/"]
fn accepted_specs_round_trip_through_the_kotlin_builder() {
    let started = Instant::now();
    let repo = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let specs = corpus(&repo);
    let project = Project::new();

    // Each program with the spec it must print.
    let mut generated: Vec<(String, Vec<u8>, String)> = Vec::new();
    let mut skipped = 0;
    for path in &specs {
        let bytes = fs::read(path).unwrap();
        let Ok(checked) = check_spec(&bytes, &path.to_string_lossy()) else {
            skipped += 1;
            continue;
        };
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Kotlin)
            .expect("a checked spec lowers");
        let program = program_name(generated.len());
        project.save(&program, &code);
        generated.push((program, bytes, path.display().to_string()));
    }
    let compared = generated.len();
    for (name, text) in extra_specs() {
        let program = program_name(generated.len());
        project.write(&program, &name, &text);
        generated.push((program, text.into_bytes(), format!("extra spec {name}")));
    }
    for (name, text) in kotlin_specs() {
        let program = program_name(generated.len());
        project.write(&program, &name, &text);
        generated.push((program, text.into_bytes(), format!("Kotlin spec {name}")));
    }
    let extra = generated.len() - compared;
    for (name, text) in limit_specs() {
        let program = program_name(generated.len());
        project.write(&program, name, &text);
        generated.push((program, text.into_bytes(), format!("limit spec {name}")));
    }
    println!(
        "generated {} programs in {:.1?}",
        generated.len(),
        started.elapsed()
    );

    let step = Instant::now();
    project.run(&repo);
    println!(
        "ktfmt and detekt clean, compiled and ran in {:.1?}",
        step.elapsed()
    );

    let out = project.root.join("out");
    let mut differences = Vec::new();
    for (name, original, title) in &generated {
        if let Err(message) = round_trip::round_trip(&out, original, name) {
            differences.push(format!("{title} ({name}): {message}"));
        }
    }
    let tally = Tally {
        compared,
        skipped,
        extra,
        corpus: specs.len(),
    };
    println!("wall time {:.1?}", started.elapsed());
    tally.report("Kotlin builder", &differences);
}

/// The name of the program at `index`, which is also its package's last
/// segment and its output's file stem.
fn program_name(index: usize) -> String {
    format!("spec{index}")
}

/// Specs for what only Kotlin needs: names that Kotlin reserves, that the
/// builder's scopes or functions have, or that meet in lower camel case, as
/// values, types and comprehension variables, also in a block deep enough
/// for a function of its own and in a body that needs one; strings with
/// templates; and more section clauses than one line holds.
fn kotlin_specs() -> Vec<(String, String)> {
    let names = [
        "val", "fun", "object", "is", "record", "session", "depth", "class", "read", "int", "lit",
        "data", "e12", "T1", "n_jobs", "nJobs", "N_JOBS",
    ];
    let mut text: String = names.iter().map(|name| format!("{name}: int\n")).collect();
    text.push_str("list[2]: int\ntype in = 1..9\ntype when = in\n");
    let body = "let sum = sum(it in list: sum(field in list: it + field + val * record))\n\
                let check = defined(typeof) && count(list) > 0\n\
                z: when\n\
                check \"$x\" != \"${y}\" \"costs $5 ${it}\"\n";
    text.push_str(&format!(
        "{}{body}{}",
        "repeat 1 {\n".repeat(9),
        "}\n".repeat(9)
    ));
    let clauses = "sections until \"X\" store a skip other until \"Y\" store b until \"Z\" \
                   until \"a very long terminator that fills a line of its own\" \
                   {\"A\" optional}\n";
    vec![
        ("kotlin_names".to_owned(), text),
        ("section_clauses".to_owned(), clauses.to_owned()),
    ]
}

/// The generated programs and what they print, outside the repository.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("kotlin-round-trip");
        // Files of an earlier run must not survive.
        for directory in ["src", "out"] {
            let path = root.join(directory);
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path).unwrap();
        }
        Self { root }
    }

    /// Save the program `code` as the file `rt/NAME/SpecBuilder.kt` of the
    /// package `rt.NAME`.
    fn save(&self, name: &str, code: &str) {
        let directory = self.root.join("src/rt").join(name);
        fs::create_dir_all(&directory).unwrap();
        let source = format!("package rt.{name}\n\n{code}");
        fs::write(directory.join("SpecBuilder.kt"), source).unwrap();
    }

    /// The program `program` of the spec `name` beyond the corpus, which
    /// must pass `iris check`.
    fn write(&self, program: &str, name: &str, text: &str) {
        let checked = check_spec(text.as_bytes(), name)
            .unwrap_or_else(|error| panic!("{name}: {error}\n{text}"));
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Kotlin).unwrap();
        self.save(program, &code);
    }

    /// Check with ktfmt and detekt, compile and run every program through
    /// `java/iris-kotlin-round-trip`. Gradle's output goes to `gradle.log`,
    /// which a failure prints.
    fn run(&self, repo: &Path) {
        let java = repo.join("java");
        let wrapper = if cfg!(windows) {
            java.join("gradlew.bat")
        } else {
            java.join("gradlew")
        };
        let log = self.root.join("gradle.log");
        let output = Command::new(wrapper)
            .current_dir(&java)
            .args(["--console=plain", ":iris-kotlin-round-trip:roundTrip"])
            .arg(format!(
                "-PirisRoundTripSources={}",
                self.root.join("src").display()
            ))
            .arg(format!(
                "-PirisRoundTripOut={}",
                self.root.join("out").display()
            ))
            .output()
            .unwrap();
        let mut text = output.stdout;
        text.extend_from_slice(&output.stderr);
        fs::write(&log, &text).unwrap();
        assert!(
            output.status.success(),
            "Gradle failed (log in {}):\n{}",
            log.display(),
            String::from_utf8_lossy(&text)
        );
    }
}
