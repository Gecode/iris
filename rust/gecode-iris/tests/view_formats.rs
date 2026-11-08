//! Generates a view of every format in `formats/`, plus specs that need dynamic
//! handles or unusual identifiers, and compiles them as one program. The program
//! reads each case and compares what every handle gives with untyped access. It
//! has to pass `rustfmt --check` and `clippy -D warnings` as generated. A second
//! program checks each way of bringing a view into a crate.
use gecode_iris_cli::check_spec;
use gecode_iris_conformance::canonical::Outcome;
use gecode_iris_conformance::case::{Case, Operation, discover};
use gecode_iris_gen::{
    Language,
    view::{self, View},
    view_code,
};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// A spec whose module the program has, and the handles of that module.
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
/// handle list: a name without one shape, and names Rust cannot spell.
const SPECS: &[(&str, &str, &str)] = &[
    (
        "redefined",
        "x:int let x=\"s\" y:int y:float z:word",
        "1 2 2.5 t",
    ),
    (
        "open",
        "n:int param p a[1]:int param q=a repeat n {let r=a}",
        "0 7",
    ),
    ("optional", "x:int if x {y:float} u:num", "0 2.5"),
    (
        "spellings",
        "fn:int self:int Self:int a_b:int aB:int A_B:int e1:int foo:int Field:int crate:int \
         gen:int repeat until \"END\" store _ {t:int}",
        "1 2 3 4 5 6 7 8 9 10 11 12 END",
    ),
];

/// Reaches beyond the widest identifier and past a full line of the string.
const LONGEST_NAME: usize = 130;

/// A depth above the highest depth type, which only a dynamic handle can read.
fn deep_spec() -> String {
    "repeat 1 {".repeat(9) + "v:int" + &"}".repeat(9)
}

#[test]
fn views_read_the_same_data_as_untyped_access() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let project = Project::new("view-formats");
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
        reads.push(Read {
            module: (*name).to_owned(),
            spec: spec.as_bytes().to_vec(),
            input: input.as_bytes().to_vec(),
            params: BTreeMap::from([("p".to_owned(), "1".to_owned())])
                .into_iter()
                .filter(|_| *name == "open")
                .collect(),
        });
    }
    let deep = deep_spec();
    modules.push(project.write_view("deep", deep.as_bytes()));
    reads.push(Read {
        module: "deep".to_owned(),
        spec: deep.into_bytes(),
        input: b"1".to_vec(),
        params: BTreeMap::new(),
    });
    let widths = layout_spec();
    modules.push(project.write_view("widths", widths.as_bytes()));
    reads.push(Read {
        module: "widths".to_owned(),
        spec: widths.into_bytes(),
        input: format!("1 {}", "1 1.5 w 1.5 ".repeat(LONGEST_NAME)).into_bytes(),
        params: BTreeMap::new(),
    });

    project.write_main(&modules, &reads);
    project.check_formatting(&modules);
    project.lint();
    let compared = project.run();
    println!("Views: {} modules, {compared}", modules.len());
    assert!(compared.contains(&format!("{} reads", reads.len())));
}

/// A view and a view without handles as a `mod` file, as an `include!` in a
/// module, and as an `include!` at the crate root. Nothing uses the handles, so
/// a warning about them or about the imports fails the check.
#[test]
fn a_view_compiles_in_every_form() {
    let project = Project::new("view-forms");
    project.write_view("view", b"n:int a[1]:float if n {s:word} x:int x:word");
    project.write_view("empty", b"skip line\n");
    fs::write(project.root.join("src/main.rs"), FORMS).unwrap();
    project.lint();
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

/// Names of every length around the widths where a declaration changes layout,
/// for each kind of type, in branches so that the optional types appear too.
fn layout_spec() -> String {
    let mut spec = String::from("c:int\n");
    for length in 1..=LONGEST_NAME {
        let name = "n".repeat(length);
        writeln!(spec, "{name}_i: int").unwrap();
        writeln!(spec, "{name}_f[1][1][1]: float").unwrap();
        writeln!(spec, "if c {{\n{name}_s: word\n}}").unwrap();
        writeln!(spec, "if c {{\n{name}_d[1][1][1][1][1][1][1][1]: float\n}}").unwrap();
    }
    spec
}

/// A Cargo project outside the workspace that holds the generated views.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        let source = root.join("src");
        if source.exists() {
            fs::remove_dir_all(&source).unwrap();
        }
        fs::create_dir_all(source).unwrap();
        let dependency = fs::canonicalize(manifest).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\n\
                 edition = \"2024\"\npublish = false\n\n[dependencies]\n\
                 gecode-iris = {{ path = {dependency:?} }}\n\n[workspace]\n",
                dependency = dependency.display().to_string()
            ),
        )
        .unwrap();
        fs::copy(manifest.join("../Cargo.lock"), root.join("Cargo.lock")).unwrap();
        Self { root }
    }

    fn write_view(&self, name: &str, spec: &[u8]) -> Module {
        let checked = check_spec(spec, name).unwrap();
        let code = view_code(&checked.parsed, &checked.analysis, Language::Rust).unwrap();
        fs::write(self.source(name), code).unwrap();
        Module {
            name: name.to_owned(),
            view: view::view(&checked.analysis).unwrap(),
        }
    }

    fn source(&self, module: &str) -> PathBuf {
        self.root.join("src").join(format!("{module}.rs"))
    }

    /// One program with a comparison of every handle of each module.
    fn write_main(&self, modules: &[Module], reads: &[Read]) {
        let mut main = String::from(PROGRAM);
        for module in modules {
            writeln!(main, "mod {};", module.name).unwrap();
        }
        for module in modules {
            let name = &module.name;
            writeln!(main, "\nfn compare_{name}(data: &Data) -> usize {{").unwrap();
            main.push_str("    let mut compared = 0;\n");
            let source = fs::read_to_string(self.source(name)).unwrap();
            for (constant, entry) in declared_handles(&source, &module.view) {
                writeln!(
                    main,
                    "    compared += compare(data, {name}::{constant}, {entry:?});"
                )
                .unwrap();
            }
            main.push_str("    compared\n}\n");
        }
        main.push_str("\nfn main() {\n    let mut compared = 0;\n");
        for read in reads {
            let params: Vec<_> = read.params.iter().collect();
            writeln!(
                main,
                "    compared += read({:?}, {:?}, &{params:?}, compare_{});",
                String::from_utf8(read.spec.clone()).unwrap(),
                ByteString(&read.input),
                read.module
            )
            .unwrap();
        }
        writeln!(
            main,
            "    println!(\"{} reads, {{compared}} entries compared\");\n}}",
            reads.len()
        )
        .unwrap();
        fs::write(self.root.join("src/main.rs"), main).unwrap();
    }

    fn check_formatting(&self, modules: &[Module]) {
        let mut rustfmt = Command::new("rustfmt");
        rustfmt.args(["--check", "--edition", "2024"]);
        rustfmt.args(modules.iter().map(|module| self.source(&module.name)));
        self.output(rustfmt, "rustfmt");
    }

    fn lint(&self) {
        let mut clippy = cargo();
        clippy.args(["clippy", "--offline", "--", "-D", "warnings"]);
        self.output(clippy, "cargo clippy");
    }

    /// Runs the program and returns its summary.
    fn run(&self) -> String {
        let mut run = cargo();
        run.args(["run", "--quiet", "--offline"]);
        self.output(run, "cargo run")
    }

    fn output(&self, mut command: Command, name: &str) -> String {
        let output = command.current_dir(&self.root).output().unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            output.status.success(),
            "{name} failed:\n{stdout}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout
    }
}

/// The constants of a generated module with the entries they read, by position:
/// the printer declares one handle per entry, in order.
fn declared_handles(source: &str, view: &View) -> Vec<(String, String)> {
    let constants: Vec<_> = source
        .lines()
        .filter_map(|line| line.strip_prefix("pub const "))
        .map(|rest| rest.split(':').next().unwrap().to_owned())
        .collect();
    assert_eq!(constants.len(), view.handles.len());
    let entries = view.handles.iter().map(|handle| handle.entry.clone());
    constants.into_iter().zip(entries).collect()
}

/// Bytes as a `&[u8]` literal in the generated program.
struct ByteString<'a>(&'a [u8]);

impl std::fmt::Debug for ByteString<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "b\"{}\"", self.0.escape_ascii())
    }
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// The program of `a_view_compiles_in_every_form`.
const FORMS: &str = r#"#![deny(warnings)]

mod empty;
mod view;

mod included {
    include!("view.rs");
}

mod included_empty {
    include!("empty.rs");
}

include!("view.rs");
include!("empty.rs");

fn main() {}
"#;

/// The start of the program: renders every view type and a `ValueRef` alike.
const PROGRAM: &str = r#"#![deny(warnings)]

use gecode_iris::handle::{Array, Depth, Handle, Leaf, Number, Numbers};
use gecode_iris::{AccessError, Data, Params, Spec, ValueRef};

trait Describe {
    fn describe(&self) -> String;
}

fn list<T: std::fmt::Debug>(items: &[T]) -> String {
    format!("{items:?}")
}

impl Describe for i64 {
    fn describe(&self) -> String {
        format!("{self:?}")
    }
}
impl Describe for f64 {
    fn describe(&self) -> String {
        format!("{self:?}")
    }
}
impl Describe for &str {
    fn describe(&self) -> String {
        format!("{self:?}")
    }
}
impl Describe for &[i64] {
    fn describe(&self) -> String {
        list(self)
    }
}
impl Describe for &[f64] {
    fn describe(&self) -> String {
        list(self)
    }
}
impl Describe for &[String] {
    fn describe(&self) -> String {
        list(self)
    }
}
impl Describe for Number {
    fn describe(&self) -> String {
        match self {
            Number::Int(number) => number.describe(),
            Number::Float(number) => number.describe(),
        }
    }
}
impl Describe for Numbers<'_> {
    fn describe(&self) -> String {
        match self {
            Numbers::Ints(row) => list(row),
            Numbers::Floats(row) => list(row),
        }
    }
}
impl<V: Leaf, R: Depth> Describe for Array<'_, V, R>
where
    for<'a> R::View<'a, V>: Describe,
{
    fn describe(&self) -> String {
        let rows: Vec<_> = self.rows().map(|row| row.describe()).collect();
        format!("[{}]", rows.join(", "))
    }
}
impl Describe for ValueRef<'_> {
    fn describe(&self) -> String {
        if self.depth() > 0 {
            let rows: Vec<_> = self.rows().map(|row| row.describe()).collect();
            return format!("[{}]", rows.join(", "));
        }
        match self.value_type() {
            "int" => self.as_int().unwrap().describe(),
            "float" => self.as_float().unwrap().describe(),
            _ => self.as_str().unwrap().describe(),
        }
    }
}
impl<T: Describe> Describe for Option<T> {
    fn describe(&self) -> String {
        self.as_ref().map_or_else(|| "absent".to_owned(), T::describe)
    }
}

/// Whether `handle` gives what untyped access gives, as a count of entries.
fn compare<H: Handle>(data: &Data, handle: H, entry: &str) -> usize
where
    for<'a> H::View<'a>: Describe,
{
    let typed = data.get(handle).describe();
    let untyped = match data.value(entry, &[]) {
        Ok(value) => value.describe(),
        Err(AccessError::UnknownEntry(_)) => "absent".to_owned(),
        Err(error) => panic!("{entry}: {error}"),
    };
    assert_eq!(typed, untyped, "entry {entry}");
    assert_eq!(handle.name(), entry);
    1
}

fn read(
    spec: &str,
    input: &[u8],
    params: &[(&str, &str)],
    compare: fn(&Data) -> usize,
) -> usize {
    let spec = Spec::parse(spec.as_bytes(), "spec.iris").unwrap();
    let mut values = Params::new();
    for (name, value) in params {
        values.set(*name, *value);
    }
    compare(&spec.read(input, "input", &values).unwrap())
}

"#;
