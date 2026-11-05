//! The documented types of `Data::get`, and the misuse that must not compile.
#[test]
fn handle_types() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/build-ui/pass/*.rs");
    cases.compile_fail("tests/build-ui/fail/*.rs");
}
