//! Round-trips every corpus spec that `iris check` accepts through the Python
//! builder: generate the builder program, check it with `ruff format`, `ruff
//! check` and `ty`, run it, and compare the spec it prints with the original.
//! The layout forms and the limit specs of the Rust round trip go the same way.
//!
//! `IRIS_PYTHON` names an interpreter that has `gecode_iris`, `ruff` and `ty`,
//! such as `python/.venv/bin/python` after the `python/` setup in AGENTS.md.
mod round_trip;

use gecode_iris_cli::check_spec;
use gecode_iris_gen::{Language, builder_code};
use round_trip::{Tally, corpus, extra_specs, limit_specs};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Imports each builder module named on standard input and records what its
/// builder prints, or why it printed nothing.
const RUNNER: &str = r#"import importlib.util
import pathlib
import sys

source, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
for name in sys.stdin.read().split():
    try:
        spec = importlib.util.spec_from_file_location(name, source / f"{name}.py")
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        text, extension = module.builder().source(), "iris"
    except Exception as error:  # noqa: BLE001
        text, extension = f"{type(error).__name__}: {error}", "error"
    with open(out / f"{name}.{extension}", "w", encoding="utf-8", newline="") as file:
        file.write(text)
"#;

/// The lint rules that the repository's Python adds to ruff's defaults.
const RUFF_RULES: &str = "B,I,UP";

#[test]
#[ignore = "needs IRIS_PYTHON, an interpreter with gecode_iris, ruff and ty"]
fn accepted_specs_round_trip_through_the_python_builder() {
    let python = std::env::var_os("IRIS_PYTHON")
        .expect("IRIS_PYTHON names an interpreter with gecode_iris, ruff and ty");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let specs = corpus(&repo);
    let project = Project::new(python);

    // Each builder with the spec it must print.
    let mut generated: Vec<(String, Vec<u8>, String)> = Vec::new();
    let mut skipped = 0;
    for path in &specs {
        let bytes = fs::read(path).unwrap();
        let Ok(checked) = check_spec(&bytes, &path.to_string_lossy()) else {
            skipped += 1;
            continue;
        };
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Python)
            .expect("a checked spec lowers");
        let module = format!("spec_{}", generated.len());
        fs::write(project.source(&module), code).unwrap();
        generated.push((module, bytes, path.display().to_string()));
    }
    let compared = generated.len();
    for (module, text) in extra_specs() {
        project.write(&module, &text);
        generated.push((
            module.clone(),
            text.into_bytes(),
            format!("extra spec {module}"),
        ));
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

    project.check();
    let names: Vec<&str> = generated
        .iter()
        .map(|(module, ..)| module.as_str())
        .collect();
    project.run(&names);

    let out = project.root.join("out");
    let mut differences = Vec::new();
    for (module, original, title) in &generated {
        if let Err(message) = round_trip::round_trip(&out, original, module) {
            differences.push(format!("{title} ({module}.py): {message}"));
        }
    }
    let tally = Tally {
        compared,
        skipped,
        extra,
        corpus: specs.len(),
    };
    tally.report("Python builder", &differences);
}

/// A directory of generated modules, outside the repository.
struct Project {
    root: PathBuf,
    python: OsString,
}

impl Project {
    fn new(python: OsString) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("python-round-trip");
        // Generated files of an earlier run must not survive.
        for directory in ["src", "out"] {
            let path = root.join(directory);
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path).unwrap();
        }
        fs::write(root.join("run_all.py"), RUNNER).unwrap();
        Self { root, python }
    }

    fn source(&self, module: &str) -> PathBuf {
        self.root.join("src").join(format!("{module}.py"))
    }

    /// The builder of a spec beyond the corpus, which must pass `iris check`.
    fn write(&self, module: &str, text: &str) {
        let checked = check_spec(text.as_bytes(), module)
            .unwrap_or_else(|error| panic!("{module}: {error}\n{text}"));
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Python).unwrap();
        fs::write(self.source(module), code).unwrap();
    }

    fn python(&self) -> Command {
        let mut command = Command::new(&self.python);
        command.current_dir(&self.root);
        command
    }

    /// Format, lint and type-check every module with the settings that the
    /// generated code promises: ruff's defaults for Python 3.10 with the
    /// repository's extra rules, and ty for Python 3.10.
    fn check(&self) {
        let ruff = ["-m", "ruff"];
        let target = ["--isolated", "--target-version", "py310"];
        let mut format = self.python();
        format
            .args(ruff)
            .args(["format", "--check"])
            .args(target)
            .arg("src");
        expect_success(format, "ruff format --check");
        let mut lint = self.python();
        lint.args(ruff).arg("check").args(target);
        lint.args(["--extend-select", RUFF_RULES, "src"]);
        expect_success(lint, "ruff check");
        let mut types = self.python();
        types.args(["-m", "ty", "check", "--python-version", "3.10", "--python"]);
        types.arg(&self.python).arg("src");
        expect_success(types, "ty check");
    }

    fn run(&self, names: &[&str]) {
        let mut run = self.python();
        run.args(["-I", "run_all.py", "src", "out"]);
        run.stdin(std::process::Stdio::piped());
        let mut child = run
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        {
            use std::io::Write as _;
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(names.join("\n").as_bytes()).unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "the runner failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn expect_success(mut command: Command, name: &str) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
