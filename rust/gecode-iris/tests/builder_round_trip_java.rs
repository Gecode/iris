//! Round-trips every corpus spec that `iris check` accepts through the Java
//! builder: generate the builder program, check that google-java-format would
//! leave it as it is, compile it with the flags of `java/`, Error Prone among
//! them, run it, and compare the spec it prints with the original. The layout
//! forms and the limit specs of the Rust round trip go the same way.
//!
//! The programs go to `java/iris-round-trip` through Gradle properties, so
//! the Gradle wrapper of `java/` runs everything that is Java: Spotless with
//! the build's google-java-format, one compilation of every program with the
//! runner, and the runner, which writes what each program prints. Each program
//! is the generated text in a package of its own. Gradle needs Java 27, on
//! `PATH` or in `JAVA_HOME`.
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
fn accepted_specs_round_trip_through_the_java_builder() {
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
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Java)
            .expect("a checked spec lowers");
        let name = format!("spec_{}", generated.len());
        project.save(&name, &code);
        generated.push((name, bytes, path.display().to_string()));
    }
    let compared = generated.len();
    for (name, text) in extra_specs() {
        project.write(&name, &text);
        generated.push((
            name.clone(),
            text.into_bytes(),
            format!("extra spec {name}"),
        ));
    }
    for (name, text) in java_specs() {
        project.write(&name, &text);
        generated.push((name.clone(), text.into_bytes(), format!("Java spec {name}")));
    }
    let extra = generated.len() - compared;
    for (name, text) in limit_specs() {
        project.write(name, &text);
        generated.push((
            name.to_owned(),
            text.into_bytes(),
            format!("limit spec {name}"),
        ));
    }
    println!(
        "generated {} programs in {:.1?}",
        generated.len(),
        started.elapsed()
    );

    let step = Instant::now();
    project.run(&repo);
    println!(
        "google-java-format clean, compiled and ran in {:.1?}",
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
    tally.report("Java builder", &differences);
}

/// Specs for what only Java needs: names that Java reserves or that meet in
/// lower camel case, as values, types and comprehension variables, also in a
/// block deep enough for a method of its own, and more section clauses than
/// `sections` takes one by one.
fn java_specs() -> Vec<(String, String)> {
    let names = [
        "var", "record", "yield", "class", "this", "args", "e12", "n_jobs", "nJobs", "N_JOBS",
    ];
    let mut text: String = names.iter().map(|name| format!("{name}: int\n")).collect();
    text.push_str("list[2]: int\ntype new = 1..9\ntype final = new\n");
    let body = "let when = sum(int in list: sum(long in list: int + long + var))\n\
                let module = defined(goto) && count(list) > 0\n\
                z: final\n";
    text.push_str(&format!(
        "{}{body}{}",
        "repeat 1 {\n".repeat(8),
        "}\n".repeat(8)
    ));
    let clauses = "sections until \"X\" store a skip other until \"Y\" store b until \"Z\" \
                   {\"A\" optional}\n";
    vec![
        ("java_names".to_owned(), text),
        ("section_clauses".to_owned(), clauses.to_owned()),
    ]
}

/// The generated programs and what they print, outside the repository.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("java-round-trip");
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

    /// Save the program `code` as the class `rt.NAME.SpecBuilder`.
    fn save(&self, name: &str, code: &str) {
        let directory = self.root.join("src/rt").join(name);
        fs::create_dir_all(&directory).unwrap();
        let source = format!("package rt.{name};\n\n{code}");
        fs::write(directory.join("SpecBuilder.java"), source).unwrap();
    }

    /// The program of a spec beyond the corpus, which must pass `iris check`.
    fn write(&self, name: &str, text: &str) {
        let checked = check_spec(text.as_bytes(), name)
            .unwrap_or_else(|error| panic!("{name}: {error}\n{text}"));
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Java).unwrap();
        self.save(name, &code);
    }

    /// Check, compile and run every program through `java/iris-round-trip`.
    /// Gradle's output goes to `gradle.log`, which a failure prints.
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
            .args(["--console=plain", ":iris-round-trip:roundTrip"])
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
