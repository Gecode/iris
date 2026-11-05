# iris-driver

The conformance driver of the Rust reference implementation. It implements the
[driver contract](../../conformance/README.md) with `parse`, `read`, `catalog`
and `dzn`, and the tests in this crate run the whole suite through it. It is a
tool for this repository, so it is neither published nor part of the release
archives; `iris` is the command for users.

```sh
cargo run --locked -p iris-driver -- parse path/to/spec.iris --version 0.1
cargo test --locked -p iris-driver --test read_conformance -- --nocapture
```

`iris-cli-under-test` is the `iris` command for the tests that compare the two.
