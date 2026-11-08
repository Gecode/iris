//! Generates a Python view of every format in `formats/`, plus specs that need
//! dynamic handles or unusual identifiers, and holds them to what they promise:
//!
//! - every view passes `ruff format --check`, `ruff check` and `ty check` as
//!   generated, with the settings that generated builders promise;
//! - pyright in strict mode and mypy with `--strict` accept the views and a file
//!   that pins, with `assert_type`, the type of `data[handle]` for each handle;
//! - one Python program reads each case and compares what every handle gives
//!   with untyped access, so a handle that promises the wrong value type or depth
//!   fails, since `data[handle]` checks it.
//!
//! `IRIS_PYTHON` names an interpreter that has `gecode_iris`, ruff, ty, pyright,
//! mypy and typing-extensions, such as `python/.venv/bin/python` after the
//! `python/` setup in AGENTS.md.
use gecode_iris_cli::check_spec;
use gecode_iris_conformance::canonical::Outcome;
use gecode_iris_conformance::case::{Case, Operation, discover};
use gecode_iris_gen::{
    Language,
    view::{self, Access, Leaf, View},
    view_code,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// A view module and the handles it declares.
struct Module {
    name: String,
    view: View,
}

/// One read that the program compares.
struct Read {
    module: String,
    spec: Vec<u8>,
    input: Vec<u8>,
    params: BTreeMap<String, String>,
}

/// Specs that are not formats, with an input. Each exercises a part of the
/// handle list: a name without one shape, and names Python cannot spell or
/// that would hide what the module itself uses, such as `int` and `ib`.
const SPECS: &[(&str, &str, &str)] = &[
    (
        "redefined",
        "x:int let x=\"s\" y:int y:float z:word",
        "1 2 2.5 t",
    ),
    (
        "open_shape",
        "n:int param p a[1]:int param q=a repeat n {let r=a}",
        "0 7",
    ),
    ("optional", "x:int if x {y:float} u:num", "0 2.5"),
    (
        "spellings",
        "def:int None:int int:int list:int str:int float:int ib:int b:int l:int match:int \
         type:int _:int __init__:int e1:int declared:int a_b:int aB:int",
        "1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17",
    ),
];

/// Reaches beyond the widest identifier and past a full line of the string.
const LONGEST_NAME: usize = 130;

/// A depth above the deepest typed handle, which only a dynamic handle reads.
fn deep_spec() -> String {
    "repeat 1 {".repeat(9) + "v:int" + &"}".repeat(9)
}

/// Names of every length around the widths where a declaration changes layout,
/// for each kind of handle type, the widest among them.
fn layout_spec() -> String {
    let mut spec = String::from("c:int\n");
    for length in 1..=LONGEST_NAME {
        let name = "n".repeat(length);
        writeln!(spec, "{name}_i: int").unwrap();
        writeln!(spec, "{name}_f[1][1][1]: float").unwrap();
        writeln!(spec, "if c {{\n{name}_s: word\n}}").unwrap();
        writeln!(spec, "if c {{\n{name}_d[1][1][1][1][1][1][1][1]: num\n}}").unwrap();
    }
    spec
}

#[test]
#[ignore = "needs IRIS_PYTHON, an interpreter with gecode_iris, ruff, ty, pyright and mypy"]
fn python_views_type_check_and_read_the_same_data_as_untyped_access() {
    let python = std::env::var_os("IRIS_PYTHON")
        .expect("IRIS_PYTHON names an interpreter with gecode_iris, ruff, ty, pyright and mypy");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let project = Project::new(python);
    let mut modules = Vec::new();
    let mut reads = Vec::new();
    for (name, spec) in format_specs(&repo) {
        modules.push(project.write_view(&name, &spec));
        reads.extend(format_reads(&repo, &name, &spec));
    }
    assert_eq!(modules.len(), 6, "the format library has six formats");
    assert!(reads.len() >= 6, "too few cases: {}", reads.len());
    for (name, spec, input) in SPECS {
        modules.push(project.write_view(name, spec.as_bytes()));
        let params = if *name == "open_shape" {
            BTreeMap::from([("p".to_owned(), "1".to_owned())])
        } else {
            BTreeMap::new()
        };
        reads.push(Read {
            module: (*name).to_owned(),
            spec: spec.as_bytes().to_vec(),
            input: input.as_bytes().to_vec(),
            params,
        });
    }
    for (name, spec, input) in [
        ("deep", deep_spec(), "1".to_owned()),
        (
            "widths",
            layout_spec(),
            format!("1 {}", "1 1.5 w 1.5 ".repeat(LONGEST_NAME)),
        ),
    ] {
        modules.push(project.write_view(name, spec.as_bytes()));
        reads.push(Read {
            module: name.to_owned(),
            spec: spec.into_bytes(),
            input: input.into_bytes(),
            params: BTreeMap::new(),
        });
    }

    project.check_style(&modules);
    project.write_typed_access(&modules);
    project.check_types(&modules);
    let summary = project.compare(&reads);
    println!("Python views: {} modules, {summary}", modules.len());
    assert!(
        summary.starts_with(&format!("{} reads", reads.len())),
        "{summary}"
    );
}

/// Each `<format>/<format>.iris`, by format name.
fn format_specs(repo: &Path) -> Vec<(String, Vec<u8>)> {
    let mut specs = Vec::new();
    for entry in fs::read_dir(repo.join("formats")).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let spec = fs::read(path.join(format!("{name}.iris"))).unwrap();
        specs.push((name.replace('-', "_"), spec));
    }
    specs.sort();
    specs
}

/// The reads of a format's cases that succeed.
fn format_reads(repo: &Path, module: &str, spec: &[u8]) -> Vec<Read> {
    let directory = repo.join("formats").join(module.replace('_', "-"));
    let mut reads = Vec::new();
    for path in discover(&[directory], None).unwrap() {
        let case = Case::load(&path, "0.1").unwrap();
        if case.operation != Operation::Read || !matches!(case.expected, Outcome::Data(_)) {
            continue;
        }
        let params = case
            .params
            .as_deref()
            .map_or_else(BTreeMap::new, |text| serde_json::from_str(text).unwrap());
        reads.push(Read {
            module: module.to_owned(),
            spec: spec.to_vec(),
            input: fs::read(case.input.unwrap()).unwrap(),
            params,
        });
    }
    reads
}

/// What `data[handle]` gives for a handle of `access`, as a type that
/// `assert_type` compares.
fn value_type(access: Access) -> String {
    let Access::Typed {
        value,
        depth,
        optional,
    } = access
    else {
        return "iris.Value | None".into();
    };
    // The printer gives a deeper entry a dynamic handle.
    if depth > 8 {
        return "iris.Value | None".into();
    }
    let leaf = match value {
        Leaf::Int => "int",
        Leaf::Float => "float",
        Leaf::Str => "str",
        Leaf::Num => "int | float",
    };
    let nested = format!("{}{leaf}{}", "list[".repeat(depth), "]".repeat(depth));
    if optional {
        format!("{nested} | None")
    } else {
        nested
    }
}

/// The identifiers of a generated module with the handles' access, by
/// position: the printer declares one handle per entry, in order.
fn declared_handles(source: &str, view: &View) -> Vec<(String, Access)> {
    let identifiers: Vec<_> = source
        .lines()
        .filter_map(|line| line.split_once(" = ib.declared("))
        .map(|(identifier, _)| identifier.to_owned())
        .collect();
    assert_eq!(identifiers.len(), view.handles.len());
    let accesses = view.handles.iter().map(|handle| handle.access);
    identifiers.into_iter().zip(accesses).collect()
}

/// Imports each view named in the manifest and reads its cases, comparing
/// every handle with untyped access, and checks that every entry has a handle.
const COMPARE: &str = r#"import importlib
import json
import pathlib
import sys

import gecode_iris as iris
import gecode_iris.build as ib

root = pathlib.Path(sys.argv[1])
sys.path.insert(0, str(root / "src"))
compared = 0
reads = json.loads((root / "manifest.json").read_text(encoding="utf-8"))
for number, read in enumerate(reads):
    module = importlib.import_module(read["module"])
    spec = iris.Spec.parse((root / f"reads/{number}.iris").read_bytes())
    data = spec.read((root / f"reads/{number}.txt").read_bytes(), params=read["params"])
    handles = [value for value in vars(module).values() if isinstance(value, ib.Field)]
    names = {handle.name for handle in handles}
    for entry in data.entries:
        assert entry.name in names, f"{read['module']}: entry {entry.name!r} has no handle"
    for handle in handles:
        untyped = data[handle.name] if handle.name in data else None
        typed = data[handle]
        assert typed == untyped, f"{read['module']}.{handle.name}: {typed!r} != {untyped!r}"
        compared += 1
print(f"{len(reads)} reads, {compared} entries compared")
"#;

/// A directory of generated views, outside the repository.
struct Project {
    root: PathBuf,
    python: OsString,
}

impl Project {
    fn new(python: OsString) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("python-views");
        // Files of an earlier run must not survive.
        for directory in ["src", "reads"] {
            let path = root.join(directory);
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path).unwrap();
        }
        fs::write(root.join("compare.py"), COMPARE).unwrap();
        fs::write(
            root.join("pyrightconfig.json"),
            r#"{"typeCheckingMode": "strict", "pythonVersion": "3.10", "executionEnvironments": [{"root": "src"}]}"#,
        )
        .unwrap();
        Self { root, python }
    }

    fn source(&self, module: &str) -> PathBuf {
        self.root.join("src").join(format!("{module}.py"))
    }

    fn write_view(&self, name: &str, spec: &[u8]) -> Module {
        let checked = check_spec(spec, name).unwrap();
        let code = view_code(&checked.parsed, &checked.analysis, Language::Python).unwrap();
        fs::write(self.source(name), code).unwrap();
        Module {
            name: name.to_owned(),
            view: view::view(&checked.analysis).unwrap(),
        }
    }

    /// A module that pins the type of `data[handle]` for every handle.
    fn write_typed_access(&self, modules: &[Module]) {
        let mut text = String::from(
            "from typing_extensions import assert_type\n\nimport gecode_iris as iris\n\n",
        );
        for module in modules {
            writeln!(text, "import {}", module.name).unwrap();
        }
        for module in modules {
            let name = &module.name;
            writeln!(text, "\n\ndef check_{name}(data: iris.Data) -> None:").unwrap();
            let source = fs::read_to_string(self.source(name)).unwrap();
            for (identifier, access) in declared_handles(&source, &module.view) {
                let expected = value_type(access);
                writeln!(
                    text,
                    "    assert_type(data[{name}.{identifier}], {expected})"
                )
                .unwrap();
            }
            if module.view.handles.is_empty() {
                text.push_str("    pass\n");
            }
        }
        fs::write(self.source("typed_access"), text).unwrap();
    }

    fn views(&self, modules: &[Module]) -> Vec<PathBuf> {
        modules
            .iter()
            .map(|module| self.source(&module.name))
            .collect()
    }

    fn python(&self) -> Command {
        let mut command = Command::new(&self.python);
        command.current_dir(&self.root);
        command
    }

    /// Format, lint and type-check every view with the settings that generated
    /// code promises: ruff's defaults for Python 3.10 with the repository's
    /// extra rules, and ty for Python 3.10.
    fn check_style(&self, modules: &[Module]) {
        let target = ["--isolated", "--target-version", "py310"];
        let mut format = self.python();
        format
            .args(["-m", "ruff", "format", "--check"])
            .args(target);
        format.args(self.views(modules));
        expect_success(format, "ruff format --check");
        let mut lint = self.python();
        lint.args(["-m", "ruff", "check", "--extend-select", "B,I,UP"])
            .args(target);
        lint.args(self.views(modules));
        expect_success(lint, "ruff check");
        let mut types = self.python();
        types.args(["-m", "ty", "check", "--python-version", "3.10", "--python"]);
        types.arg(&self.python).args(self.views(modules));
        expect_success(types, "ty check");
    }

    /// pyright and mypy, both strict, on the views and the typed accesses.
    fn check_types(&self, modules: &[Module]) {
        let mut files = self.views(modules);
        files.push(self.source("typed_access"));
        let mut pyright = self.python();
        pyright.args(["-m", "pyright", "--pythonpath"]);
        pyright
            .arg(&self.python)
            .arg("-p")
            .arg(&self.root)
            .args(&files);
        expect_success(pyright, "pyright");
        let mut mypy = self.python();
        mypy.args(["-m", "mypy", "--strict", "--python-version", "3.10"]);
        mypy.current_dir(self.root.join("src")).args(&files);
        expect_success(mypy, "mypy");
    }

    /// Runs the comparison program and returns its summary.
    fn compare(&self, reads: &[Read]) -> String {
        let mut manifest = Vec::new();
        for (number, read) in reads.iter().enumerate() {
            fs::write(self.root.join(format!("reads/{number}.iris")), &read.spec).unwrap();
            fs::write(self.root.join(format!("reads/{number}.txt")), &read.input).unwrap();
            manifest.push(serde_json::json!({
                "module": read.module,
                "params": read.params,
            }));
        }
        let manifest = serde_json::to_string(&manifest).unwrap();
        fs::write(self.root.join("manifest.json"), manifest).unwrap();
        let mut run = self.python();
        run.args(["-I", "compare.py"]).arg(&self.root);
        expect_success(run, "the comparison")
    }
}

fn expect_success(mut command: Command, name: &str) -> String {
    let output = command.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{name} failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}
