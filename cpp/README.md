# cpp

The C++23 library `iris` parses and checks reader specs and reads data
files with them. It is a native port of Gecode's implementation
(`gecode/instance/spec.cpp`, `resolve.cpp`, `reader.cpp`, `eval.cpp`,
`data.cpp` and `impl.hpp` at commit `599f977b7855`) that does not depend on
Gecode, built with CMake. It is designed so that Gecode 7 can adopt it.
`PLAN.md` §8.4 describes the library; milestone M4c builds it. The
[C++ guide](https://iris.gecode.dev/using/cpp/) on the site shows how to use
it, and its [API reference](https://iris.gecode.dev/using/cpp/api/) lists
every public class and method.

The ported files keep Gecode's copyright notice. Like Gecode and the rest of
this repository, the library is released under the [MIT License](../LICENSE).

```cpp
#include <iris/iris.hpp>

auto spec = iris::Spec::parse(text, "sat.iris"); // throws iris::Error
auto checked = iris::Spec::try_parse(text);      // std::expected<Spec, Error>
auto loaded = iris::Spec::load("sat.iris");      // or try_load

iris::Data data = spec.read("sat.cnf", {{"k", "3"}});
std::int64_t n = data.integer("n");
std::span<const std::int64_t> clause = data.ints("clause", {0});
auto r = spec.try_read("sat.cnf");               // std::expected<Data, Error>
```

Every operation that throws `iris::Error` has a `try_` variant that returns
it in a `std::expected` instead: `parse`, `load`, `read`, `read_text`,
`catalog` and `catalog_text`. A file that cannot be opened throws
`std::system_error` from both variants. `Spec::read_text` and
`catalog_text` read input from memory.

`iris::Error` carries the error code, the message, the reader-spec and input
locations, the excerpt of the input line, the context, the record path of a
failed check, the earlier rejection in the same instance that may explain
the error, and for a spec from the spec builder the host call site of the
statement it belongs to. `details()` returns them as one
`iris::ErrorDetails`, from which an error can be rebuilt. `what()` renders
the diagnostic of Specification §11.1: `origin:line:column: message`, then
the excerpt with a caret, `while reading:`, `spec:`, `call site:` and
`after an earlier error in the instance:` lines when the error has them.

`iris::Data` holds the entries in definition order. Its accessors take a
name and a path of indices, such as `data.ints("clause", {3})`, and throw
`iris::AccessError` on misuse, such as an unknown name or a value of another
type. `find` and `iris::ValueRef` (`type`, `depth`, `size`, `at`) inspect
values without misuse. Rows are `std::span`s, and strings
`std::string_view`s, that stay valid while a copy of the `Data` exists.

`Spec::catalog` reads every instance of a file and returns the catalog of
Specification §9.6: the entries `instances`, `name` and `failed`.
`Data::dzn()` returns the entries as MiniZinc data, in the bytes of
appendix B. Reading and cataloging enforce the work budgets of chapter 10.

Typed handles (`iris::Field<V, D, P>` in `handle.hpp`) name an entry with
its value type, depth and presence, and `data.get(handle)` returns the entry
as the matching view: `std::int64_t`, `std::span<const std::int64_t>`,
`iris::Array<std::int64_t, 2>`, or a `std::optional` of one of them.
`params.set(handle, value)` sets a parameter through its handle with a
value of the handle's type, formatted as Specification §9.2 converts it.

`<iris/build.hpp>` is the spec builder. `iris::build::SpecBuilder` writes a
reader spec from C++ statements in nested lambdas, which return handles.
Expressions use the operators, `pattern("{} {}", a, b)` checks its
placeholders at compile time, and `build()` prints the spec and parses it.
Iris `a ^ b` is `pow(a, b)`. Where the C library's `::pow` is declared, as
MSVC's standard headers declare it, `using namespace iris::build;` makes an
unqualified `pow` ambiguous, so add `using iris::build::pow;` beside it.
The same holds for `bind`, the `{x:type}` part of a pattern, where the
socket function `::bind` of `<sys/socket.h>` or `<winsock2.h>` is declared:
add `using iris::build::bind;`.
On macOS, `<ApplicationServices/ApplicationServices.h>` declares a QuickDraw
type `Pattern`: add `using iris::build::Pattern;`. Put each of these
using-declarations inside a function or namespace, since at global scope it
collides with the global declaration.
A mistake throws `iris::build::BuildError`, with the `std::source_location`
of the statement that made it. [`docs/builders.md`](../docs/builders.md)
describes the design.

## Allocation

Every allocation the library makes comes from a `std::pmr::memory_resource`.
`Spec::parse` and `Spec::load` take one as their last argument, the default
resource unless given; parsing, reading and cataloging with the spec, and
the data they return, allocate from it, as does the string `Data::dzn()`
returns. `iris::Params` takes its own:

```cpp
std::pmr::synchronized_pool_resource memory;
auto spec = iris::Spec::parse(text, "sat.iris", memory);
iris::Params params({{"k", "3"}}, memory);
iris::Data data = spec.read_text(input, "sat.cnf", params);
```

The resource must outlive the spec, the parameters and the data, and be
thread-safe if threads read with one spec at the same time. Each operation
holds its resource for the thread that runs it, so concurrent reads with
different specs do not mix their resources. The exceptions are deliberate:
`iris::Error`, `iris::AccessError` and `std::system_error`, and their
strings, use the global heap, as they leave the library, and reading a file
opens it with the C library, whose `FILE` may come from `malloc`. With
`std::pmr::new_delete_resource()`, the library allocates through
`std::allocator`, which allocates as that resource does.

## Install and use

Each release publishes a source archive on the
[GitHub releases page](https://github.com/Gecode/iris/releases), tagged
`cpp-vVERSION`: `iris-cpp-VERSION.tar.gz`, with its SHA-256 checksum in
`iris-cpp-VERSION.tar.gz.sha256`. The archive has one top-level directory,
`iris-cpp-VERSION/`, which holds the library's headers and sources, its
`CMakeLists.txt`, the template of the package configuration, the `Doxyfile`,
this README, the changelog and the license. It leaves out the tests, the
conformance driver, the fuzz targets, the examples and the presets, so it
needs no Rust and builds no tests.

A project can build the library with its own build through FetchContent of
the archive, checked against the hash in the `.sha256` file of the release
(the hash below is a placeholder):

```cmake
include(FetchContent)
FetchContent_Declare(iris
  URL https://github.com/Gecode/iris/releases/download/cpp-v0.1.0/iris-cpp-0.1.0.tar.gz
  URL_HASH SHA256=0000000000000000000000000000000000000000000000000000000000000000)
FetchContent_MakeAvailable(iris)
target_link_libraries(app PRIVATE iris::iris)
```

Or it can build and install the library once, and find the installed CMake
package. From the extracted archive, or from `cpp/` in a checkout of this
repository:

```sh
cmake -S . -B build/release -G Ninja -DCMAKE_BUILD_TYPE=Release -DIRIS_BUILD_TESTS=OFF
cmake --build build/release
cmake --install build/release --prefix /opt/iris
```

The prefix then holds the headers in `include/iris`, the library in `lib`,
and `lib/cmake/iris` with `iris-config.cmake`, `iris-config-version.cmake`
and the exported target. A project finds it with
`-DCMAKE_PREFIX_PATH=/opt/iris`:

```cmake
find_package(iris 0.1 REQUIRED CONFIG)
target_link_libraries(app PRIVATE iris::iris)
```

The version file accepts a request for the same minor version. A project
can also build the library from a copy of this repository, through
FetchContent or `add_subdirectory`:

```cmake
include(FetchContent)
FetchContent_Declare(iris SOURCE_DIR "${PROJECT_SOURCE_DIR}/third_party/iris/cpp")
FetchContent_MakeAvailable(iris)
target_link_libraries(app PRIVATE iris::iris)
```

A Git source of this repository works the same with `SOURCE_SUBDIR cpp`.

A vendored copy builds no tests and has no install rules unless
`-DIRIS_INSTALL=ON`, which a project that installs targets linking
`iris::iris` needs.

`scripts/cpp-dist.sh VERSION OUTDIR` at the repository root builds the
archive and its checksum from the tree of a commit, `HEAD` unless a third
argument names another, and fails unless `CMakeLists.txt` there declares
`VERSION`. The same commit gives the same bytes. `scripts/check-cpp-dist.sh
ARCHIVE` builds and installs the archive as a project of its own, then builds
and runs `tests/consumer` against the installed copy and through FetchContent
of the archive by URL and hash. [RELEASING.md](../RELEASING.md#c) describes a
release.

## Requirements

- A C++23 compiler and standard library. Apple Clang 21, GCC 14 and
  Clang 20 with GCC 14's libstdc++ are tested; CI also builds with MSVC.
- CMake 3.28 or newer. `uvx --from cmake==4.4.4 cmake` runs a pinned one.
- Ninja, for the presets.
- Stable Rust, for the conformance test, which runs the shared runner in
  `rust/`.
- Doxygen 1.9.8 or newer, for the `iris-docs` target alone.

## Build and test

From `cpp/`, the `dev` preset configures a Debug build in `build/dev` with
warnings as errors and writes `compile_commands.json`:

```sh
cmake --preset dev
cmake --build --preset dev
ctest --preset dev
```

`ctest` runs the tests below. `spec-api` and `data-api` check the public
API; `data-api` also checks the driver's JSON of data, and both outputs
under every rounding mode. `budget` checks the exact budget limits that no
conformance case reaches. `memory` checks the allocation hook: it replaces
the global `operator new` and checks that parsing, loading, reading,
cataloging and rendering with a counting resource allocate from it alone,
also on several threads and with the default resource, and return all of
it. `headers-compile-alone` compiles each public header by itself, and
`headers-name-no-gecode` checks that none names Gecode. `build-api` checks
the spec builder: its scope, provenance, redefinition, printability and
capture checks and their call sites, the text it prints, and the data it
reads, including the jobshop example against every jobshop case.
`build-call-sites` ports `rust/iris-build/tests/call_sites.rs`: the call
site of every statement, of the language errors that `build()` reports,
including those that lie past their statement, and of the read and catalog
failures of a built spec, and none for a failure that names no statement or
for a spec parsed from text.
`build-types` checks with `static_assert` the types that `get`, the
statements and the expressions give, and the values a parameter handle
takes. Each `compiles-NAME` test builds a
file in `tests/compile_fail`, and its `compile-fails-NAME` test builds it
with `IRIS_EXPECT_FAILURE` defined and passes only when the compiler
rejects it with the expected diagnostic: a value of the wrong type or
depth, a parameter value of the wrong type, a pattern whose placeholders do not match its arguments, and an unqualified `pow` or `bind` that a using-directive makes ambiguous with the C library's `::pow` or the socket function `::bind`.
`consumer-install`
installs the build to `build/dev/tests/consumer/prefix`;
`consumer-find-package` then builds and runs the project in
`tests/consumer` against it with `find_package(iris)`, and
`consumer-fetch-content` builds and runs it with FetchContent of this
source tree. Both use this build's compiler and flags. `parse-conformance`, `read-conformance`,
`catalog-conformance` and `dzn-conformance` run the driver `iris-driver` on
every case of their operation in `conformance/cases/0.1` and `formats`, and
fail unless all of them pass. `fuzz-replay-parse` and `fuzz-replay-read`
run the two fuzz targets (see [Fuzzing](#fuzzing)) over every file in
`formats`. `example-read` and the other `example-` tests run the guide's
examples (see [Guide examples](#guide-examples)). `builder-round-trip` runs
the C++ builder round trip of `rust/` with this build's compiler and its
compile and link flags, those of its configuration included: it
generates a builder program with `iris gen builder --lang cpp` for every
accepted spec in `conformance/cases/0.1` and `formats`, checks it with
clang-format and clang-tidy, builds it against this source tree with the
warnings as errors, and compares the spec it prints with the original (see
[gecode-iris-gen](../rust/iris-gen/README.md#the-c-backend)). It needs
clang-format 22.1.8 and is skipped without it; configure with
`-DIRIS_CLANG_FORMAT=PATH` and `-DIRIS_CLANG_TIDY=PATH` to name the tools,
and it skips clang-tidy unless that is 22.1.8 too. `generated-views` runs
the `cpp_views` test of `rust/` in the same way: it prints a view with
`iris gen --lang cpp` for every format and for specs that exercise every
handle type and layout, builds one program that includes them all, pins the
type of `data.get` for each handle with `static_assert`, and compares what
every handle reads with untyped access on each format case (see
[gecode-iris-gen](../rust/iris-gen/README.md#views)). It runs without
clang-format, and checks the views with clang-format and clang-tidy 22.1.8
when they are found. `format-builders` reads every case of every format in
`formats` through the format's builder in `examples/` (see
[Guide examples](#guide-examples)) and fails unless each one passes. Run one alone with, for
example, `ctest --preset dev -R read-conformance`. The first conformance run builds the
runner with Cargo.

To run the runner by hand, from the repository root:

```sh
(cd rust && cargo run --locked -p gecode-iris-conformance -- run --driver "\"$PWD/../cpp/build/dev/iris-driver\"" ../conformance/cases/0.1)
(cd rust && cargo run --locked -p gecode-iris-conformance -- run --driver "\"$PWD/../cpp/build/dev/iris-driver\"" ../formats)
```

The driver implements the `parse`, `read`, `catalog` and `dzn` operations.
`catalog` writes JSON, or dzn with `--format dzn`.

`-DIRIS_BUILD_TESTS=OFF` builds the library alone, and
`-DIRIS_CONFORMANCE_TESTS=OFF` drops the conformance test.
`-DIRIS_CONFORMANCE_CASE_TIMEOUT=SECONDS` sets the runner's limit for one
case, 30 seconds by default; each conformance test may take 60 times that.

## Guide examples

The site's C++ guide shows each file in `examples/`. The build compiles every
`.cpp` file there, and its `example-` test runs it from the repository root,
since the examples read files under `formats/`, and compares its output with
the `.out` file of the same name. The site's check fails when the page and the
files disagree, so a new example needs a `.out` recording and a place in
`site/scripts/cpp-examples.mjs` and on the page. After a change, write a
recording from the root with `cpp/build/dev/tests/iris-example-read >
cpp/examples/read.out`.

`build_NAME.hpp` is the hand-written spec builder of the format `formats/NAME`,
with `-` in the directory's name spelled `_`, and `build_NAME.cpp` a program
that reads a case with it. `iris-format-builder-driver`, from
`tests/format_builder_driver.cpp`, speaks the driver protocol with the format
name as its first argument and answers with the spec of that builder in place
of the case's own. The `format-builders` test runs the runner with it on each
format directory and fails unless every case passes, as judged with the case's
operation and parameters, and unless every format directory has a builder. By
hand, from the root:

```sh
(cd rust && cargo run --locked -p gecode-iris-conformance -- run --driver "\"$PWD/../cpp/build/dev/tests/iris-format-builder-driver\" jobshop" ../formats/jobshop)
```

`view.sh` is the `iris gen --lang cpp` command the guide shows, and the
site's check compares `view.out` with what the CLI prints.

## API reference

The `iris-docs` target runs Doxygen over the public headers and writes HTML
to `build/dev/docs/html`:

```sh
cmake --build --preset dev --target iris-docs
```

`Doxyfile` treats every warning as an error, and an undocumented public
member is a warning. The target fails with a message if CMake found no
`doxygen`. The site build runs the same `Doxyfile`; see the
[site README](../site/README.md#c-guide-and-api-reference).

## Sanitizers

The `asan` preset builds the `dev` configuration in `build/asan` under
AddressSanitizer and UndefinedBehaviorSanitizer, which stop at the first
error, and the `tsan` preset in `build/tsan` under ThreadSanitizer. Both
need GCC or Clang. From `cpp/`:

```sh
cmake --preset asan
cmake --build --preset asan
ctest --preset asan
```

and the same with `tsan`. The conformance tests then run the sanitized
driver, and `memory` reads with one spec on two threads at once, which TSan
checks. The `tsan` test preset sets `TSAN_OPTIONS=halt_on_error=1`, and
both presets allow the slower sanitized driver 300 seconds a case. A build
directory configured before keeps its cache, so add `--fresh` to the first
command to apply the preset to it again. On Linux, use GCC for `tsan`:
Clang links the TSan runtime statically there, and its `operator new`
clashes with the one the `memory` test defines. TSan may also fail at
startup with "unexpected memory mapping" until
`sudo sysctl vm.mmap_rnd_bits=28`.

## Fuzzing

Two libFuzzer targets in `fuzz/` exercise the library under ASan and UBSan:

- `iris-fuzz-parse` parses arbitrary bytes as a reader spec. It renders
  every error, and aborts unless the spec is rejected with
  `E-SPEC-UTF8` exactly when its bytes are not valid UTF-8.
- `iris-fuzz-read` splits its input at the first NUL byte into a reader
  spec and data input, as Rust's `read` target does. It parses the spec,
  then reads and catalogs the input with no parameters, and renders data
  as the driver's JSON and as dzn, and errors as text and as the driver's
  JSON.

libFuzzer comes with LLVM Clang. Apple Clang lacks it, so on macOS use
Homebrew LLVM; on Ubuntu 24.04 install `clang-20` and
`libclang-rt-20-dev`. From `cpp/`:

```sh
export SDKROOT="$(xcrun --show-sdk-path)"           # macOS only
CXX=/opt/homebrew/opt/llvm/bin/clang++ cmake --preset fuzz   # or CXX=clang++-20
cmake --build --preset fuzz
```

The `fuzz` preset builds with `-O1`, assertions, and ASan and UBSan, which
stop at the first error, in `build/fuzz`. Configuring it with another
compiler fails. The Rust seed corpora serve both implementations: write
them once from the repository root with `python3 -I
rust/fuzz/seed-corpus.py`, then fuzz a target from `cpp/`:

```sh
mkdir -p build/fuzz/corpus/parse build/fuzz/artifacts/parse
build/fuzz/fuzz/iris-fuzz-parse build/fuzz/corpus/parse ../rust/fuzz/corpus/parse \
  -dict=fuzz/iris.dict -max_total_time=300 -max_len=8192 -timeout=60 \
  -artifact_prefix=build/fuzz/artifacts/parse/
```

and the same with `read`. New inputs go to the first corpus directory, in
the ignored build directory, so the Rust corpus stays as written.
`fuzz/iris.dict` lists the keywords, types, functions, layouts and
punctuation of the language. Reproduce a failing input with
`build/fuzz/fuzz/iris-fuzz-parse FILE`. Every other build compiles the
targets with `fuzz/replay.cpp` in place of libFuzzer, as
`build/dev/fuzz/iris-fuzz-parse FILE_OR_DIRECTORY...`, which runs them
with any compiler.

### Comparing with Rust

`iris-conformance differential` generates cases, runs them through the Rust
driver and this one, and fails on any difference in outcome. Half the cases
are reader specs generated with matching input; the other half mutate the
conformance cases given as roots. Build the Rust driver in release mode and
the sanitized driver, then from `rust/`:

```sh
cargo build --release --locked -p iris-driver -p gecode-iris-conformance
(cd ../cpp && cmake --preset asan && cmake --build --preset asan --target iris-driver)
ASAN_OPTIONS=abort_on_error=1 UBSAN_OPTIONS=print_stacktrace=1:abort_on_error=1 \
  target/release/iris-conformance differential \
  --driver "\"$PWD/target/release/iris-driver\"" \
  --driver "\"$PWD/../cpp/build/asan/iris-driver\"" \
  --duration 600 --jobs 8 --keep /tmp/iris-differential \
  ../conformance/cases/0.1 ../formats
```

`abort_on_error` matters: without it a sanitizer report exits 1, which the
driver protocol reads as a language error. The run prints its seed and its
counts per strategy, operation and outcome. On a disagreement it reduces
the case, writes `original/` and `reduced/` case directories under `--keep`
with the drivers' output in `drivers.txt`, prints the command that replays
the case alone, and exits 1. See
[`rust/iris-conformance/README.md`](../rust/iris-conformance/README.md#differential-runs).

A crash or disagreement that is a bug here is fixed in `src/`, and its
reduced input becomes a conformance case in the right area of
`conformance/cases/0.1`, which must pass with both drivers and
`check-grammar`. A crash that no case can express becomes a replay input.
If the Rust implementation or the Specification is at fault, report it
rather than changing the reader here.

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

## Continuous integration

[`.github/workflows/cpp.yml`](../.github/workflows/cpp.yml) runs on changes
to `cpp/`, `conformance/`, `formats/`, `rust/` and the workflow. It builds
the `dev` preset and runs `ctest`, including the four conformance tests,
with GCC 14 and Clang 20 on Ubuntu 24.04, Apple Clang on macOS, and MSVC on
Windows; runs the `asan` preset with Clang 20 and the `tsan` preset with
GCC 14 on Ubuntu, where the Apple Clang, MSVC and sanitizer jobs name
clang-format 22.1.8 so that `ctest` runs `builder-round-trip`;
runs the builder round trip and the generated views with GCC 14 and Clang
20, with clang-format and clang-tidy 22.1.8;
runs each fuzz target for two minutes with Clang 20; compares the Rust
driver with the `asan` driver for five minutes, seeded with the run's ID;
and checks the formatting, runs clang-tidy as above and builds `iris-docs`. A failing fuzz or
differential job uploads its failing inputs or reproducers. CMake 4.4.4 and Ninja
come from PyPI through uv. For a `cpp-vVERSION` tag,
[`.github/workflows/cpp-release.yml`](../.github/workflows/cpp-release.yml)
runs this workflow, builds and checks the source archive, and creates the
GitHub release (see [Install and use](#install-and-use)).

## Layout

- `include/iris/`: the public headers. `iris.hpp` includes `spec.hpp`,
  `data.hpp`, `handle.hpp`, `params.hpp` and `error.hpp`; `build.hpp`, the
  spec builder, is included on its own. None depends on Gecode.
- `CHANGELOG.md`: the changes of each release.
- `Doxyfile`: the settings of the API reference.
- `cmake/`: the template of the package's `iris-config.cmake`.
- `src/`: the library. `lexer.cpp`, `parser.cpp` and `resolve.cpp` port
  Gecode's lexer, parser and name resolution; `ast.hpp` ports `impl.hpp`.
  `reader.cpp` ports `reader.cpp`, the interpreter that reads input;
  `eval.cpp` and `eval.hpp` port the expression evaluator and checks, and
  `input.hpp` the input cursor. `memory.hpp` holds the allocation hook:
  the allocator of the library's containers and the scope that selects
  its resource. `value.hpp` and `data.cpp` port the values
  of `impl.hpp` and `data.cpp`, and `dzn.cpp` the dzn output of
  `data.cpp`. `budget.hpp` holds the work budgets of Specification chapter
  10. `text.cpp` checks UTF-8, and `numbers.cpp` converts number literals.
  The entry points in `spec.cpp` round to nearest whatever the caller's
  rounding mode, and `dzn.cpp` spells floats with integer arithmetic.
  `builder/` is the spec builder: `state.hpp` and `state.cpp` hold the
  statements a builder records and the checks made as they are written,
  `builder.cpp` holds the statement methods and maps each emitted line to
  a call site, `check.cpp` runs the checks made at `build()`, and
  `print.cpp` prints the text. `call_sites.hpp` and `call_sites.cpp` hold
  the table of call sites that a built spec keeps and the access that
  attaches it and records the statement an error belongs to.
- `driver/`: `iris-driver`, the conformance driver. It speaks the protocol
  in [`conformance/README.md`](../conformance/README.md). `driver.cpp` answers
  a request with the spec that a loader gives, and `main.cpp` loads the
  request's spec file.
- `fuzz/`: the libFuzzer targets `parse.cpp` and `read.cpp`, the
  dictionary `iris.dict`, and `replay.cpp`, which runs a target without
  libFuzzer.
- `examples/`: the guide's examples and their recorded output, including a
  spec builder for each format.
- `tests/`: the API, builder, budget and memory tests; `compile_fail/`,
  the programs the builder must reject; `conformance.cmake`, which
  selects the cases of one operation and runs them in batches;
  `example.cmake`, which runs an example and compares its output;
  `cpp-programs.cmake`, which runs the builder round trip and the generated
  views of `rust/`; `format_builder_driver.cpp` and `format-builders.cmake`,
  which read the format cases through the format builders;
  `no-gecode.cmake`; and `consumer/`, a project that uses the library.
