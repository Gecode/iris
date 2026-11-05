//! Reads one fuzz input as a reader spec and its data input, then renders every
//! result the way the conformance driver does.
//!
//! The bytes before the first NUL byte are the reader spec and the bytes after
//! it are the data input; an input without a NUL byte is a spec with empty
//! data. Specs therefore never contain NUL here, while data may contain any
//! byte. The parse target covers specs with NUL.
//!
//! Reading and cataloging use the default work budgets of Specification §10,
//! so a slow but legal input ends in `E-BUDGET` rather than a timeout.
#![no_main]

use gecode_iris_core::{Data, Failure, Params, Spec};
use libfuzzer_sys::fuzz_target;

const SPEC_ORIGIN: &str = "fuzz.iris";
const INPUT_ORIGIN: &str = "fuzz.txt";

fuzz_target!(|case: &[u8]| {
    let (spec_source, input) = split_case(case);
    match Spec::parse(spec_source, SPEC_ORIGIN) {
        Ok(spec) => {
            let params = Params::new();
            render(spec.read(input, INPUT_ORIGIN, &params));
            render(spec.catalog(input, INPUT_ORIGIN, &params));
        }
        Err(failure) => render_failure(&failure),
    }
});

/// Split a case at its first NUL byte into a reader spec and a data input.
fn split_case(case: &[u8]) -> (&[u8], &[u8]) {
    match case.iter().position(|&byte| byte == 0) {
        Some(separator) => (&case[..separator], &case[separator + 1..]),
        None => (case, &[]),
    }
}

fn render(result: Result<Data, Failure>) {
    match result {
        Ok(data) => {
            data.to_json().expect("canonical JSON renders all data");
            data.to_dzn().expect("writing dzn to a vector cannot fail");
        }
        Err(failure) => render_failure(&failure),
    }
}

fn render_failure(failure: &Failure) {
    if let Failure::Language(error) = failure {
        error
            .to_json()
            .expect("error JSON renders every language error");
    }
    let _caret_message = failure.to_string();
}
