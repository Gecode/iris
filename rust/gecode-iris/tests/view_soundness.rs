//! Holds the handles of generated views to their promise: on any data that a
//! read produces, `Data::try_get` through a typed handle succeeds. The cases are
//! every successful read of the conformance suite and format library, and
//! generated specs from the differential generator. This needs no compiled
//! views: it calls what a handle's `get` calls, with the same marker types.
use gecode_iris::handle::{
    self, D0, D1, D2, D3, D4, D5, D6, D7, D8, Depth, Float, Int, Leaf, Maybe, Num, Presence,
    Required, Str,
};
use gecode_iris::{AccessError, Data, Params, Spec};
use gecode_iris_cli::{check_spec, protocol};
use gecode_iris_conformance::case::{Case, Operation, discover};
use gecode_iris_conformance::differential;
use gecode_iris_gen::view::{self, Access, View};
use std::{collections::HashSet, fs, path::Path};

/// Cases of the differential generator, for a test of a few seconds.
const GENERATED_CASES: u64 = 2_000;

fn try_presence<V: Leaf, D: Depth, O: Presence>(
    data: &Data,
    name: &str,
) -> Result<(), AccessError> {
    handle::view::<V, D, O>(data, name).map(|_| ())
}

fn try_depth<V: Leaf, D: Depth>(
    data: &Data,
    name: &str,
    optional: bool,
) -> Result<(), AccessError> {
    if optional {
        try_presence::<V, D, Maybe>(data, name)
    } else {
        try_presence::<V, D, Required>(data, name)
    }
}

fn try_leaf<V: Leaf>(
    data: &Data,
    name: &str,
    depth: usize,
    optional: bool,
) -> Result<(), AccessError> {
    match depth {
        0 => try_depth::<V, D0>(data, name, optional),
        1 => try_depth::<V, D1>(data, name, optional),
        2 => try_depth::<V, D2>(data, name, optional),
        3 => try_depth::<V, D3>(data, name, optional),
        4 => try_depth::<V, D4>(data, name, optional),
        5 => try_depth::<V, D5>(data, name, optional),
        6 => try_depth::<V, D6>(data, name, optional),
        7 => try_depth::<V, D7>(data, name, optional),
        8 => try_depth::<V, D8>(data, name, optional),
        _ => Ok(()), // Rust has no depth type here, so the handle is dynamic.
    }
}

/// What is wrong with `view` on `data`, if anything.
fn violation(view: &View, data: &Data) -> Option<String> {
    let names: HashSet<&str> = view.handles.iter().map(|h| h.entry.as_str()).collect();
    if let Some(entry) = data.entries().iter().find(|e| !names.contains(e.name())) {
        return Some(format!("entry '{}' has no handle", entry.name()));
    }
    view.handles.iter().find_map(|handle| {
        let Access::Typed {
            value,
            depth,
            optional,
        } = handle.access
        else {
            return None;
        };
        let name = &handle.entry;
        let result = match value {
            view::Leaf::Int => try_leaf::<Int>(data, name, depth, optional),
            view::Leaf::Float => try_leaf::<Float>(data, name, depth, optional),
            view::Leaf::Str => try_leaf::<Str>(data, name, depth, optional),
            view::Leaf::Num => try_leaf::<Num>(data, name, depth, optional),
        };
        result
            .err()
            .map(|error| format!("{handle:?} does not read: {error}"))
    })
}

/// How a spec's view fares on one input. `None` when the read fails, since a
/// handle promises nothing then.
fn check_read(spec_text: &[u8], input: &[u8], params: &Params) -> Option<Result<(), String>> {
    let checked = check_spec(spec_text, "spec.iris").ok()?;
    let view = view::view(&checked.analysis).unwrap();
    let spec = Spec::parse(spec_text, "spec.iris").ok()?;
    let data = spec.read(input, "input", params).ok()?;
    Some(match violation(&view, &data) {
        Some(message) => Err(message),
        None => Ok(()),
    })
}

fn params(json: Option<&str>) -> Params {
    json.map_or_else(Params::new, |text| protocol::parse_params(text).unwrap())
}

/// Specs and inputs picked to break a promise: empty repeats, missing rows,
/// parameter overrides, a chosen instance, and lookups across arms and frames.
const HARD_CASES: &[(&str, &str, Option<&str>)] = &[
    ("n:int repeat n {let s=\"text\"}", "0", None),
    ("n:int repeat n {let f=1.5 let g=f}", "0", None),
    ("n:int repeat n {repeat n {x:float a[2]:word}}", "0", None),
    ("n:int a[2]:int repeat n {let row=a}", "0 1 2", None),
    ("n:int a[2]:int repeat n {let row=a}", "1 1 2", None),
    (
        "repeat 2 {k:int if k {list a:int until eol}}",
        "0\n0\n",
        None,
    ),
    ("repeat 2 {k:int if k {a[2][3]:float}}", "0 0", None),
    ("repeat 1 {if 0 {repeat 2 {z:int}}}", "", None),
    ("repeat 1 {if 0 {z[1]:word} a:int}", "7", None),
    ("repeat 2 {x:num}", "1 2.5", None),
    ("repeat 2 {x:num}", "1 2", None),
    ("a[2]:num b[2][1]:num", "1 2.5 1 2.5", None),
    ("x:num let y=-x", "2.5", None),
    ("n:int a[n]:num", "0", None),
    ("n:int repeat n {x:num}", "0", None),
    ("x:int if x {y:int} else {y:float}", "0 1.5", None),
    (
        "repeat 2 {k:int if k {let v=1} else {let v=1.5}}",
        "1 0",
        None,
    ),
    (
        "param p=1 param q=1.5 param r=\"s\"",
        "",
        Some(r#"{"p":"2","q":"3","r":"4"}"#),
    ),
    ("a[2]:int param keep=a", "1 2", None),
    ("a[2]:int param keep=a", "1 2", Some(r#"{"keep":"7"}"#)),
    ("param open", "", Some(r#"{"open":"text"}"#)),
    ("repeat 2 {v:int param row=v+1}", "1 3", None),
    (
        "instances {name:word n:int if n {x:int}}",
        "a 0\nb 1 5",
        Some(r#"{"instance":"2"}"#),
    ),
    (
        "instances {name:word n:int if n {x:int}}",
        "a 0\nb 1 5",
        Some(r#"{"instance":"a"}"#),
    ),
    ("x:int instances {x:word}", "1 a", None),
    (
        "instances {sections {\"A {n:int}\" \"B\" optional {let y=n}}}",
        "A 1\nB\n",
        None,
    ),
    (
        "sections {\"A\" {let y=x} \"B {x:word}\" optional}",
        "B s\nA\n",
        None,
    ),
    (
        "sections {\"A {x:int}\" optional \"B\" optional last}",
        "B\n",
        None,
    ),
    (
        "sections until \"END\" store e {\"A\" optional}",
        "A\n",
        None,
    ),
    (
        "sections until \"END\" store e {\"A\" optional}",
        "END",
        None,
    ),
    ("repeat until \"END\" store _ {x:int}", "1 END", None),
    (
        "repeat 2 {repeat until \"END\" store _ {x:int}}",
        "END END",
        None,
    ),
    (
        "repeat 2 {sections until \"END\" store e {\"A\" optional}}",
        "END END",
        None,
    ),
    ("x:word let y=x x:int", "a 1", None),
    ("let x=1 repeat 2 {let y=x let x=\"s\"} let z=x", "", None),
    ("x:int repeat 0 {x:word}", "1", None),
    ("repeat 0 {if 0 {x:int}}", "", None),
    ("a[1][1]:float let r=a[0] let e=a[0][0]", "1.5", None),
];

#[test]
fn typed_handles_read_hard_cases() {
    for (spec, input, json) in HARD_CASES {
        let outcome = check_read(spec.as_bytes(), input.as_bytes(), &params(*json));
        match outcome {
            Some(Ok(())) => {}
            Some(Err(message)) => panic!("{spec}: {message}"),
            None => panic!("{spec} on {input:?} must read"),
        }
    }
}

#[test]
fn typed_handles_read_every_corpus_case() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let roots = [repo.join("conformance/cases/0.1"), repo.join("formats")];
    let mut reads = 0;
    let mut violations = Vec::new();
    for path in discover(&roots, None).unwrap() {
        let case = Case::load(&path, "0.1").unwrap();
        if case.operation != Operation::Read {
            continue;
        }
        let spec = fs::read(&case.spec).unwrap();
        let input = fs::read(case.input.as_ref().unwrap()).unwrap();
        let params = params(case.params.as_deref());
        match check_read(&spec, &input, &params) {
            Some(Ok(())) => reads += 1,
            Some(Err(message)) => violations.push(format!("{}: {message}", case.display_path)),
            None => {}
        }
    }
    println!(
        "Views on corpus reads: {reads} reads, {} violations",
        violations.len()
    );
    assert!(reads >= 340, "too few reads: {reads}");
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}

#[test]
fn typed_handles_read_generated_cases() {
    let mut reads = 0;
    let mut violations = Vec::new();
    for index in 0..GENERATED_CASES {
        let case = differential::generate(1, index);
        let params = params(case.params.as_deref());
        match check_read(&case.spec, &case.input, &params) {
            Some(Ok(())) => reads += 1,
            Some(Err(message)) => violations.push(format!(
                "case {index}: {message}\n{}",
                String::from_utf8_lossy(&case.spec)
            )),
            None => {}
        }
    }
    println!(
        "Views on generated reads: {reads} reads, {} violations",
        violations.len()
    );
    assert!(reads >= 1000, "too few reads: {reads}");
    assert!(violations.is_empty(), "{}", violations.join("\n"));
}
