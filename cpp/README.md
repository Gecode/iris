# cpp

The C++23 library `iris` parses and checks reader specs and reads data
files with them. It is a native port of Gecode's implementation
(`gecode/instance/spec.cpp`, `resolve.cpp`, `reader.cpp`, `eval.cpp`,
`data.cpp` and `impl.hpp` at commit `5e7bbd5c26b0`) that does not depend on
Gecode, built with CMake. It is designed so that Gecode 7 can adopt it.
`PLAN.md` §8.4 describes the library; milestone M4c builds it.

The ported files keep Gecode's copyright notice. Like Gecode and the rest of
this repository, the library is released under the [MIT License](../LICENSE).

```cpp
#include <iris/iris.hpp>

auto spec = iris::Spec::parse(text, "sat.iris"); // throws iris::Error
auto checked = iris::Spec::try_parse(text);      // std::expected<Spec, Error>

iris::Data data = spec.read("sat.cnf", iris::Params{{"k", "3"}});
std::int64_t n = data.integer("n");
std::span<const std::int64_t> clause = data.ints("clause", {0});
```

`iris::Error` carries the error code, the message, the reader-spec and input
locations, the context, the record path of a failed check, and `what()`,
which is Gecode's `origin:line: message` envelope. `Spec::read_text` reads
input from memory, and `try_read` and `try_read_text` return the error
instead of throwing it. `iris::Data` holds the entries in definition order;
its accessors throw `iris::AccessError` on misuse, such as an unknown name
or a value of another type.

`Spec::catalog` reads every instance of a file and returns the catalog of
Specification §9.6: the entries `instances`, `name` and `failed`.
`catalog_text`, `try_catalog` and `try_catalog_text` mirror the read
functions. `Data::dzn()` returns the entries as MiniZinc data, in the bytes
of appendix B. Reading and cataloging enforce the work budgets of chapter
10.

## Requirements

- A C++23 compiler and standard library. Apple Clang 21 and GCC 14 are
  tested.
- CMake 3.28 or newer. `uvx --from cmake==4.4.4 cmake` runs a pinned one.
- Ninja, for the presets.
- Stable Rust, for the conformance test, which runs the shared runner in
  `rust/`.

## Build and test

From `cpp/`, the `dev` preset configures a Debug build in `build/dev` with
warnings as errors and writes `compile_commands.json`:

```sh
cmake --preset dev
cmake --build --preset dev
ctest --preset dev
```

`ctest` runs seven tests. `spec-api` and `data-api` check the public API;
`data-api` also checks the driver's JSON of data, and both outputs under
every rounding mode. `budget` checks the exact budget limits that no
conformance case reaches. `parse-conformance`, `read-conformance`,
`catalog-conformance` and `dzn-conformance` run the driver `iris-driver` on
every case of their operation in `conformance/cases/0.1` and `formats`, and
fail unless all of them pass. Run one alone with, for example, `ctest
--preset dev -R read-conformance`. The first conformance run builds the
runner with Cargo.

To run the runner by hand, from the repository root:

```sh
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$PWD/../cpp/build/dev/iris-driver\"" ../conformance/cases/0.1)
(cd rust && cargo run --locked -p iris-conformance -- run --driver "\"$PWD/../cpp/build/dev/iris-driver\"" ../formats)
```

The driver implements the `parse`, `read`, `catalog` and `dzn` operations.
`catalog` writes JSON, or dzn with `--format dzn`.

`-DIRIS_BUILD_TESTS=OFF` builds the library alone, and
`-DIRIS_CONFORMANCE_TESTS=OFF` drops the conformance test.

## Sanitizers

From the repository root:

```sh
cmake -S cpp -B cpp/build/asan -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_COMPILE_WARNING_AS_ERROR=ON \
  '-DCMAKE_CXX_FLAGS=-fsanitize=address,undefined -fno-sanitize-recover=all -fno-omit-frame-pointer' \
  '-DCMAKE_EXE_LINKER_FLAGS=-fsanitize=address,undefined'
cmake --build cpp/build/asan
ctest --test-dir cpp/build/asan --output-on-failure
```

## Format and lint

The clang-format output is the style. From the repository root, with the
tools pinned through PyPI:

```sh
git ls-files 'cpp/*.hpp' 'cpp/*.cpp' | xargs uvx clang-format==22.1.8 --dry-run --Werror
git ls-files 'cpp/*.cpp' | xargs uvx clang-tidy==22.1.8 -p cpp/build/dev --quiet
```

Replace `--dry-run --Werror` with `-i` to rewrite the files. clang-tidy reads
`cpp/build/dev/compile_commands.json`, so build the `dev` preset first. On
macOS, run `export SDKROOT="$(xcrun --show-sdk-path)"` first, so that the
clang-tidy wheel finds the standard library headers. `.clang-tidy` treats
every warning as an error and states why each dropped check is dropped.

## Linux

To build and test with GCC 14 on Ubuntu 24.04 in a container, from the
repository root:

```sh
docker run --rm -v "$PWD":/iris:ro -w /iris ubuntu:24.04 bash -c '
  set -eo pipefail
  apt-get update && apt-get install -y --no-install-recommends gcc g++-14 cmake ninja-build ca-certificates curl
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal && . "$HOME/.cargo/env"
  CXX=g++-14 cmake -S cpp -B /tmp/build -G Ninja -DCMAKE_COMPILE_WARNING_AS_ERROR=ON
  cmake --build /tmp/build
  CARGO_TARGET_DIR=/tmp/cargo ctest --test-dir /tmp/build --output-on-failure'
```

The `gcc` package provides the `cc` that Cargo links build scripts with.
The container runs all four conformance tests too. The cases in
[`libstdcxx-skips.txt`](../conformance/adapters/gecode/libstdcxx-skips.txt),
which the Gecode reference reads or prints wrongly with libstdc++, must pass
there as they do with libc++.

## Layout

- `include/iris/`: the public headers. `iris.hpp` includes the others:
  `spec.hpp`, `data.hpp`, `params.hpp` and `error.hpp`.
- `src/`: the library. `lexer.cpp`, `parser.cpp` and `resolve.cpp` port
  Gecode's lexer, parser and name resolution; `ast.hpp` ports `impl.hpp`.
  `reader.cpp` ports `reader.cpp`, the interpreter that reads input;
  `eval.cpp` and `eval.hpp` port the expression evaluator and checks, and
  `input.hpp` the input cursor. `value.hpp` and `data.cpp` port the values
  of `impl.hpp` and `data.cpp`, and `dzn.cpp` the dzn output of
  `data.cpp`. `budget.hpp` holds the work budgets of Specification chapter
  10. `text.cpp` checks UTF-8, and `numbers.cpp` converts number literals.
  The entry points in `spec.cpp` round to nearest whatever the caller's
  rounding mode, and `dzn.cpp` spells floats with integer arithmetic.
- `driver/`: `iris-driver`, the conformance driver. It speaks the protocol
  in [`conformance/README.md`](../conformance/README.md).
- `tests/`: the API and budget tests, and `conformance.cmake`, which
  selects the cases of one operation and runs them in batches.
