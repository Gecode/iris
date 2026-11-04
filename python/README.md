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
data["n_jobs"]  # int
data.ints("machine", 3)  # list[int]
data.array("duration")  # numpy.ndarray if numpy is installed and rectangular
print(data.to_json())
```

## Install

PyPI publication is planned for milestone M7. Once the package is
published, add it to a uv project, install it into an existing environment,
or run the `iris` command without installing it:

```sh
uv add gecode-iris
uv pip install gecode-iris
uvx --from gecode-iris iris --help
```

Until then, build it from a checkout, which needs the Rust toolchain. In a uv
project beside an Iris checkout named `iris`:

```sh
uv add ../iris/python
```

The site's [Python guide](https://iris.gecode.dev/using/python/) shows these
commands; its build checks that they still appear here.

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
- `data.entries` is a tuple of `Entry` named tuples, `(name, value_type,
  depth)`, in the order the names were defined. `value_type` is `"int"`,
  `"float"` or `"string"`.
- `data["name"]` returns an entry's value, copied into Python objects: an
  `int`, `float` or `str` at depth 0, and nested lists at greater depths. The
  type `Value` describes these. `"name" in data` tests for an entry, and
  `data.to_dict()` returns every entry as a `dict` in entry order.
- `data.ints(name, *path)` returns the integer row that `path` selects, as
  `list[int]`. A negative index counts from the end of its row, as in a list.
  The selection must have depth 1 and value type `int`.
- `data.array(name, *path)` returns the array that `path` selects. When numpy
  can be imported and the array is rectangular, the result is a writable
  `numpy.ndarray` of `int64`, `float64` or `str_` elements, with one dimension
  per level; otherwise it is the same nested lists as `data[name]`. numpy is
  not a dependency of the package, and `array` imports it on each call. An
  empty array has size 0 in every dimension, since an empty row does not
  record the widths of its missing children.
- Access errors are built-in exceptions: `KeyError` for an unknown entry,
  `IndexError` for an index outside its row, and `TypeError` for a path
  through a scalar, `array` on a scalar, or `ints` on a selection that is not
  an integer row.
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

## The `iris` command

The package installs an `iris` console script, the same command as the Rust
`iris` binary: `iris check`, `read`, `list`, `fmt`, `lsp` and `conformance`
take the same options and produce the same output and exit statuses (2 for a
usage error, 1 for a spec or instance error). Installing the package into an
environment, or running `uvx --from gecode-iris iris`, is enough to use it
without a Rust toolchain.

The command line is the Rust library function `iris_cli::run`, which the
extension exposes as `gecode_iris._native.run_cli(argv)`. The script,
`gecode_iris._cli:main`, adds no command logic. It flushes Python's
`sys.stdout` and `sys.stderr`, restores the default `SIGINT` handler so that
Ctrl-C stops a long run such as `iris lsp`, and exits with the status that
`run_cli(sys.argv)` returns. The Rust code reads and writes the process's
standard streams directly, and help and usage messages take the program name
from `sys.argv[0]`. `tests/test_cli.py` runs the installed script and compares
its output with the Rust binary's recorded output.

## Layout

`python/` is a Cargo workspace of its own, with its own `Cargo.lock`. Its
crate, `gecode-iris-python`, depends on `rust/gecode-iris` and
`rust/iris-cli` by path and builds the extension module
`gecode_iris._native`. The Python sources are in
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

That command keeps the versions already in the lockfile, but it picks the
newest version of a crate that `python/Cargo.lock` does not list yet. Pin any
such crate that `rust/Cargo.lock` also lists to that version, for example
`cargo update --manifest-path python/Cargo.toml -p toml --precise
1.1.8+spec-1.1.0`.

## Minimum Python version

The package uses the stable ABI (abi3) and supports Python 3.10 and later.
The floor appears in two places: the `abi3-py310` feature of `pyo3` in
`Cargo.toml` decides the ABI, and `requires-python` in `pyproject.toml`
reaches the wheel metadata, since maturin does not derive it.
`tests/test_package.py` fails when they differ. To raise the floor, change
both.

## Build and test

The tools are uv for environments, interpreters, dependencies and running
tools; maturin 1.16.0, through `uvx`, for the extension; ruff for linting and
formatting; ty for type checking; and pyright and mypy for the stub checks.
The `lint` dependency group in `pyproject.toml` pins the exact versions of
ruff, ty, pyright and mypy, and CI installs the same group. The `docs` group
pins pdoc for the [API reference](#api-reference). ruff and ty are
configured in `pyproject.toml`. The Rust crate needs the stable toolchain that `rust/`
uses.

Set up a development environment in `python/`:

```sh
uv venv --python 3.15
uvx maturin@1.16.0 develop --uv
uv pip install --group test --group lint --group docs
uv run --no-sync pytest
```

`maturin develop` builds the extension into `python/gecode_iris/` and
installs the package into `.venv` in editable mode, so Python changes apply
at once. Run it again after a Rust change, and add `--release` before a
conformance run. `--no-sync` keeps `uv run` from creating a `uv.lock` and
reinstalling the package over the development build.

Lint, format and type-check in `python/`, after the setup above, since ty
resolves `pytest` and `pdoc` from `.venv`:

```sh
uv run --no-sync ruff format --check
uv run --no-sync ruff check
uv run --no-sync ty check
```

The package ships `py.typed` and the stub `python/gecode_iris/_native.pyi`
for the compiled module, so type checkers see `Spec` and `Data` with their
full signatures. `tests/test_package.py` fails when the stub and the module
declare different names. The stub names numpy's array type for
`Data.array`; numpy is optional, so `[tool.ty.analysis]` lets ty pass
without it, and treats that type as unknown. ty skips
`tests/typecheck/data_misuse.py`, whose errors are intended.

Run the test suite without numpy and with it. `--with` adds numpy for one
run without installing it into `.venv`; the tests that need numpy are
skipped in the first run:

```sh
uv run --no-sync pytest
uv run --no-sync --with numpy pytest
```

pyright and mypy check the stubs through two files in `tests/typecheck/`.
Both checkers must accept `data_usage.py`, which uses the documented API,
and report an error on each line of `data_misuse.py` that ends in an
`# error:` comment. numpy makes the `Data.array` types precise; without it,
pyright's strict mode reports them as unknown:

```sh
uv run --no-sync --with numpy mypy tests/typecheck/data_usage.py
uv run --no-sync --with numpy pyright tests/typecheck/data_usage.py
uv run --no-sync --with numpy mypy tests/typecheck/data_misuse.py
uv run --no-sync --with numpy pyright tests/typecheck/data_misuse.py
```

The last two commands exit with status 1. Each reports errors on every
marked line and on no other line.

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
uvx maturin@1.16.0 build --release --out target/wheels
uv venv --python 3.10 "$TMPDIR/iris-py3.10"
uv pip install --python "$TMPDIR/iris-py3.10/bin/python" target/wheels/*.whl --group test
"$TMPDIR/iris-py3.10/bin/python" -m pytest
```

`tests/test_cli.py` then runs the `iris` script that the wheel installed into
that environment.

## Guide examples

`examples/` holds the examples of the site's
[Python guide](https://iris.gecode.dev/using/python/), each with its recorded
standard output in a `.out` file. `tests/test_examples.py` runs every `.py`
example with the installed package, and every `.sh` example, which is one
`iris` command line, with the installed console script. Both run from the
repository root, since the examples read files under `formats/`, and the test
compares their output with the recording. An example whose first line is
`# Needs numpy.` runs only when numpy can be imported, so run pytest with and
without numpy as above.

The site renders each file with its recorded output, and its build fails when
a file in `examples/` does not appear in the guide or a `.sh` recording
differs from the Rust CLI. After changing an example, run it from the
repository root and update its `.out` file.

## API reference

The site hosts an API reference at
[`/using/python/api/`](https://iris.gecode.dev/using/python/api/), generated
from the stubs and docstrings by pdoc, which the `docs` group pins. pdoc was
chosen over mkdocstrings because it is a library without a site generator:
`docs/reference.py` renders the package into an HTML fragment with the
templates in `docs/templates/`, and the Astro site places the fragment in a
Starlight page with its own outline and Tailwind styles. mkdocstrings runs
inside MkDocs, a second site with its own theme, navigation and build.

pdoc imports the package and reads a `.pyi` stub only for the module it
documents. `Spec` and `Data` come from `_native` but are documented in the
package, so `reference.py` applies `_native.pyi` to them itself. It also names
the `Value` and `NDArray` aliases in signatures instead of expanding them, and
reads `Entry` and `Error` from the modules that define them, since the
package sets their `__module__` to `gecode_iris`. The output does not depend
on whether numpy is installed.

`npm run build` in `site/` runs `site/scripts/prepare-python-api.mjs`, which
builds a wheel with `uvx maturin@1.16.0 build`, installs it with the `docs`
group into a temporary Python 3.15 environment, and runs `reference.py`. The
output, `site/src/generated/python-api.json`, is an ignored build output. To
render it from the development environment, run
`uv run --no-sync python docs/reference.py OUTPUT.json` in `python/`.

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

## Source distribution

The crates in `rust/` that the package depends on are outside `python/`, and
`maturin sdist` copies them into the source distribution. A crate must
therefore not read files outside its own directory at build time;
`iris-conformance` embeds copies of the two Specification files it needs for
this reason (see [its README](../rust/iris-conformance/README.md)). To check
that the source distribution builds, build a wheel from it outside the
checkout, in `python/`:

```sh
uvx maturin@1.16.0 sdist --out dist
uv build --wheel --out-dir "$TMPDIR/iris-sdist" dist/*.tar.gz
```

## Continuous integration

[`.github/workflows/python.yml`](../.github/workflows/python.yml) runs on
changes to `python/`, `rust/`, `conformance/`, `formats/` and the workflow
itself. It publishes nothing; publishing is milestone M7.

- `lint` installs the `test`, `lint` and `docs` groups and runs the
  formatter, linter and type checks above, the pyright and mypy stub checks,
  and `cargo fmt` and `clippy` for the crate. For `data_misuse.py` it
  compares the lines each checker reports with the lines marked `# error:`.
- `wheel` builds one abi3 wheel per platform with `PyO3/maturin-action` and
  maturin 1.16.0, using
  the committed `Cargo.lock`: manylinux and musllinux for x86_64 and aarch64,
  macOS for arm64 and x86_64, and Windows for x86_64.
- `sdist` builds the source distribution, then builds a wheel from it outside
  the checkout and imports it.
- `test` installs each wheel into a new uv environment, on Python 3.10 and
  3.15, and runs pytest without and with numpy, and the `iris` script. The
  musllinux wheels run in an Alpine container. On Linux x86_64, macOS arm64
  and Windows, Python 3.15 also runs the conformance suite through the
  installed `iris conformance`, against `conformance/cases/0.1` and `formats`.

The [site workflow](../.github/workflows/site.yml) also runs on changes to
`python/`, since its build renders the API reference from the package.
