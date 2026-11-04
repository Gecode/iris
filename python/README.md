# python

The Python package `gecode-iris`, imported as `gecode_iris`, binds the Rust
reference implementation with PyO3 and maturin. Python adds no reader
behavior: parsing, reading, errors, canonical JSON and dzn all come from
`iris-core`, so a conformance difference is a binding bug. `PLAN.md` §8.2
describes the package; milestone M4a builds it.

```python
import gecode_iris as iris

spec = iris.Spec.load("jobshop.iris")
data = spec.read("jobshop1.txt", params={"instance": "la01"})
print(data.to_json())
```

## API

- `Spec.parse(text, origin="<string>")` parses a reader spec. A `str` is
  encoded as UTF-8; `bytes` are used as given, so invalid UTF-8 raises
  `E-SPEC-UTF8`. `Spec.load(path)` reads a file and uses the path as the
  origin.
- `spec.read(source, params=None, *, origin=None)` reads one instance, and
  `spec.catalog(...)` lists a file's instances with the same arguments.
  `source` is either input bytes, whose origin defaults to `"<input>"`, or a
  path as `str` or `os.PathLike`. A `str` is always a path, never input text.
  `origin` applies only to bytes.
- `params` maps parameter names to string values, as the Specification
  defines them (§9.1). Keys and values must be `str`; anything else raises
  `TypeError`, with no conversion through `str()`. Use `{"instance": "2"}` or
  `{"instance": "la01"}` to select an instance.
- `Data.to_json()` returns the canonical JSON document, ending in a newline,
  and `Data.to_dzn()` returns the dzn text of the Specification's appendix B.
  python-002 adds element access.
- A language error raises `gecode_iris.Error`, with the attributes `code`,
  `message`, `line`, `column`, `path` (the record path as a tuple) and
  `origin`. The location is the input location when there is one, and
  otherwise the reader spec location; any part may be `None`. `str(error)` is
  the full diagnostic and `error.to_json()` the canonical error document.
  Errors pickle.
- Host failures raise built-in exceptions and have no error code: `OSError`
  subclasses such as `FileNotFoundError` for file access, `ValueError` for a
  source too large to parse, `NotImplementedError` for an unimplemented
  feature, and `RuntimeError` otherwise.

The reader releases the GIL, and one `Spec` can be shared between threads.
The Rust `Spec` must stay on one thread, so the Python `Spec` keeps the spec
source and parses it again for each read, which takes microseconds.

## Layout

`python/` is a Cargo workspace of its own, with its own `Cargo.lock`. Its
crate, `gecode-iris-python`, depends on `rust/gecode-iris` by path and
builds the extension module `gecode_iris._native`. The Python sources are in
`python/python/gecode_iris`, in maturin's mixed layout, and the tests in
`python/tests`.

It does not join the `rust/` workspace because:

- The `cargo build`, `clippy` and `test --workspace` commands and the Rust
  CI would then build the PyO3 crate on every platform, and cargo-dist would
  plan it as a release package.
- A member outside `rust/` would need an unusual pointer back to that
  workspace.
- The wheel builds compile only `python/`, inside manylinux and musllinux
  containers.

`Cargo.lock` started as a copy of `rust/Cargo.lock`, so the crates both
workspaces use have the same versions. `pyproject.toml` sets `locked = true`
for maturin, so every build uses the lockfile as committed. When a crate in
`rust/` gains or updates a dependency, the Python build fails until someone
refreshes the lockfile from the repository root:

```sh
cargo update --manifest-path python/Cargo.toml --workspace
```

## Minimum Python version

The package uses the stable ABI (abi3) and supports Python 3.10 and later.
The floor appears in two places: the `abi3-py310` feature of `pyo3` in
`Cargo.toml` decides the ABI, and `requires-python` in `pyproject.toml`
reaches the wheel metadata, since maturin does not derive it.
`tests/test_package.py` fails when they differ. To raise the floor, change
both.

## Build and test

The tools are uv for environments, interpreters, dependencies and running
tools; maturin, through `uvx`, for the extension; ruff for linting and
formatting; and ty for type checking. ruff and ty are configured in
`pyproject.toml`. The Rust crate needs the stable toolchain that `rust/`
uses.

Set up a development environment in `python/`:

```sh
uv venv --python 3.15
uvx maturin develop --uv
uv pip install --group test
uv run --no-sync pytest
```

`maturin develop` builds the extension into `python/gecode_iris/` and
installs the package into `.venv` in editable mode, so Python changes apply
at once. Run it again after a Rust change, and add `--release` before a
conformance run. `--no-sync` keeps `uv run` from creating a `uv.lock` and
reinstalling the package over the development build.

Lint, format and type-check in `python/`, after the setup above, since ty
resolves `pytest` from `.venv`:

```sh
uvx ruff format --check
uvx ruff check
uvx ty check
```

ty cannot read the compiled `_native` module, and its type stubs arrive with
python-002. Until then `[tool.ty.analysis]` allows that one import to stay
unresolved: ty checks the Python sources and tests, and treats `Spec` and
`Data` as unknown.

Check the Rust crate from the repository root:

```sh
cargo fmt --manifest-path python/Cargo.toml --all -- --check
cargo clippy --manifest-path python/Cargo.toml --all-targets --locked -- -D warnings
```

The crate has no Rust tests; pytest and the conformance suite cover it.

To test the abi3 wheel on the oldest supported Python, build it and install
it into a fresh environment in `python/`. uv downloads a managed Python 3.10
when none is installed:

```sh
uvx maturin build --release --out target/wheels
uv venv --python 3.10 "$TMPDIR/iris-py3.10"
uv pip install --python "$TMPDIR/iris-py3.10/bin/python" target/wheels/*.whl --group test
"$TMPDIR/iris-py3.10/bin/python" -m pytest
```

## Conformance

`tests/conformance_driver.py` is the conformance driver. It speaks the
[driver protocol](../conformance/README.md#four-driver-commands) through the
public API only. Run the suite from the repository root, with `PY` set to the
interpreter of an environment where the package is installed. `-I` keeps the
source tree off `sys.path`, so the installed package is what runs:

```sh
PY="$PWD/python/.venv/bin/python"
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$PY\" -I \"$PWD/../python/tests/conformance_driver.py\"" ../conformance/cases/0.1)
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$PY\" -I \"$PWD/../python/tests/conformance_driver.py\"" ../formats)
```

Every case passes with no `--driver-id` and no exemptions.
