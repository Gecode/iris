//! Compile-fail tests for `iris!`. Bless changed messages with
//! `TRYBUILD=overwrite cargo test --locked -p gecode-iris --test macro_errors`.
//!
//! The snapshots record the stable compiler, which marks the whole string
//! literal. A nightly compiler marks the error's position inside it instead.
#[test]
fn invalid_invocations_fail_to_compile() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
