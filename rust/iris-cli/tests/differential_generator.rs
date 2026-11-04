//! Checks that the differential fuzzer's `generate` strategy mostly writes
//! valid reader specs with input they read, so that a run reaches the
//! reader rather than stopping at parse errors.

use std::collections::BTreeMap;

use iris_conformance::differential;
use iris_core::{Failure, Params, Spec};

const SEED: u64 = 7_4006;
const CASES: u64 = 400;
const MINIMUM_VALID_PERCENT: u64 = 80;
const MINIMUM_READ_PERCENT: u64 = 40;

#[test]
fn generated_specs_are_mostly_valid_and_read() {
    let mut valid = 0;
    let mut read = 0;
    let mut codes = BTreeMap::<String, u64>::new();
    for index in 0..CASES {
        let case = differential::generate(SEED, index);
        let checked = iris_cli::check_spec(&case.spec, "generated.iris");
        let spec = Spec::parse(&case.spec, "generated.iris");
        assert_eq!(
            checked.is_ok(),
            spec.is_ok(),
            "check and parse disagree on case {index}"
        );
        let spec = match spec {
            Ok(spec) => spec,
            Err(failure) => {
                *codes.entry(failure_code(&failure)).or_default() += 1;
                continue;
            }
        };
        valid += 1;
        let params = params(case.params.as_deref());
        match spec.read(&case.input, "generated.txt", &params) {
            Ok(_) => read += 1,
            Err(failure) => *codes.entry(failure_code(&failure)).or_default() += 1,
        }
    }
    println!("generated {CASES} cases: valid={valid} read={read} failures={codes:?}");
    assert!(
        valid * 100 >= CASES * MINIMUM_VALID_PERCENT,
        "only {valid} of {CASES} generated specs are valid: {codes:?}"
    );
    assert!(
        read * 100 >= CASES * MINIMUM_READ_PERCENT,
        "only {read} of {CASES} generated cases read: {codes:?}"
    );
}

fn params(json: Option<&str>) -> Params {
    let mut params = Params::new();
    let Some(json) = json else {
        return params;
    };
    let pairs: BTreeMap<String, String> =
        serde_json::from_str(json).expect("generated params are a string-to-string object");
    for (name, value) in pairs {
        params.set(name, value);
    }
    params
}

fn failure_code(failure: &Failure) -> String {
    match failure {
        Failure::Language(error) => error.code.as_str().into(),
        other => format!("{other:?}"),
    }
}
