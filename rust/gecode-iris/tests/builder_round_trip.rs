//! Round-trips every corpus spec that `iris check` accepts through the Rust
//! builder: generate the builder program, format-check it, lint it, run it, and
//! compare the spec it prints with the original. Specs written for the layout
//! and lint rules of the backend go the same way.
mod round_trip;

use gecode_iris_cli::check_spec;
use gecode_iris_gen::{Language, builder_code};
use round_trip::{Tally, corpus, extra_specs, limit_specs};
use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// A corpus spec and the module that holds its builder.
struct Generated {
    path: PathBuf,
    module: String,
}

#[test]
fn accepted_specs_round_trip_through_the_rust_builder() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let specs = corpus(&repo);
    let project = Project::new();

    let mut generated = Vec::new();
    let mut skipped = 0;
    for path in &specs {
        let bytes = fs::read(path).unwrap();
        let Ok(checked) = check_spec(&bytes, &path.to_string_lossy()) else {
            skipped += 1;
            continue;
        };
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Rust)
            .expect("a checked spec lowers");
        assert!(
            !code.contains("rustfmt::skip"),
            "{} is not deep, so its builder must be formatted",
            path.display()
        );
        let module = format!("spec_{}", generated.len());
        fs::write(project.source(&module), code).unwrap();
        generated.push(Generated {
            path: path.clone(),
            module,
        });
    }
    let compared = generated.len();
    project.write_extra_specs(&mut generated);
    let extra = generated.len() - compared;
    let limits = project.write_limit_specs();
    project.write_main(&generated);
    project.check_formatting(&generated, &limits);
    project.lint();
    project.run(&limits);

    let out = project.root.join("out");
    let mut differences = Vec::new();
    for spec in &generated {
        let original = fs::read(&spec.path).unwrap();
        if let Err(message) = round_trip::round_trip(&out, &original, &spec.module) {
            differences.push(format!("{}: {message}", spec.path.display()));
        }
    }
    for (name, text) in &limits {
        if let Err(message) = round_trip::round_trip(&out, text.as_bytes(), name) {
            differences.push(format!("limit spec {name}: {message}"));
        }
    }
    let tally = Tally {
        compared,
        skipped,
        extra,
        corpus: specs.len(),
    };
    tally.report("Builder", &differences);
}

/// Return a command that runs rustfmt. It recurses on the main thread, and the
/// deepest limit specs overflow the default stack on macOS, so it gets the
/// largest stack that the system allows.
fn rustfmt() -> Command {
    if cfg!(unix) {
        let mut command = Command::new("sh");
        let script = r#"ulimit -s "$(ulimit -Hs)"; exec rustfmt "$@""#;
        command.args(["-c", script, "rustfmt"]);
        command
    } else {
        Command::new("rustfmt")
    }
}

/// A Cargo project outside the workspace that holds the generated programs.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new() -> Self {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("builder-round-trip");
        // Generated files of an earlier run must not survive; the target stays.
        for directory in ["src", "out", "extra"] {
            let path = root.join(directory);
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path).unwrap();
        }
        let dependency = fs::canonicalize(manifest).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"builder-round-trip\"\nversion = \"0.0.0\"\n\
                 edition = \"2024\"\npublish = false\n\n[dependencies]\n\
                 gecode-iris = {{ path = {:?} }}\n\n[workspace]\n",
                dependency.display().to_string()
            ),
        )
        .unwrap();
        fs::copy(manifest.join("../Cargo.lock"), root.join("Cargo.lock")).unwrap();
        Self { root }
    }

    /// One module per extra spec, whose `eq_op` allowance must be in use.
    fn write_extra_specs(&self, generated: &mut Vec<Generated>) {
        let directory = self.root.join("extra");
        fs::create_dir_all(&directory).unwrap();
        for (module, text) in extra_specs() {
            let checked = check_spec(text.as_bytes(), &module)
                .unwrap_or_else(|error| panic!("extra spec {module}: {error}\n{text}"));
            let code = builder_code(&checked.parsed, &checked.analysis, Language::Rust).unwrap();
            let code = code.replace("#[allow(clippy::eq_op)]", "#[expect(clippy::eq_op)]");
            fs::write(self.source(&module), code).unwrap();
            let path = directory.join(format!("{module}.iris"));
            fs::write(&path, text).unwrap();
            generated.push(Generated { path, module });
        }
    }

    /// One program per limit spec, because a deep one needs a crate attribute.
    fn write_limit_specs(&self) -> Vec<(&'static str, String)> {
        let directory = self.root.join("src/bin");
        fs::create_dir_all(&directory).unwrap();
        let specs = limit_specs();
        for (name, text) in &specs {
            let checked = check_spec(text.as_bytes(), name)
                .unwrap_or_else(|error| panic!("limit spec {name}: {error}"));
            let code = builder_code(&checked.parsed, &checked.analysis, Language::Rust).unwrap();
            fs::write(directory.join(format!("{name}.rs")), code).unwrap();
        }
        specs
    }

    fn source(&self, module: &str) -> PathBuf {
        self.root.join("src").join(format!("{module}.rs"))
    }

    /// One program that runs every builder and records what each one prints.
    fn write_main(&self, specs: &[Generated]) {
        let mut main = String::from(
            "#![deny(warnings)]\n#![allow(dead_code)]\n\nuse gecode_iris::build::SpecBuilder;\n\n",
        );
        for spec in specs {
            writeln!(main, "mod {};", spec.module).unwrap();
        }
        main.push_str("\ntype Builder = fn() -> SpecBuilder;\n\n");
        main.push_str("const BUILDERS: &[(&str, Builder)] = &[\n");
        for spec in specs {
            writeln!(main, "    ({0:?}, {0}::builder),", spec.module).unwrap();
        }
        main.push_str(
            "];\n\nfn main() {\n    \
             std::panic::set_hook(Box::new(|_| {}));\n    \
             for (name, builder) in BUILDERS {\n        \
             let result = std::panic::catch_unwind(|| builder().source());\n        \
             let (extension, text) = match result {\n            \
             Ok(Ok(source)) => (\"iris\", source),\n            \
             Ok(Err(error)) => (\"error\", error.to_string()),\n            \
             Err(_) => (\"error\", \"the builder panicked\".to_owned()),\n        \
             };\n        \
             std::fs::write(format!(\"out/{name}.{extension}\"), text).unwrap();\n    \
             }\n}\n",
        );
        fs::write(self.root.join("src/main.rs"), main).unwrap();
    }

    fn check_formatting(&self, specs: &[Generated], limits: &[(&str, String)]) {
        let mut rustfmt = rustfmt();
        rustfmt.args(["--check", "--edition", "2024"]);
        rustfmt.args(specs.iter().map(|spec| self.source(&spec.module)));
        rustfmt.args(limits.iter().map(|(name, _)| self.limit_source(name)));
        self.expect_success(rustfmt, "rustfmt");
    }

    fn limit_source(&self, name: &str) -> PathBuf {
        self.root.join("src/bin").join(format!("{name}.rs"))
    }

    fn lint(&self) {
        let mut clippy = cargo();
        clippy.args(["clippy", "--offline", "--", "-D", "warnings"]);
        self.expect_success(clippy, "cargo clippy");
    }

    fn run(&self, limits: &[(&str, String)]) {
        let mut run = cargo();
        run.args(["run", "--offline", "--bin", "builder-round-trip"]);
        self.expect_success(run, "cargo run");
        for (name, _) in limits {
            let mut run = cargo();
            run.args(["run", "--quiet", "--offline", "--bin", name]);
            let output = run.current_dir(&self.root).output().unwrap();
            let path = self.root.join("out").join(name);
            let (extension, text) = if output.status.success() {
                ("iris", output.stdout)
            } else {
                ("error", output.stderr)
            };
            fs::write(path.with_extension(extension), text).unwrap();
        }
    }

    fn expect_success(&self, mut command: Command, name: &str) {
        let output = command.current_dir(&self.root).output().unwrap();
        assert!(
            output.status.success(),
            "{name} failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}
