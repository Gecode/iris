//! Round-trips every corpus spec that `iris check` accepts through the
//! TypeScript builder: generate the builder program, check that oxfmt would
//! leave it as it is, lint it with oxlint and type-check it with tsc under
//! the settings of `typescript/`, run it with Node, and compare the spec it
//! prints with the original. The layout forms and the limit specs of the Rust
//! round trip go the same way.
//!
//! The programs go to one directory outside the repository, which resolves
//! `gecode-iris` to `typescript/` through a link, so oxfmt, oxlint and tsc
//! each check every program in one run, and `typescript/test/round-trip-runner.ts`
//! runs them all in one Node process. It needs Node 22.18 or later, `npm ci`
//! and `npm run build` in `typescript/`.
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
#[ignore = "needs Node and the built package in typescript/"]
fn accepted_specs_round_trip_through_the_typescript_builder() {
    let started = Instant::now();
    let repo = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let package = repo.join("typescript");
    assert!(
        package.join("dist/build/node.js").exists() && package.join("node_modules").exists(),
        "run `npm ci` and `npm run build` in {} first",
        package.display()
    );
    let specs = corpus(&repo);
    let project = Project::new(&package);

    // Each program with the spec it must print.
    let mut generated: Vec<(String, Vec<u8>, String)> = Vec::new();
    let mut skipped = 0;
    for path in &specs {
        let bytes = fs::read(path).unwrap();
        let Ok(checked) = check_spec(&bytes, &path.to_string_lossy()) else {
            skipped += 1;
            continue;
        };
        let code = builder_code(&checked.parsed, &checked.analysis, Language::TypeScript)
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
    for (name, text) in typescript_specs() {
        let program = program_name(generated.len());
        project.write(&program, &name, &text);
        generated.push((
            program,
            text.into_bytes(),
            format!("TypeScript spec {name}"),
        ));
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
    project.check(&package);
    println!("oxfmt, oxlint and tsc clean in {:.1?}", step.elapsed());
    let step = Instant::now();
    project.run(&package);
    println!("ran in {:.1?}", step.elapsed());

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
    tally.report("TypeScript builder", &differences);
}

/// The most programs that one run of oxfmt or oxlint checks.
const BATCH: usize = 500;

/// The name of the program at `index`, which is also its file's and its
/// output's stem.
fn program_name(index: usize) -> String {
    format!("spec{index}")
}

/// Specs for what only TypeScript needs: names that JavaScript reserves, that
/// the program uses itself or that oxlint refuses, as values, types and
/// comprehension variables, also in blocks deep enough for functions of
/// their own, with a type that a deep block takes from the block around it,
/// an `if` chain whose links are too deep, and a body that needs a function;
/// strings with both quotes, backticks and `${`; more section clauses than
/// one line holds; and floats that oxlint's `oxc/approx-constant` takes for
/// `Math` constants.
fn typescript_specs() -> Vec<(String, String)> {
    let names = [
        "in",
        "int",
        "b",
        "s1",
        "a2",
        "e1",
        "t1",
        "block1",
        "body1",
        "builder",
        "process",
        "p",
        "undefined",
        "Infinity",
        "NaN",
        "arguments",
        "lit",
        "sum",
        "_name",
        "name_",
        "name",
        "eval",
        "Scope",
        "SpecBuilder",
        "n_jobs",
        "nJobs",
        "N_JOBS",
        "globalThis",
    ];
    let mut text: String = names.iter().map(|name| format!("{name}: int\n")).collect();
    text.push_str("list[2]: int\ntype function = 1..9\ntype await = function\n");
    let body = "z: digit\n\
                if z > 1 { y: await } else if z > 2 { w: digit } else if z > 3 { v: int } else if: int\n\
                let total = sum(it in list: sum(field in list: it + field + int * b + _name))\n\
                let check = defined(undefined) && count(list) > 0\n\
                check \"say \\\"hi\\\" it's\" != \"`x` ${y} $5\" \"costs $5 ${it}\"\n";
    text.push_str(&format!(
        "{}type digit = 0..9\n{}{body}{}",
        "repeat 1 {\n",
        "repeat 1 {\n".repeat(10),
        "}\n".repeat(11)
    ));
    let clauses = "sections until \"X\" store e1 skip other until \"Y\" store b until \"Z\" \
                   until \"a very long terminator that fills a line of its own\" \
                   {\"A\" optional}\n";
    // Every type of the blocks around the deepest one, which its function
    // takes, more than one line holds.
    let mut captured_types = String::from("list a: int until eol\n");
    for level in 0..10 {
        captured_types.push_str(&format!("repeat 1 {{\ntype digit_{level} = 0..{level}\n"));
    }
    for level in 0..10 {
        captured_types.push_str(&format!("x{level}: digit_{level}\n"));
    }
    captured_types.push_str(&"}\n".repeat(10));
    // Comprehensions nested so deep that the body is a function, which takes
    // more variables than one line holds.
    let variables: Vec<String> = (0..18).map(|index| format!("v{index}")).collect();
    let mut comprehensions = String::from("list a: int until eol\nlet s = ");
    for variable in &variables {
        comprehensions.push_str(&format!("sum({variable} in a: "));
    }
    comprehensions.push_str(&variables.join(" + "));
    comprehensions.push_str(&")".repeat(variables.len()));
    comprehensions.push('\n');
    // `else` branches nested so that their statements start far right while
    // the arrow functions around them are few.
    let mut else_chains = String::from("n: int\n");
    for level in 0..9 {
        else_chains.push_str(&format!(
            "if n > {level} {{}} else if n < {level} {{}} else {{\n"
        ));
    }
    else_chains.push_str("x: int\n");
    else_chains.push_str(&"}\n".repeat(9));
    vec![
        ("typescript_names".to_owned(), text),
        ("section_clauses".to_owned(), clauses.to_owned()),
        (
            "near_constants".to_owned(),
            "let pi = 3.141592653589793\nlet e = 2.718281828459045\nlet half = -0.7071\n\
             repeat 1 {\n  let ln = 0.6931 + 2.3026\n  let near = 3.14 + 1.5708\n}\n"
                .to_owned(),
        ),
        ("captured_types".to_owned(), captured_types),
        ("comprehension_variables".to_owned(), comprehensions),
        ("else_chains".to_owned(), else_chains),
    ]
}

/// The generated programs and what they print, outside the repository.
struct Project {
    root: PathBuf,
}

impl Project {
    /// A directory whose programs import `gecode-iris/build` from `package`
    /// and are checked with its TypeScript settings.
    fn new(package: &Path) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("typescript-round-trip");
        // Files of an earlier run must not survive.
        for directory in ["src", "out"] {
            let path = root.join(directory);
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path).unwrap();
        }
        fs::write(root.join("package.json"), "{ \"type\": \"module\" }\n").unwrap();
        let tsconfig = format!(
            "{{\n  \"extends\": {},\n  \"compilerOptions\": {{ \"typeRoots\": [{}] }},\n  \
             \"include\": [\"src\"]\n}}\n",
            json_string(&package.join("tsconfig.json")),
            json_string(&package.join("node_modules/@types")),
        );
        fs::write(root.join("tsconfig.json"), tsconfig).unwrap();
        let modules = root.join("node_modules");
        fs::create_dir_all(&modules).unwrap();
        let link = modules.join("gecode-iris");
        if fs::symlink_metadata(&link).is_ok() {
            fs::remove_file(&link)
                .or_else(|_| fs::remove_dir(&link))
                .unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(package, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(package, &link).unwrap();
        Self { root }
    }

    /// Save the program `code` as `src/NAME.ts`.
    fn save(&self, name: &str, code: &str) {
        fs::write(self.root.join("src").join(format!("{name}.ts")), code).unwrap();
    }

    /// The program `program` of the spec `name` beyond the corpus, which
    /// must pass `iris check`.
    fn write(&self, program: &str, name: &str, text: &str) {
        let checked = check_spec(text.as_bytes(), name)
            .unwrap_or_else(|error| panic!("{name}: {error}\n{text}"));
        let code = builder_code(&checked.parsed, &checked.analysis, Language::TypeScript).unwrap();
        self.save(program, &code);
    }

    /// Check every program with oxfmt, oxlint and tsc, as `typescript/`
    /// configures them. oxfmt and oxlint skip what git ignores, as the
    /// target directory, in a directory they walk, so they get the programs
    /// as files, in batches that a command line holds; oxlint runs in
    /// `package` to find its type-aware linter there.
    fn check(&self, package: &Path) {
        let mut programs: Vec<PathBuf> = fs::read_dir(self.root.join("src"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        programs.sort();
        let tsconfig = self.root.join("tsconfig.json");
        for (batch, files) in programs.chunks(BATCH).enumerate() {
            let mut oxfmt = Command::new(tool(package, "oxfmt"));
            oxfmt
                .current_dir(package)
                .arg("--check")
                .arg("-c")
                .arg(package.join(".oxfmtrc.json"))
                .args(files);
            self.expect_success(&format!("oxfmt-{batch}"), oxfmt);
            let mut oxlint = Command::new(tool(package, "oxlint"));
            oxlint
                .current_dir(package)
                .args(["--type-aware", "--deny-warnings", "-c"])
                .arg(package.join(".oxlintrc.json"))
                .arg("--tsconfig")
                .arg(&tsconfig)
                .args(files);
            self.expect_success(&format!("oxlint-{batch}"), oxlint);
        }
        let mut tsc = Command::new(tool(package, "tsc"));
        tsc.current_dir(package)
            .arg("-p")
            .arg(&tsconfig)
            .arg("--noEmit");
        self.expect_success("tsc", tsc);
    }

    /// Run every program with `typescript/test/round-trip-runner.ts`, which
    /// writes what each prints to `out`.
    fn run(&self, package: &Path) {
        let mut node = Command::new("node");
        node.current_dir(package)
            .arg(package.join("test/round-trip-runner.ts"))
            .arg(self.root.join("src"))
            .arg(self.root.join("out"));
        self.expect_success("node", node);
    }

    /// Run `command`, with its output in `NAME.log`, which a failure prints.
    fn expect_success(&self, name: &str, mut command: Command) {
        let log = self.root.join(format!("{name}.log"));
        let output = command
            .output()
            .unwrap_or_else(|error| panic!("cannot run {name}: {error}"));
        let mut text = output.stdout;
        text.extend_from_slice(&output.stderr);
        fs::write(&log, &text).unwrap();
        assert!(
            output.status.success(),
            "{name} failed (log in {}):\n{}",
            log.display(),
            String::from_utf8_lossy(&text)
        );
    }
}

/// The executable `name` that `npm ci` installs in `package`.
fn tool(package: &Path, name: &str) -> PathBuf {
    let bin = package.join("node_modules/.bin");
    if cfg!(windows) {
        bin.join(format!("{name}.cmd"))
    } else {
        bin.join(name)
    }
}

/// `path` as a JSON string.
fn json_string(path: &Path) -> String {
    let mut text = String::from("\"");
    for character in path.to_string_lossy().chars() {
        match character {
            '"' | '\\' => {
                text.push('\\');
                text.push(character);
            }
            character => text.push(character),
        }
    }
    text.push('"');
    text
}
