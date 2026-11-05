//! Round-trips every corpus spec that `iris check` accepts through the Rust
//! builder: generate the builder program, format-check it, lint it, run it, and
//! compare the spec it prints with the original. Specs written for the layout
//! and lint rules of the backend go the same way.
#[path = "../../iris-gen/tests/equal_operands/mod.rs"]
mod equal_operands;
#[path = "../../iris-gen/tests/layout_forms/mod.rs"]
mod layout_forms;

use gecode_iris_cli::check_spec;
use gecode_iris_conformance::compare::compare;
use gecode_iris_gen::{Language, builder_code};
use std::{
    collections::BTreeSet,
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

    let mut differences = Vec::new();
    for spec in &generated {
        if let Err(message) = project.round_trip(&fs::read(&spec.path).unwrap(), &spec.module) {
            differences.push(format!("{}: {message}", spec.path.display()));
        }
    }
    for (name, text) in &limits {
        if let Err(message) = project.round_trip(text.as_bytes(), name) {
            differences.push(format!("limit spec {name}: {message}"));
        }
    }
    println!(
        "Builder round trip: compared {compared}, skipped {skipped}, extra {extra}, differences {}",
        differences.len()
    );
    for difference in &differences {
        println!("DIFF {difference}");
    }
    assert_eq!(compared + skipped, specs.len());
    assert!(compared >= 770, "too few accepted specs: {compared}");
    assert!(differences.is_empty(), "{} differences", differences.len());
}

/// Every distinct `.iris` file of the conformance cases and format library.
fn corpus(repo: &Path) -> BTreeSet<PathBuf> {
    let mut pending = vec![repo.join("conformance/cases/0.1"), repo.join("formats")];
    let mut specs = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "iris")
            {
                specs.insert(fs::canonicalize(path).unwrap());
            }
        }
    }
    specs
}

/// Lengths of names and text for every layout form, on either side of the widths
/// where a call, then a line, is full.
const LAYOUT_LENGTHS: &[usize] = &[33, 56, 96];
/// The forms whose closures end a block with a value, which is where
/// `clippy::let_and_return` looks, with every length up to this.
const BLOCK_ENDS: (&[&str], usize) = (
    &[
        "comprehension-binary",
        "comprehension-block",
        "comprehension-nested-domain",
        "comprehension-tail",
    ],
    100,
);

/// Specs beyond the corpus: every layout form at a few widths, the ends of
/// closures at every width, and the operands of `clippy::eq_op`, by name.
fn extra_specs() -> Vec<(String, String)> {
    let mut specs = Vec::new();
    let mut add = |title: &str, form: &str, length: usize, depth: usize| {
        let (name, text) = ("n".repeat(length), "t".repeat(length));
        let module = format!("{}_{depth}_{length}", title.replace('-', "_"));
        specs.push((module, layout_forms::spec(form, &name, &text, depth)));
    };
    for (title, form) in layout_forms::FORMS {
        let (ends, longest) = BLOCK_ENDS;
        for depth in [0, 4, 7] {
            if depth > 0 && !layout_forms::nests(form) {
                continue;
            }
            for &length in LAYOUT_LENGTHS {
                add(title, form, length, depth);
            }
            if ends.contains(title) && depth < 7 {
                (1..=longest)
                    .filter(|length| !LAYOUT_LENGTHS.contains(length))
                    .for_each(|length| add(title, form, length, depth));
            }
        }
    }
    let operands = [
        ("equal", equal_operands::EQUAL),
        ("different", equal_operands::DIFFERENT),
    ];
    for (title, expressions) in operands {
        for (index, expression) in expressions.iter().enumerate() {
            specs.push((format!("{title}_{index}"), equal_operands::spec(expression)));
        }
    }
    specs
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

/// Specs at the limits of Specification §5.12, by name. None of them is in the
/// corpus count, and the deep ones show that generation needs no stack of its
/// own and that the generated program still compiles.
fn limit_specs() -> Vec<(&'static str, String)> {
    let nested = |open: &str, inner: &str, close: &str, count: usize| {
        format!("{}{inner}{}", open.repeat(count), close.repeat(count))
    };
    let else_ifs: String = (1..=255)
        .map(|arm| format!("else if x == {arm} {{\n  skip line\n}}\n"))
        .collect();
    let comprehension_levels = 255;
    let comprehensions: String = (0..comprehension_levels)
        .map(|level| format!("sum(v{level} in a: "))
        .collect();
    // Each level holds the next one's list, so the chain resolves at every step.
    // A field of a repeat needs an index first, so the longest chain that name
    // resolution accepts alternates the two suffixes and stops at 254 of them.
    let record_levels = 127;
    let mut records = String::new();
    let mut chain = String::from("r0");
    for level in 0..record_levels {
        records.push_str(&format!("repeat 1 as r{level} {{\n"));
        if level > 0 {
            chain.push_str(&format!("[0].r{level}"));
        }
    }
    records.push_str("v: int\n");
    records.push_str(&"}\n".repeat(record_levels));
    vec![
        (
            "limit_repeat",
            nested("repeat 1 {\n", "v: int\n", "}\n", 256),
        ),
        (
            "limit_if",
            format!("x: int\n{}", nested("if x > 0 {\n", "y: int\n", "}\n", 256)),
        ),
        (
            "limit_arm",
            nested("sections {\n\"a\" {\n", "v: int\n", "}\n}\n", 256),
        ),
        (
            "limit_else_if",
            format!("x: int\nif x == 0 {{\n  skip line\n}}\n{else_ifs}"),
        ),
        (
            "limit_parentheses",
            format!("let x = {}\n", nested("(", "1", ")", 255)),
        ),
        ("limit_unary", format!("let x = {}1\n", "-".repeat(255))),
        (
            "limit_additions",
            format!("let x = 1{}\n", "+1".repeat(255)),
        ),
        ("limit_powers", format!("let x = 1{}\n", "^1".repeat(255))),
        (
            "limit_indices",
            format!("x: int\nlet y = x{}\n", "[0]".repeat(254)),
        ),
        ("limit_fields", format!("{records}let x = {chain}[0].v\n")),
        (
            "limit_comprehensions",
            format!(
                "let a = 1\nlet x = {comprehensions}v0{}\n",
                ")".repeat(comprehension_levels)
            ),
        ),
    ]
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

    /// Whether a builder ran and printed a spec equal to its original.
    fn round_trip(&self, original: &[u8], name: &str) -> Result<(), String> {
        let out = self.root.join("out");
        let error = out.join(format!("{name}.error"));
        if error.exists() {
            return Err(format!("builder: {}", fs::read_to_string(error).unwrap()));
        }
        let printed = fs::read(out.join(format!("{name}.iris")))
            .map_err(|error| format!("no printed spec: {error}"))?;
        check_spec(&printed, name)
            .map_err(|error| format!("the printed spec is rejected: {error}"))?;
        match compare(original, &printed) {
            Ok(None) => Ok(()),
            Ok(Some(difference)) => Err(difference.to_string()),
            Err(failure) => Err(failure.to_string()),
        }
    }
}

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}
