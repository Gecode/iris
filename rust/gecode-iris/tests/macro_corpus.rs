//! Compiles every valid reader spec in the conformance cases and the format
//! library through both forms of `iris!`, then runs the program. It checks
//! that each expansion reads every case of its spec exactly as `Spec::parse`
//! reads it.
use gecode_iris::Spec;
use gecode_iris_conformance::case::{self, Case, Operation};
use std::{
    collections::BTreeMap,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

#[test]
fn valid_specs_expand_and_read_like_spec_parse() {
    let cases = load_cases();
    let mut specs = cases_by_spec(&cases);
    let discovered = specs.len();
    specs.retain(|path, _| Spec::parse(&fs::read(path).unwrap(), "iris!").is_ok());
    let compared: usize = specs.values().map(|cases| inputs(cases).count()).sum();
    let program = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("macro_corpus.rs");
    fs::write(&program, corpus_program(&specs)).unwrap();
    println!(
        "Compiling {} of {discovered} reader specs (the valid ones) through inline and file \
         iris!, comparing {compared} cases",
        specs.len()
    );
    assert!(
        specs.len() >= 770,
        "too few valid reader specs: {}",
        specs.len()
    );
    trybuild::TestCases::new().pass(&program);
}

fn load_cases() -> Vec<Case> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roots = [repo.join("conformance/cases/0.1"), repo.join("formats")];
    case::discover(&roots, None)
        .unwrap()
        .iter()
        .map(|path| Case::load(path, "0.1").unwrap())
        .collect()
}

/// The cases of each distinct spec file, keyed by the canonical spec path.
fn cases_by_spec(cases: &[Case]) -> BTreeMap<PathBuf, Vec<&Case>> {
    let mut specs: BTreeMap<PathBuf, Vec<&Case>> = BTreeMap::new();
    for case in cases {
        specs
            .entry(case.spec.canonicalize().unwrap())
            .or_default()
            .push(case);
    }
    specs
}

fn inputs<'a>(cases: &'a [&'a Case]) -> impl Iterator<Item = (&'a Case, &'a Path)> {
    cases
        .iter()
        .filter_map(|case| Some((*case, case.input.as_deref()?)))
}

const PRELUDE: &str = r#"use gecode_iris::{Failure, Params, Spec};

/// One case read through one `iris!` expansion and through `Spec::parse` on
/// the same bytes and origin.
struct Comparison {
    case: &'static str,
    expand: fn() -> Spec,
    bytes: &'static [u8],
    origin: &'static str,
    input: &'static str,
    params: &'static [(&'static str, &'static str)],
    catalog: bool,
}

impl Comparison {
    fn run(&self) {
        let mut params = Params::new();
        for (name, value) in self.params {
            params.set(*name, *value);
        }
        let reference = Spec::parse(self.bytes, self.origin).unwrap();
        assert_eq!(
            self.outcome(&(self.expand)(), &params),
            self.outcome(&reference, &params),
            "{} with origin {}",
            self.case,
            self.origin,
        );
    }

    /// The canonical JSON of the data or language error, or the failure's
    /// Debug form.
    fn outcome(&self, spec: &Spec, params: &Params) -> String {
        let result = if self.catalog {
            spec.catalog_path(self.input, params)
        } else {
            spec.read_path(self.input, params)
        };
        let json = match result {
            Ok(data) => data.to_json().unwrap(),
            Err(Failure::Language(error)) => error.to_json().unwrap(),
            Err(failure) => return format!("{failure:?}"),
        };
        String::from_utf8(json).unwrap()
    }
}
"#;

// Every expansion is constructed once. Some work-budget cases take seconds in
// a debug build, so the comparisons run on one thread per core.
const MAIN: &str = r#"
fn main() {
    for expand in EXPANSIONS {
        expand();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some(comparison) =
                    COMPARISONS.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                {
                    comparison.run();
                }
            });
        }
    });
}
"#;

/// A program with an inline and a file expansion of each spec, and a table of
/// comparisons for every case that has an input.
fn corpus_program(specs: &BTreeMap<PathBuf, Vec<&Case>>) -> String {
    let mut program = String::from(PRELUDE);
    let mut expansions = String::new();
    let mut comparisons = String::new();
    for (index, (path, cases)) in specs.iter().enumerate() {
        let path = path.to_str().expect("spec paths are UTF-8");
        let inline = macro_spec(&string_literal(&fs::read_to_string(path).unwrap()));
        let file = macro_spec(&format!("file = {path:?}"));
        writeln!(program, "\n// {path}").unwrap();
        writeln!(
            program,
            "const SPEC_{index}: &[u8] = include_bytes!({path:?});"
        )
        .unwrap();
        writeln!(program, "fn inline_{index}() -> Spec {{\n    {inline}\n}}").unwrap();
        writeln!(program, "fn file_{index}() -> Spec {{\n    {file}\n}}").unwrap();
        writeln!(expansions, "    inline_{index},\n    file_{index},").unwrap();
        for (case, input) in inputs(cases) {
            let input = input.to_str().expect("input paths are UTF-8");
            for (expand, origin) in [("inline", "iris!"), ("file", path)] {
                writeln!(
                    comparisons,
                    "    Comparison {{ case: {:?}, expand: {expand}_{index}, bytes: SPEC_{index}, \
                     origin: {origin:?}, input: {input:?}, params: &{:?}, catalog: {} }},",
                    case.display_path,
                    params(case),
                    case.operation == Operation::Catalog,
                )
                .unwrap();
            }
        }
    }
    writeln!(
        program,
        "\nconst EXPANSIONS: &[fn() -> Spec] = &[\n{expansions}];"
    )
    .unwrap();
    writeln!(
        program,
        "\nconst COMPARISONS: &[Comparison] = &[\n{comparisons}];"
    )
    .unwrap();
    program.push_str(MAIN);
    program
}

/// The one expression that turns an `iris!` expansion into a `Spec`.
fn macro_spec(argument: &str) -> String {
    format!("gecode_iris::iris!({argument})")
}

/// A Rust literal whose value is `text`: raw with the fewest hashes, or
/// escaped when the text contains a carriage return, which raw strings reject.
fn string_literal(text: &str) -> String {
    if text.contains('\r') {
        return format!("{text:?}");
    }
    let hashes = (0..)
        .map(|count| "#".repeat(count))
        .find(|hashes| !text.contains(&format!("\"{hashes}")))
        .unwrap();
    format!("r{hashes}\"{text}\"{hashes}")
}

fn params(case: &Case) -> Vec<(String, String)> {
    case.params.as_deref().map_or_else(Vec::new, |text| {
        serde_json::from_str::<BTreeMap<String, String>>(text)
            .unwrap()
            .into_iter()
            .collect()
    })
}
