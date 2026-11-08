//! Generates a TypeScript view of every format in `formats/`, plus specs that
//! need dynamic handles, open depths or unusual identifiers, and holds them to
//! what they promise:
//!
//! - every view passes oxfmt's check and oxlint as generated, with the
//!   settings of `typescript/`;
//! - tsc, with the settings of `typescript/`, accepts the views and a program
//!   that pins, with a type equality, what `data.get(handle)` and
//!   `data.get(handle, { bigint: true })` give for each handle;
//! - that program reads each case and compares what every handle gives with
//!   untyped access, so a handle that promises the wrong value type, depth or
//!   presence fails, since `data.get` checks it, and every entry must have a
//!   handle.
//!
//! It is ignored by default, as the TypeScript builder round trip is, since
//! it needs Node 22.18 or later, `npm ci` and `npm run build` in
//! `typescript/`; CI runs it with `--ignored` after that build.
mod typescript_project;

use gecode_iris_cli::check_spec;
use gecode_iris_conformance::canonical::Outcome;
use gecode_iris_conformance::case::{Case, Operation, discover};
use gecode_iris_gen::{
    Language,
    view::{self, Access, Leaf, View},
    view_code,
};
use std::{collections::BTreeMap, fmt::Write as _, fs, path::Path};
use typescript_project::TypeScriptProject;

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

/// JavaScript's reserved and strict-mode words, globals that a declaration
/// would hide, the type that a view imports, the fields of a handle, and
/// names that are short or start with underscores, as a builder program
/// would rename them.
const SPELLINGS: &[&str] = &[
    "break",
    "case",
    "class",
    "const",
    "default",
    "delete",
    "enum",
    "export",
    "false",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "this",
    "typeof",
    "void",
    "with",
    "yield",
    "let",
    "static",
    "implements",
    "interface",
    "package",
    "await",
    "arguments",
    "eval",
    "globalThis",
    "undefined",
    "NaN",
    "Infinity",
    "Handle",
    "Handle2",
    "name",
    "valueType",
    "depth",
    "presence",
    "type",
    "any",
    "string",
    "number",
    "then",
    "constructor",
    "__proto__",
    "_",
    "e1",
    "b",
    "int",
    "a_b",
    "_a_b",
    "aB",
];

/// Specs that are not formats, with an input: a name without one shape, an
/// open shape, an optional entry, and a spec without entries.
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
    ("empty", "skip line", "anything\n"),
];

/// Reaches beyond the widest identifier and past a full line.
const LONGEST_NAME: usize = 130;

/// A depth above the deepest that a handle states, required and optional.
fn deep_spec() -> String {
    let nested = |body: &str| format!("{}{body}{}", "repeat 1 {".repeat(9), "}".repeat(9));
    format!("c:int {} if c {{ {} }}", nested("v:int"), nested("w:word"))
}

/// Names of every length around the widths where a declaration changes
/// layout, for each kind of handle type, the widest among them.
fn layout_spec() -> String {
    let mut spec = String::from("c:int\n");
    for length in 1..=LONGEST_NAME {
        let name = "n".repeat(length);
        writeln!(spec, "{name}_i: int").unwrap();
        writeln!(spec, "{name}_f[1][1][1]: float").unwrap();
        writeln!(spec, "if c {{\n{name}_s: word\n}}").unwrap();
        writeln!(spec, "if c {{\n{name}_d[1][1][1][1][1][1][1][1]: num\n}}").unwrap();
        let deep = format!("{}{name}_o: word{}", "repeat 1 {".repeat(9), "}".repeat(9));
        writeln!(spec, "{deep}").unwrap();
    }
    spec
}

#[test]
#[ignore = "needs Node and the built package in typescript/"]
fn typescript_views_type_check_and_read_the_same_data_as_untyped_access() {
    let project = TypeScriptProject::new("typescript-views", &["reads"]);
    let repo = project.package.parent().unwrap().to_owned();
    let mut modules = Vec::new();
    let mut reads = Vec::new();
    for (name, spec) in format_specs(&repo) {
        modules.push(write_view(&project, &name, &spec));
        reads.extend(format_reads(&repo, &name, &spec));
    }
    assert_eq!(modules.len(), 6, "the format library has six formats");
    assert!(reads.len() >= 6, "too few cases: {}", reads.len());
    for (name, spec, input) in SPECS {
        modules.push(write_view(&project, name, spec.as_bytes()));
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
    let spellings: String = SPELLINGS
        .iter()
        .map(|name| format!("{name}:int "))
        .collect();
    let numbers: Vec<String> = (1..=SPELLINGS.len()).map(|n| n.to_string()).collect();
    for (name, spec, input) in [
        ("spellings", spellings, numbers.join(" ")),
        ("deep", deep_spec(), "1 2 w".to_owned()),
        (
            "widths",
            layout_spec(),
            format!("1 {}", "1 1.5 w 1.5 o ".repeat(LONGEST_NAME)),
        ),
    ] {
        modules.push(write_view(&project, name, spec.as_bytes()));
        reads.push(Read {
            module: name.to_owned(),
            spec: spec.into_bytes(),
            input: input.into_bytes(),
            params: BTreeMap::new(),
        });
    }

    let views: Vec<_> = modules
        .iter()
        .map(|module| project.source(&module.name))
        .collect();
    project.check_style(&views);
    write_compare(&project, &modules);
    project.check_types();
    let summary = compare(&project, &reads);
    println!("TypeScript views: {} modules, {summary}", modules.len());
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

fn write_view(project: &TypeScriptProject, name: &str, spec: &[u8]) -> Module {
    let checked = check_spec(spec, name).unwrap();
    let code = view_code(&checked.parsed, &checked.analysis, Language::TypeScript).unwrap();
    fs::write(project.source(name), code).unwrap();
    Module {
        name: name.to_owned(),
        view: view::view(&checked.analysis).unwrap(),
    }
}

/// What `data.get(handle)` gives for a handle of `access`, with integers as
/// `bigint` when `bigint` is set, as a TypeScript type.
fn value_type(access: Access, bigint: bool) -> String {
    let open = if bigint { "BigIntValue" } else { "Value" };
    let Access::Typed {
        value,
        depth,
        optional,
    } = access
    else {
        // A builder's dynamic handle may always be absent.
        return format!("{open} | undefined");
    };
    let nested = if depth > 8 {
        open.to_owned()
    } else {
        let leaf = match (value, bigint) {
            (Leaf::Str, _) => "string",
            (Leaf::Float, _) | (Leaf::Int | Leaf::Num, false) => "number",
            (Leaf::Int, true) => "bigint",
            (Leaf::Num, true) => "(bigint | number)",
        };
        let nested = format!("{leaf}{}", "[]".repeat(depth));
        // A lone union needs no parentheses.
        if depth == 0 {
            nested.trim_matches(['(', ')']).to_owned()
        } else {
            nested
        }
    };
    if optional {
        format!("{nested} | undefined")
    } else {
        nested
    }
}

/// The identifiers of a generated module with the handles' access, by
/// position: the printer declares one handle per entry, in order.
fn declared_handles(source: &str, view: &View) -> Vec<(String, Access)> {
    let identifiers: Vec<_> = source
        .lines()
        .filter_map(|line| line.strip_prefix("export const "))
        .map(|line| line.split_once(':').unwrap().0.to_owned())
        .collect();
    assert_eq!(identifiers.len(), view.handles.len());
    let accesses = view.handles.iter().map(|handle| handle.access);
    identifiers.into_iter().zip(accesses).collect()
}

/// The start of the comparison program: it pins the type of every handle's
/// access, and `compare` reads each case of the manifest and compares every
/// handle of its view with untyped access, and checks that every entry has a
/// handle.
const COMPARE: &str = r#"import { readFileSync } from "node:fs";
import { inspect, isDeepStrictEqual } from "node:util";
import { type BigIntValue, type Data, type Handle, Spec, type Value } from "gecode-iris";

type IsAny<T> = 0 extends 1 & T ? true : false;
type Equal<A, B> =
  IsAny<A> extends IsAny<B> ? ([A] extends [B] ? ([B] extends [A] ? true : false) : false) : false;
function same<A, B>(check: Equal<A, B>): void {
  void check;
}

/** What `access` gives: its value, or that it fails. */
function outcome(access: () => unknown): { value: unknown } | "fails" {
  try {
    return { value: access() };
  } catch {
    return "fails";
  }
}

interface Read {
  module: string;
  params: Record<string, string>;
}

function compare(root: string, views: Record<string, Record<string, Handle>>): void {
  const reads = JSON.parse(readFileSync(`${root}/manifest.json`, "utf8")) as Read[];
  let compared = 0;
  for (const [number, read] of reads.entries()) {
    const spec = Spec.parse(readFileSync(`${root}/reads/${number}.iris`));
    const input = readFileSync(`${root}/reads/${number}.txt`);
    const data = spec.read(input, "<input>", { params: read.params });
    const handles = Object.values(views[read.module] ?? {});
    const names = new Set(handles.map((handle) => handle.name));
    for (const entry of data.entries) {
      if (!names.has(entry.name)) {
        throw new Error(`${read.module}: entry ${entry.name} has no handle`);
      }
    }
    for (const handle of handles) {
      const present = data.has(handle.name);
      for (const bigint of [false, true]) {
        const options = { bigint } as const;
        const untyped = outcome(() =>
          present ? data.value(handle.name, [], options) : undefined,
        );
        const typed = outcome(() => data.get(handle, options));
        if (!isDeepStrictEqual(typed, untyped)) {
          const both = `${inspect(typed)} != ${inspect(untyped)}`;
          throw new Error(`${read.module}.${handle.name} (bigint ${bigint}): ${both}`);
        }
        if (typed === "fails" && bigint) {
          throw new Error(`${read.module}.${handle.name} fails as bigint`);
        }
      }
      compared += 1;
    }
  }
  console.log(`${reads.length} reads, ${compared} entries compared`);
}
"#;

/// The comparison program, `src/compare.ts`, with a function per module that
/// pins the type of `data.get` for every handle.
fn write_compare(project: &TypeScriptProject, modules: &[Module]) {
    let mut text = String::from(COMPARE);
    let mut imports = String::new();
    for module in modules {
        writeln!(imports, "import * as {0} from \"./{0}.ts\";", module.name).unwrap();
    }
    text = text.replacen("\ntype IsAny", &format!("{imports}\ntype IsAny"), 1);
    for module in modules {
        let name = &module.name;
        writeln!(text, "\nexport function pin_{name}(data: Data): void {{").unwrap();
        let source = fs::read_to_string(project.source(name)).unwrap();
        for (identifier, access) in declared_handles(&source, &module.view) {
            let handle = format!("{name}.{identifier}");
            for (bigint, options) in [(false, ""), (true, ", { bigint: true }")] {
                let expected = value_type(access, bigint);
                writeln!(
                    text,
                    "  {{\n    const got = data.get({handle}{options});\n    \
                     same<typeof got, {expected}>(true);\n  }}"
                )
                .unwrap();
            }
        }
        text.push_str("}\n");
    }
    let views: Vec<String> = modules.iter().map(|module| module.name.clone()).collect();
    writeln!(
        text,
        "\ncompare(process.argv[2] ?? \".\", {{ {} }});",
        views.join(", ")
    )
    .unwrap();
    fs::write(project.source("compare"), text).unwrap();
}

/// Runs the comparison program and returns its summary.
fn compare(project: &TypeScriptProject, reads: &[Read]) -> String {
    let mut manifest = Vec::new();
    for (number, read) in reads.iter().enumerate() {
        fs::write(
            project.root.join(format!("reads/{number}.iris")),
            &read.spec,
        )
        .unwrap();
        fs::write(
            project.root.join(format!("reads/{number}.txt")),
            &read.input,
        )
        .unwrap();
        manifest.push(serde_json::json!({
            "module": read.module,
            "params": read.params,
        }));
    }
    let manifest = serde_json::to_string(&manifest).unwrap();
    fs::write(project.root.join("manifest.json"), manifest).unwrap();
    project.node("compare", &project.source("compare"), &[&project.root])
}
