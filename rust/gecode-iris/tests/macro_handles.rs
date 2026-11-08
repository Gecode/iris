//! Typed access through the handle fields of `iris!`, and the misuse that must
//! not compile. Bless changed messages with
//! `TRYBUILD=overwrite cargo test --locked -p gecode-iris --test macro_handles`.
#[test]
fn handle_fields_are_typed() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/macro-handles/pass/*.rs");
    cases.compile_fail("tests/macro-handles/fail/*.rs");
}
