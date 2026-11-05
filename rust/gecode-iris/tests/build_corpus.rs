//! Specs that static types would refuse, built with the dynamic forms. Each
//! result matches its conformance case's expected data or error.
use gecode_iris::build::{
    D1, DynField, Field, Int, Maybe, SpecBuilder, Str, count, defined, discard, float, int, lit,
    max, min, size, sum, word,
};
use gecode_iris::{Data, Failure, Params, Spec};
use iris_conformance::canonical::{Outcome, OutcomeKind};
use iris_conformance::case::Case;
use std::{fs, path::Path};

fn case(path: &str) -> Case {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases/0.1");
    Case::load(&root.join(path), "0.1").unwrap()
}

fn read(spec: &Spec, case: &Case) -> Result<Data, Failure> {
    let input = fs::read(case.input.as_ref().unwrap()).unwrap();
    spec.read(&input, "input.bin", &Params::new())
}

fn outcome(result: &Result<Data, Failure>) -> Outcome {
    match result {
        Ok(data) => Outcome::decode(OutcomeKind::Data, &data.to_json().unwrap(), "0.1").unwrap(),
        Err(Failure::Language(error)) => {
            Outcome::decode(OutcomeKind::Error, &error.to_json().unwrap(), "0.1").unwrap()
        }
        Err(other) => panic!("unexpected failure: {other}"),
    }
}

#[test]
fn lookup_before_definition() {
    let case =
        case("spec-examples/08-expressions-and-checks--expressions-lookup-before-definition");
    let mut b = SpecBuilder::unnamed();
    let missing: Field<Int> = b.forward("missing");
    b.let_("v", missing);
    b.let_(missing, lit(1));
    let spec = b.build().unwrap();
    assert_eq!(outcome(&read(&spec, &case)), case.expected);
}

#[test]
fn comparison_results() {
    let case = case("spec-examples/08-expressions-and-checks--expressions-comparison-results");
    let mut b = SpecBuilder::unnamed();
    b.let_("exact", lit(9007199254740993i64).eq(9007199254740992i64));
    b.let_("rounded", lit(9007199254740993i64).eq(9007199254740992.0));
    b.let_("same", lit("a").eq("a"));
    b.let_("mixed_equal", lit("1").dynamic().eq(1));
    b.let_("mixed_unequal", lit("1").dynamic().ne(1));
    b.let_("unicode", lit("\u{e9}").eq("e\u{301}"));
    let spec = b.build().unwrap();
    assert_eq!(outcome(&read(&spec, &case)), case.expected);
}

#[test]
fn discard_and_redefine() {
    let case = case("spec-examples/06-types-and-values--values-discard-redefine");
    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    b.read("y", word());
    b.read(discard(), int());
    let x: DynField = b.read(x, float().dim(2));
    b.list(discard(), int().until(0));
    let spec = b.build().unwrap();
    let result = read(&spec, &case);
    assert_eq!(outcome(&result), case.expected);
    let data = result.unwrap();
    let x = data.get(x).unwrap();
    assert_eq!(x.as_floats().unwrap(), &[2.0, 3.0]);
}

#[test]
fn a_name_left_undefined() {
    let case = case("expressions/control-1");
    let mut b = SpecBuilder::unnamed();
    let n = b.read("n", int());
    let w: Field<Str, gecode_iris::build::D0, Maybe> = b.forward("w");
    b.if_(n.gt(1), |t| {
        t.read(w, word());
    })
    .else_(|e| {
        e.let_(w, lit("none"));
    });
    let m = b.let_("m", n * 3 + 1);
    let a: Field<Int, D1> = b.read("a", int().dim(2));
    b.let_("s", sum(a));
    b.check((max(a) - min(a)).eq(2)).message("spread");
    let zz = b.undefined("zz");
    b.check(count(a).eq(2) & defined(m) & !defined(zz));
    b.check((lit(7) / 2).eq(3) & (lit(7) % 2).eq(1));
    let spec = b.build().unwrap();
    let input = fs::read(case.input.as_ref().unwrap()).unwrap();
    let result = spec.read(&input, "input.txt", &Params::new());
    assert_eq!(outcome(&result), case.expected);
    assert_eq!(result.unwrap().get(w), Some("x"));
}

#[test]
fn store_underscore_lookup() {
    let case = case("spec-examples/08-expressions-and-checks--expressions-store-underscore-lookup");
    let mut b = SpecBuilder::unnamed();
    let stored: Field<Str> = b.forward("_");
    b.repeat_until("END").store(stored);
    b.repeat(1, |r| r.repeat_until("STOP").store(stored).body(|_| {}));
    b.check(stored.eq("END"));
    let spec = b.build().unwrap();
    let result = read(&spec, &case);
    assert_eq!(outcome(&result), case.expected);
    assert_eq!(result.unwrap().get(stored), "END");
}

#[test]
fn store_underscore_records() {
    let case =
        case("spec-examples/08-expressions-and-checks--expressions-store-underscore-records");
    let mut b = SpecBuilder::unnamed();
    let stored: Field<Str> = b.forward("_");
    let record = b.repeat_until("END").store(stored).body(|r| {
        r.as_name("_");
        r.record()
    });
    let n = b.let_("n", size(record));
    let spec = b.build().unwrap();
    let result = read(&spec, &case);
    assert_eq!(outcome(&result), case.expected);
    let data = result.unwrap();
    assert_eq!(data.get(stored), "END");
    assert_eq!(data.get(n), 0);
}

#[test]
fn underscore_left_undefined() {
    let text = Spec::parse(b"check !defined(_)", "spec").unwrap();
    let mut b = SpecBuilder::unnamed();
    let underscore = b.undefined("_");
    b.check(!defined(underscore));
    let spec = b.build().unwrap();
    assert_eq!(spec.source(), "check !defined(_)\n");
    assert_eq!(
        outcome(&spec.read(b"", "input.bin", &Params::new())),
        outcome(&text.read(b"", "input.bin", &Params::new())),
    );
}

/// The dynamic forms of the Rust section of `docs/builders.md`.
#[test]
fn dynamic_forms_of_the_guide() {
    let mut s = SpecBuilder::unnamed();
    let x: Field<Int> = s.forward("x"); // the annotation gives the type
    s.if_(defined(x), |t| {
        t.let_("y", x + 1); // use before the definition
    });
    s.read(x, int()); // the first definition binds it
    let again: DynField = s.read(x, float()); // redefined: use the returned handle
    s.check(lit("1").dynamic().ne(1));
    let zz = s.undefined("zz");
    s.check(!defined(zz));
    let end: Field<Str> = s.forward("_"); // `store _` keeps an entry named `_`
    s.repeat_until("END").store(end);

    let spec = s.build().unwrap();
    let data = spec.read(b"1 2.5 END", "input", &Params::new()).unwrap();
    assert_eq!(data.get(again).unwrap().as_float().unwrap(), 2.5);
    assert!(data.try_get(x).is_err(), "the old handle is stale");
    assert_eq!(data.get(end), "END");
}
