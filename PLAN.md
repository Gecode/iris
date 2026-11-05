# Iris: project plan

Iris, the Gecode instance reading and interpretation system, is the
declarative reader language that Gecode 6.5.0 added as `gecode/instance`
([Gecode/gecode#270](https://github.com/Gecode/gecode/pull/270)). This
repository turns that language into a standalone project: a specification
with its own version, a conformance suite, several implementations, editor
tooling, an agent skill, and a documentation site.

The name is written **Iris** in prose and **`iris`** in code, commands,
package names, and file extensions. It is never written in capitals.

This plan covers the goals, versions, architecture, repository layout, each
deliverable, the order of work, and the decisions made and still open.

---

## 1. Versions

| Language version | What it is | Used by |
|---|---|---|
| **Iris 0.1** | The language of Gecode 6.5.0, written down precisely. No new features; the only deliberate differences are listed in the Specification's appendix C. | Gecode 6.5.0 (its own implementation) |
| **Iris 1.0** | The language developed further, based on what 0.1 shows us. | Gecode 7 |

The first releases of every package are **0.1.0** and implement Iris 0.1.
The packages reach **1.0.0** when Iris 1.0 is finished, together with
Gecode 7.

## 2. Goals

1. **One language, defined in one place.** A normative specification for
   each language version, precise enough for independent implementations to
   agree byte for byte on data and on error locations.
2. **A conformance suite as the contract.** Language-neutral test cases
   that every implementation runs, Gecode included. An implementation
   conforms to a language version when it passes that version's suite.
3. **Implementations people want to use:**
   - Rust crate: the reference implementation, also used by the CLI, LSP,
     formatter, WASM playground, and the Python and TypeScript packages.
   - Python package built on the Rust crate.
   - TypeScript package built on the Rust crate through WebAssembly, for
     Node, browsers, and edge runtimes.
   - Java library for current Java, designed to work well from Kotlin,
     plus a small Kotlin extensions module.
   - C++ library with no Gecode dependency, designed so that Gecode 7 can
     adopt it.
4. **Tooling:** the `iris` CLI, a formatter, a linter if it has useful
   lints (§8.13), a tree-sitter
   grammar, a TextMate grammar, a language server, and editor packages (VS
   Code, JetBrains IDEs, Zed, Neovim, Helix).
5. **An agent skill** that teaches coding agents to write and debug reader
   specs with the CLI.
6. **A documentation site** with a tutorial, the language reference, a
   library of formats, API guides for each language, and an in-browser
   playground.
7. **Brand:** a logo and favicon based on the Gecode colors.

### Non-goals

- New language features in 0.1. Ideas collected while writing the 0.1
  Specification and the format library go into the 1.0 design (§10).
- Binary formats and writing instances. Iris reads text.

---

## 3. Terminology (settle this first)

"Spec" already means the user's reader program in Gecode (`Spec::parse`,
`-spec FILE`). It must not also mean the language document. Proposal:

| Term | Meaning |
|---|---|
| **reader spec** (or just *spec*) | A program in the Iris language, such as `dimacs-cnf.iris`. Matches Gecode's API. |
| **Iris Language Specification** (*the Specification*) | The normative document for one language version in `spec/`. Always capitalized, never shortened to "spec". |
| **instance** / **input** | The text file being read. |
| **data** | The result of reading: named, typed, possibly ragged arrays. |
| **implementation** | A library that parses reader specs and reads inputs. |
| **host** | The program that embeds an implementation (Gecode, a Python script, ...). |

These terms and the domain glossary go into `CONTEXT.md` at the root, and
every document uses them.

File extension for reader specs: **`.iris`**.

---

## 4. Architecture

### 4.1 Implementation strategy (decided)

| Language | Strategy | Why |
|---|---|---|
| Rust | **Reference implementation**, native | Safe, fast, compiles to native and WASM, and one parser serves the reader, LSP, formatter, and playground. |
| Python | **Binding** to the Rust crate (PyO3 + maturin) | Wheels are easy to build, and Python users get Rust speed. |
| TypeScript | **Binding** to the Rust crate through WebAssembly (wasm-bindgen) | One artifact runs in Node, browsers, and edge runtimes, and every fix in Rust reaches TypeScript users. |
| C++ | **Native**, ported from Gecode's implementation | The C++ code exists and is tested, so the port mostly removes Gecode types (`IntArgs`, Gecode heap, `Exception`) and modernizes the API. It is the candidate for Gecode 7. |
| Java/Kotlin | **Native**, pure Java | JVM users avoid native dependencies. A pure-Java jar runs everywhere, including Android, and the JIT makes it fast enough. |

So there are three independent implementations (Rust, C++, Java) plus
Gecode 6.5.0. That makes the Specification honest, and differential testing
between implementations finds bugs that no single implementation finds.

### 4.2 The Rust reference implementation is layered

```
iris-syntax   lexer, lossless CST (rowan), error-tolerant parser, formatter
     │
iris-sema     name resolution, scopes, record names, types and shapes,
     │        diagnostics with codes
     ├── iris-lint   lints and their fixes, if built (§8.13)
     ├── iris-macros the `iris!` proc macro, used through gecode-iris
     │
iris-core     reader, expression evaluator, data model, work budgets,
     │        JSON and dzn output
     ├── iris-cli    `iris` binary
     ├── iris-lsp    language server
     ├── iris-build  spec builder with typed handles
     ├── iris-wasm   wasm-bindgen API for the playground
     ├── iris-js     wasm-bindgen binding for the TypeScript package
     └── (python/)   PyO3 binding
```

`iris-lint` depends on `iris-sema` only, so the reader and the Python
binding do not carry it. The CLI, the LSP, and the WASM crate use it.
`iris-macros` depends on `iris-syntax` and `iris-sema`, never on
`iris-core`: it checks a spec at compile time, and the code it expands to
calls the reader through `gecode-iris`. `iris-build` depends on `iris-core`
and `iris-macros`, for `pattern!`, and `gecode-iris` re-exports it as
`gecode_iris::build`.

The parser must be **lossless and error-tolerant** from the start. The LSP
and formatter need a syntax tree for broken input, and the reader needs
precise spans for error messages. Writing one parser that serves both
avoids keeping two parsers in sync.

From 1.0 on, every implementation supports both language versions (§10.2),
so the language version is a parameter of parsing from the start rather
than something added later.

### 4.3 The data model is part of the Specification

Every implementation exposes the same model, in each language's idiom:

- Data is an ordered list of **entries** (in definition order).
- Each entry has a **name**, a **value type** (`int`, `float`, `string`),
  and a **depth** (0 is a scalar, 1 an array, 2 an array of rows, ...).
- Arrays may be **ragged**. Integers are 64-bit signed.
- Accessors take a **path** of 0-based indices.

The Specification also defines a **canonical JSON encoding** of data and of
errors. The conformance suite compares implementations on it, and the CLI
emits it.

```json
{"iris": "0.1",
 "entries": [
   {"name": "vars",   "type": "int", "depth": 0, "value": 3},
   {"name": "clause", "type": "int", "depth": 2, "value": [[1, -2], [3]]}
 ]}
```

Floats use the shortest round-trip decimal form. The `type` field tells
`3` apart from `3.0`. If non-finite floats are valid tokens (see §6.2),
they are encoded as strings.

### 4.4 Errors get stable codes

Gecode reports errors as messages. To compare errors across
implementations without making exact wording normative, the 0.1
Specification adds **error codes** (for example `E-RANGE`,
`E-SECTIONS-MISSING`, `E-CHECK-FAILED`, `E-BUDGET`). Conformance compares
the **code, line, and column**, plus the record path for check failures.
The Specification gives a recommended message for each code, so
implementations read alike, but the wording is not tested. The Gecode
adapter maps Gecode's messages to codes (§7.3). If Gecode 7 adopts the
Iris C++ library, Gecode gets the codes as well.

---

## 5. Repository layout

One monorepo. The Specification, the suite, and every implementation change
together, and one pull request can update the Specification, add a
conformance case, and fix every implementation.

```
iris/
├── CONTEXT.md               glossary and domain language
├── PLAN.md                  this file
├── docs/adr/                architecture decision records
├── spec/                    the Iris Language Specification (normative)
│   ├── 0.1/                 the Gecode 6.5.0 language
│   │   ├── iris.md          chapters as Markdown, numbered sections
│   │   ├── grammar.ebnf     the formal grammar
│   │   └── errors.md        the error code registry
│   ├── 1.0/                 the language for Gecode 7 (once started)
│   └── schema/              JSON Schemas for data, errors, and test cases
├── conformance/
│   ├── cases/<version>/<area>/<case>/
│   │                        spec.iris, input.txt, params.json,
│   │                        expected.json or error.json
│   └── adapters/gecode/     driver that links Gecode's libgecodeinstance
├── formats/                 library of reader specs for standard formats
│   └── <format>/            <format>.iris, README.md, samples/, expected/
├── rust/                    Cargo workspace (crates from §4.2, and the
│                            conformance runner iris-conformance, §7.2)
├── python/                  PyO3/maturin package
├── typescript/              npm package over rust/iris-js (WebAssembly)
├── cpp/                     C++ library (CMake)
├── java/                    Gradle build: iris (Java), iris-kotlin
├── tree-sitter-iris/        grammar.js, queries/, tests, bindings
├── editors/
│   ├── textmate/            iris.tmLanguage.json (VS Code, Shiki, GitHub)
│   ├── vscode/              extension: grammar plus LSP client
│   ├── jetbrains/           plugin: TextMate grammar plus LSP client
│   ├── zed/                 extension: tree-sitter plus LSP
│   └── nvim/, helix/        configuration snippets and docs
├── skill/iris/              the agent skill (SKILL.md and references)
├── site/                    documentation site (Astro Starlight)
├── brand/                   logo, favicon, palette, usage notes
└── .github/workflows/       CI, one workflow per component plus conformance
```

---

## 6. The Iris 0.1 Specification

### 6.1 Sources

Gecode 6.5.0's behavior defines Iris 0.1. The sources are the Doxygen
reference in `gecode/instance.hh` (`TaskInstanceSpec`), the parser
(`spec.cpp`), name resolution (`resolve.cpp`), the evaluator (`eval.cpp`),
the reader (`reader.cpp`), and the tests (`test/instance.cpp`). Where the
Doxygen text and the code disagree, we record the difference and decide.
Each decision gets an entry in a "Differences from Gecode 6.5.0" appendix.
That appendix should stay short. It holds the deliberate difference on
UTF-8 (§12) and Gecode bugs that are fixed in a 6.5.x release.

### 6.2 What the Doxygen text leaves open

These details are only in the C++ code today. The Specification must pin
each one down, with a conformance case:

- **Lexing the input:** the exact syntax of `int` and `float` tokens
  (exponents, a leading `+`, `inf` and `nan`, hexadecimal), which
  characters count as white space (the code treats `\v` and `\f` as
  white space in some places and not in others), CR LF handling, and
  whether a final newline is required.
- **Positions:** whether columns count bytes, code points, or grapheme
  clusters; how tabs count; how input that is not UTF-8 is handled.
- **Comment and stop prefixes:** the rule that a prefix ending in a letter
  or digit must not be followed by one, applied with Unicode or with ASCII
  only.
- **Work budgets.** There are two: a fixed limit of 2^20 on repeat
  iterations and empty rows that read no input, and an expression budget
  proportional to the input size. The Specification needs either the exact
  formulas or a minimum that implementations must allow. I recommend exact
  formulas, so that every implementation accepts the same programs.
- **Integer semantics:** 64-bit checked arithmetic, truncating division,
  `%` with negative operands, and `^` overflow.
- **Error order:** when one input has several problems, which error is
  reported (the first in reading order, with ties broken by statement
  order).
- **Name scopes:** record names, `each`, nested repeats, and type names.
  The Doxygen prose is careful but informal. The Specification should
  state the rules for scopes and record names as judgments or a table.
- **Layouts:** the exact cell order for each TSPLIB layout, including the
  column-wise variants.
- **`instances`:** how instances are selected, and the exact content of
  catalog results.
- **dzn output:** already well described. Move the description into the
  Specification as a normative appendix.

### 6.3 Host constants

`gecode.int_max` and `gecode.int_min` are Gecode-specific names. In 0.1
they are standard constants with fixed values (±2147483646), exactly as in
Gecode 6.5.0. Neutral names (`int.max`, `i32.max`, `i64.max`) and a general
way for hosts to supply constants are 1.0 topics (§10).

### 6.4 Form and tooling

- Markdown in `spec/<version>/`, rendered by the site under
  `/spec/<version>/`, with stable section numbers for references
  (`§4.3.2`).
- Uses RFC 2119 keywords (MUST, SHOULD, ...).
- The grammar is in `grammar.ebnf`. A CI check confirms that every
  conformance spec parses with it, using a small generated checker or the
  Rust parser in strict mode.
- **Every example in the Specification is tested.** Code blocks tagged
  `iris example` that include input and expected data are extracted into
  conformance cases in CI.
- 0.1 is frozen once released. Corrections are errata, which never change
  what Gecode 6.5.0 accepts.

---

## 7. Conformance suite

### 7.1 Cases

A case is a directory:

```
conformance/cases/0.1/read/ranges/exclusive-upper/
├── spec.iris
├── input.txt          (or input.bin for tests of bad encodings)
├── params.json        optional: {"instance": "la01"}
├── expected.json      canonical data, or
├── error.json         {"code": "E-RANGE", "line": 4, "column": 1}
└── case.toml          optional: title, Specification section, tags
```

There are also cases that only parse a spec (no input) and cases for
`catalog`. Areas follow the Specification's chapters: lexing, types,
ranges, arrays, layouts, lists, repeat, records, sections, patterns,
expressions, checks, params, instances, budgets, settings, errors.

Cases are grouped by language version. A 0.1 case also runs against 1.0
implementations, which must either give the same result or fail in the way
a documented breaking change says (§10.2).

**Seeding the 0.1 suite:**

1. Port every test in Gecode's `test/instance.cpp` (about 1,200 lines, 40
   test groups) into cases.
2. Add the six example specs (DIMACS CNF, TSPLIB, TSPTW, job shop, sudoku,
   nonogram) with small sample files.
3. Add one case for every item in §6.2.
4. Add one case for every Specification example (generated).

### 7.2 Runner and driver protocol

Each implementation provides a small **conformance driver**: a command that
takes `parse|read|catalog|dzn SPEC [INPUT] [--params JSON] [--version V]`
and prints canonical JSON for the data or the error, or the dzn text for
`dzn`. One runner, the Rust crate `rust/iris-conformance` (also offered by
the `iris` CLI as `iris conformance`), runs every case against any driver
and prints a report. `conformance/README.md` is the authoritative protocol. The same runner works for Rust, C++, Java, Python, and
Gecode.

### 7.3 Gecode as an implementation

`conformance/adapters/gecode/` is a small C++ program, linked against
Gecode's `libgecodeinstance`, that implements the driver protocol. CI
builds Gecode 6.5.0 and runs the 0.1 suite against it. This makes Gecode
the oracle while the 0.1 Specification is written: a disagreement is
either a bug in the Specification or a bug in Gecode, and Gecode bugs are
reported upstream. Gecode itself needs no changes.

### 7.4 Fuzzing

- `cargo fuzz` on the parser and the reader, and libFuzzer with
  sanitizers on the C++ implementation.
- **Differential fuzzing:** generate random reader specs and matching
  inputs, then compare the canonical output of Rust, C++, Java, and
  Gecode. Cases where they disagree become conformance cases once fixed.

---

## 8. Deliverables

### 8.1 Rust (`rust/`)

- Crates: `iris-syntax`, `iris-sema`, `iris-core`, `iris-cli`,
  `iris-lsp`, `iris-wasm`, `iris-js`, `iris-macros` (after 0.1),
  `iris-build` (§8.15), and `iris-lint` if built (§8.13), published
  under the umbrella crate `gecode-iris`.
- API sketch:
  ```rust
  let spec = Spec::parse(text, "dimacs-cnf.iris")?;
  let data = spec.read_path("f.cnf", &Params::new())?;
  let n: i64 = data.int("vars")?;
  let clause: &[i64] = data.ints("clause", &[3])?;   // zero-copy row
  ```
  Columnar storage gives zero-copy slices for rows. Errors are one type,
  `iris::Error`, with a code, a span, a record path, and a `Display`
  implementation that renders Gecode's caret format.
- Optional `serde` feature: deserialize `Data` into user structs (after
  0.1).
- An `iris!` macro for inline specs (after 0.1), in a proc-macro crate
  `iris-macros` that `gecode-iris` re-exports. It parses and checks an
  embedded spec at compile time with `iris-syntax` and `iris-sema`, turns
  their diagnostics into compile errors pointing into the spec text, and
  expands to a ready `Spec`. It depends on `proc-macro2`, `quote` and
  `syn`; small dependencies like these are acceptable throughout the Rust
  crates.
  - Input: `iris!("...")`, usually a raw literal, so that Rust's tokenizer
    never sees Iris syntax such as `#` comments or `..<`; or
    `iris!(file = "PATH")`, with PATH relative to the calling package's
    `CARGO_MANIFEST_DIR`. The file form embeds the file with
    `include_bytes!`, so the file may hold any bytes, and editing it
    rebuilds the caller. Anything else, including macro calls such as
    `concat!`, is a usage error.
  - Origin: `iris!` for an inline spec, the path as written for a file.
  - Expansion: `Spec::parse` on the same bytes and origin, so the spec is
    parsed again at run time. A cheaper expansion would need `iris-core`
    types, or a serialized form of them, in the macro.
  - Diagnostics: the error code, then exactly what `iris check` prints.
    Stable Rust can only mark the whole literal or file path; on nightly the
    macro marks the error's position inside the literal.
  - `gecode-iris` defines `iris!` as a `macro_rules!` wrapper that passes
    `$crate` to the proc macro, so the expansion works when a dependent
    renames the crate.
- Performance target: at least as fast as Gecode on the 100 MB DIMACS
  benchmark from #270 (1.40 s), with ordinary buffered input (§12). A
  `criterion` benchmark suite tracks the format library inputs.
- CLI `iris`:
  `iris read SPEC FILE... [--summary|--json|--dzn] [--param k=v]`,
  `iris list`, `iris check SPEC`, `iris fmt` (§8.12), `iris lint`
  (§8.13, if built), `iris lsp`,
  `iris conformance DRIVER`, and from 1.0 `iris migrate` (§10.2). The exit
  codes match `gecode-instance` (2 for usage errors, 1 for spec or
  instance errors). Release binaries are built with `cargo-dist`, with a
  Homebrew tap and an install script.

### 8.2 Python (`python/`)

- Distribution name `gecode-iris`, import name `gecode_iris`. The name
  `iris` is taken on PyPI, and SciTools Iris imports as `iris`.
- PyO3 with the `abi3` stable ABI (Python 3.10+, provisional until §12
  open question 2 is decided), built by maturin. Wheels
  for manylinux and musllinux (x86_64 and aarch64), macOS (arm64 and
  x86_64), and Windows, built with `maturin-action`.
- API:
  ```python
  import gecode_iris as iris
  spec = iris.Spec.load("jobshop.iris")
  data = spec.read("jobshop1.txt", params={"instance": "la01"})
  data["n_jobs"]            # int
  data.ints("machine", 3)   # list[int]
  data.array("weight")      # numpy.ndarray if numpy is installed and rectangular
  data.to_dict(); data.to_json(); data.to_dzn()
  ```
  `iris.Error` has the attributes `code`, `line`, `column`, `path`, and
  `origin`. The package ships complete type stubs (`.pyi`) and `py.typed`.
- The wheel also installs the `iris` CLI entry point, so
  `pip install gecode-iris` (or `uvx gecode-iris`) is enough to use the
  skill.
- Tests: pytest, plus the conformance driver.
- The TypeScript package (§8.16) binds the Rust crate the same way,
  through WebAssembly.

### 8.3 Java and Kotlin (`java/`)

- Java 27, the newest feature release, as the baseline. We may lower it
  later if users need an LTS. Gradle with the Kotlin DSL.
  Published to Maven Central as `dev.gecode:iris` and
  `dev.gecode:iris-kotlin`.
- Modern Java: JPMS module `dev.gecode.iris`, records, sealed interfaces
  for values and errors, pattern-matching `switch`, primitive arrays
  (`long[]`, `int[]`, `double[]`) for columns, and no dependencies except
  JSpecify annotations.
- Designed for Kotlin:
  - `@NullMarked` (JSpecify), so Kotlin sees non-null types rather than
    platform types.
  - `IrisException extends RuntimeException`: no checked exceptions.
  - Factory methods with simple parameters, not builders, in the reading
    API. Spec builders (§8.15) are a separate API. They declare handles as
    typed constants, and a scoped value holds the open block.
  - Data can be read into records: `spec.reader(Shop.class,
    MethodHandles.lookup())` checks the record's components against the
    entries once, then reads through the canonical constructor.
  - No preview features in the published API.
  - Collections are `List`, not arrays, wherever no primitives are
    involved.
- `iris-kotlin`: operator `get` (`data["clause"][3]`), extensions that
  return `IntArray`, `LongArray`, and `DoubleArray`, `Sequence` views,
  `Params` from `mapOf(...)`, and later delegated properties
  (`val n: Int by data`) and a kotlinx.serialization decoder into data
  classes.
- Tests: JUnit 5, plus the conformance driver as a Gradle task. JMH
  benchmarks on the format library inputs.

### 8.4 C++ (`cpp/`)

- A native port of Gecode's implementation, with Gecode types replaced by
  the standard library: `std::span`, `std::string_view`, and standard
  containers with a pluggable allocator rather than Gecode's heap.
- API sketch:
  ```cpp
  auto spec = iris::Spec::parse(text, "sat.iris");          // throws iris::Error
  auto data = spec.read("f.cnf", {{"instance", "la01"}});
  std::span<const std::int64_t> c = data.ints("clause", {3});
  auto r = spec.try_read("f.cnf");                          // std::expected<Data, Error>
  ```
- CMake 3.28+, a `find_package(iris)` config package, FetchContent support,
  and a vcpkg port and Conan recipe later. An optional C++ module
  (`import iris;`) where the compilers support it.
- CI on GCC, Clang, and MSVC, with ASan, UBSan, and TSan, libFuzzer, and
  the conformance driver.

**Designed for Gecode 7.** Gecode 7 may switch from its own implementation
to this library, so the library must fit Gecode's needs:

- **C++ standard.** The library uses C++23. Gecode's planning leans
  towards C++20 for Gecode 7, because of conservative build environments
  such as academic HPC systems. If Gecode 7 settles on C++20 and wants to
  adopt this library, we revisit the standard then.
- **Allocation:** a hook so that Gecode can supply its heap.
- **Errors:** `iris::Error` carries everything Gecode's
  `Instance::Error` shows, so that Gecode can wrap it or derive from it.
- **Host constants:** Gecode supplies `gecode.int_max` and
  `gecode.int_min` through the host constant mechanism of 1.0, not as
  built-ins.
- **Build:** CMake only, matching Gecode 7's plan to drop Autoconf. It must
  work both as an installed package and as a vendored subdirectory.
- **License:** MIT, as Gecode.

### 8.5 Tree-sitter grammar (`tree-sitter-iris/`)

- `grammar.js` covering reader specs, with a structured parse of line
  patterns inside strings (captures `{x:t}`, `{$x}`, `{_}`, and `...`), so
  that captures are highlighted and can be navigated.
- Hard parts to solve early, with a spike:
  - Keywords can be names when a colon follows (`line: int`).
  - A range without an upper bound ends before a statement keyword or
    before `layout`, `offset`, or `until`. This needs tree-sitter's
    reserved words (0.25+) or an external scanner.
  - Statements have no separators (`repeat n { node: int  x: float }`).
- Queries: `highlights.scm`, `locals.scm`, `folds.scm`, `indents.scm`,
  `tags.scm`, and `injections.scm`. The injections cover pattern strings
  and, later, reader specs embedded in C++ raw strings, Rust, Python,
  Java, and Kotlin, recognized by a marker comment or by text that starts
  with `format `.
- Tests: the tree-sitter corpus, plus CI that parses every syntactically
  valid conformance spec and every format library spec with no `ERROR`
  nodes. The conformance suite marks which specs are syntactically valid;
  for the others, the test checks that the parser reports an error.
- Published to npm (`tree-sitter-iris`) and crates.io, and offered to
  nvim-treesitter and Helix.

### 8.6 TextMate grammar (`editors/textmate/`)

VS Code, Shiki (the documentation site), and GitHub's Linguist all
highlight with TextMate grammars, so we need one besides tree-sitter. It is
hand-written and kept small, and it is tested against the same corpus with
`vscode-tmgrammar-test`.

### 8.7 Language server (`rust/iris-lsp`)

- Diagnostics from parsing and resolution, with error codes linked to the
  documentation.
- Hover: the documentation for keywords and types, and the inferred type
  and shape of each name (`clause: int, depth 2`).
- Go to definition, find references, and rename for names, types, and
  record names.
- Completion for keywords, names in scope, types, layouts, and functions.
- Lints (§8.13), if built, as warnings, with their fixes as quick fixes.
- Semantic tokens, document symbols (an outline of repeats, sections, and
  instances), folding, and whole-document and range formatting with the
  formatter (§8.12).
- Inlay hints with the inferred shapes of declarations.
- **Live reading:** a directive comment such as `# iris: sample
  samples/ft06.txt` lets the server read a sample file on every change and
  report instance errors at the right place in the spec, together with a
  summary of the data. This is the feature that makes writing specs fast.
- Editor packages: a VS Code extension (TextMate grammar plus the LSP
  client, published to the Marketplace and Open VSX), a JetBrains plugin
  (§8.14), a Zed extension, and documented Neovim and Helix configurations.
- Directive comments (`# iris: ...`) are a tooling convention, not
  language syntax. Implementations ignore them like any comment, so a spec
  that uses them still reads in Gecode 6.5.0. The live-reading directive
  and lint suppressions (§8.13), if built, share the one syntax.

### 8.8 Agent skill (`skill/iris/`)

- A `SKILL.md` in the Agent Skills format, with progressive disclosure:
  - When to use it: writing, debugging, or explaining a reader spec, or
    turning a sample instance file into a spec.
  - The workflow: inspect the sample, look in the format library for a
    close match, draft the spec, run `iris check`, run
    `iris read --summary`, fix the errors, add `check` statements for the
    invariants the model relies on, test with an invalid file, and finish
    with `iris fmt` (and `iris lint`, if built).
  - A language cheat sheet and common patterns (header counts,
    `KEY : VALUE` sections, matrices with layouts, lists with sentinels,
    several instances per file, character grids).
  - How to read Iris errors, and how to embed specs in Gecode
    (`InstanceOptions`), Python, Java, and C++.
  - Iris 0.1 only, since that is what every implementation supports at
    0.1.0. Guidance on choosing between 0.1 and 1.0 (0.1 for Gecode 6.5,
    1.0 for Gecode 7 and the standalone libraries) is added at M9.
- `references/` holds the language reference and the format library index,
  loaded on demand.
- Evaluated with skill-creator style evals: a set of real benchmark files
  (RCPSP, MRCPSP, CVRP, QAP, graph coloring, PSPLIB, MaxSAT WCNF) where
  the agent must write a spec that reads each file correctly. Success is
  checked against expected data.
- Distributed from this repository as a Claude Code plugin. Also
  consider adding it to `gecode-skills` and linking it from the Gecode
  skill.

### 8.9 Format library (`formats/`)

Ready-made reader specs for standard benchmark formats. Each has a README,
sample files with known data, and conformance-style tests. It starts with
Gecode's six and grows toward: DIMACS CNF and WCNF, TSPLIB (TSP, ATSP,
CVRP), TSPTW, job shop (OR-Library, JSPLIB, Taillard), flow shop and
open shop, RCPSP and MRCPSP (PSPLIB `.sm` and `.mm`, Patterson), QAPLIB,
DIMACS graph coloring (`.col`), sudoku, nonogram, and bin packing
(BPPLIB). The site generates one page per format.

The library is also the best test of the language: every format that 0.1
cannot express, or expresses awkwardly, is input to the 1.0 design.

### 8.10 Documentation site (`site/`)

- **Astro Starlight**, the same stack family as gecode.dev, so styles and
  knowledge carry over. Hosted at `iris.gecode.dev`.
- Sections:
  1. **Introduction:** what Iris is, a 2-minute example, and how it relates
     to Gecode.
  2. **Tutorial:** writing the DIMACS spec, then TSPLIB sections, then job
     shop records and checks.
  3. **Guides:** one per topic (patterns, sections, records and checks,
     several instances, parameters, errors, performance).
  4. **Language Specification:** the normative text from `spec/`, with a
     version switcher (0.1, 1.0).
  5. **Format library:** pages generated from `formats/`.
  6. **Using Iris from:** Rust, Python, TypeScript, Java, Kotlin, C++,
     Gecode. Each links to its generated API docs (docs.rs, a Python API
     reference built with mkdocstrings or pdoc and hosted on the site,
     Javadoc and Dokka, and Doxygen for C++).
  7. **Tools:** CLI, formatter, linter if built (one page per lint), LSP,
     editors, and the agent skill.
  8. **Playground:** the Rust implementation compiled to WASM. Edit a spec
     and an input side by side, and see the data (as a table, JSON, or
     dzn) or the error with its caret, updated as you type, with a Format
     button and any lints. Links can be shared through state in the URL.
- Syntax highlighting through the TextMate grammar in Expressive Code.
- CI builds the site on every pull request, checks links, and tests the
  site's examples like those of the Specification.

### 8.11 Brand: logo and favicon (`brand/`)

**Palette**, from the Gecode logo:

| Role | Color |
|---|---|
| Red card | `#DA251D` |
| Orange card | `#EB891B` |
| Blue card (and the gecode.dev link color) | `#005CA1` |
| Green stem (gecode.dev brand color) | `#0B7646` |
| Ink | `#27393E` (the gecode.dev border color) |

**The mark** (chosen, in `brand/`): an iris drawn with the cards of the
Gecode logo. The orange card is the standard, the red and blue cards are
the falls, and the green stem carries the flower. The icons use a tuned
version with wider gaps, so the parts stay apart at 16 and 32 px.
`brand/concepts/` keeps the concepts that led to it.

**Deliverables**, all generated by `brand/build.py` (see `brand/README.md`
for the file list and the rules for use):
- the mark in color and in one color for light and dark backgrounds;
  stacked and horizontal lockups with the lowercase wordmark "iris" in
  Raleway, as on gecode.dev, each in four versions;
- `favicon.svg`, `favicon.ico` (16, 32, 48), `apple-touch-icon.png` (180),
  `icon-192.png`, `icon-512.png`, and a web manifest.

Still to make, by the components that need them: a social card
(`og-image.png`, 1200×630) for the site, and icons for the VS Code
extension, the JetBrains plugin, the PyPI and crates.io READMEs, and the
GitHub repository avatar, all from the existing mark.

Check every version against WCAG contrast for any text, at 16 px, and in
dark mode.

### 8.12 Formatter (`iris fmt`, in `iris-syntax`)

- **One style.** Specs are short, often embedded in programs, and shared
  through the format library, so a single style is worth more than
  configurability. The only setting is the line width, read from
  `max_line_length` in `.editorconfig` for the spec's path, and 100 when
  the property is unset or `off`. The formatter reads no other
  `.editorconfig` property and has no configuration file of its own.
- **The style** is decided by formatting Gecode's six example specs and
  the first formats of the library, then writing the result down as a
  style guide on the site. The guide can come before the tool, so the
  format library is written in the final style from the start. Starting
  points: one statement per line (statements have no separators), two
  spaces of indentation inside `{ }`, no column alignment (it causes diff
  noise), single blank lines kept and longer runs collapsed, and comments
  kept where they are. The decided style is in
  [`docs/formatting.md`](docs/formatting.md).
- **It never changes meaning.** It changes only whitespace and line
  breaks: it never rewrites strings, line patterns, numbers, or names, and
  never reorders statements. Rewrites belong to lint fixes and
  `iris migrate`. It knows both language versions.
- **Broken input:** `iris fmt` refuses a spec with syntax errors and
  leaves it unchanged. The LSP still formats a range that parses cleanly.
- **CLI:** `iris fmt FILE...` rewrites in place, `--check` exits 1 when a
  file is not formatted (for CI), `--diff` prints the change, and `-`
  reads standard input, with `--stdin-filepath PATH` naming the path whose
  `.editorconfig` applies. The LSP uses the document's path, and the
  playground uses 100. Formatting reader specs embedded in host source
  files, found as the tree-sitter injections find them (§8.5), comes after
  0.1.
- **Stability:** formatter output changes only in minor package releases,
  and the changelog lists each style change, so that `--check` in users'
  CI does not break on a patch release.
- **Tests:**
  - Idempotence: formatting a formatted spec changes nothing.
  - Same tree: the formatted spec parses to the same syntax tree, ignoring
    whitespace.
  - Same data: every conformance case reads the same data or reports the
    same error code after its spec is formatted (locations shift, so they
    are not compared).
  - Snapshot tests of the style at several line widths, and of
    `.editorconfig` lookup (nested files, `root = true`, globs that match
    `.iris`), plus formatting the random specs from the
    differential fuzzer (§7.4) under the same three checks.
- CI checks that the format library and the site's examples are
  formatted. Specification examples and conformance specs are exempt: some
  test unusual layout on purpose, and error results include positions in
  the spec that formatting would move.
- Available in Rust and in everything built on it: the CLI, the LSP, the
  playground, and the `iris` command in the Python wheel. The C++ and
  Java libraries do not format.

### 8.13 Linter (`iris lint`, crate `iris-lint`), only if it earns its place

**We build the linter only if there are useful lints.** A useful lint
flags a mistake that was actually made in a real spec (the format library,
Gecode's examples, the skill evals, or a user report); the Specification
cannot make it an error; and it gives no false warnings on the format
library. Candidates and the specs that show them are collected in
`docs/lint-candidates.md` while the format library and the skill evals
are written. When those are done we decide (§12, open question 3):

- No useful lints: no linter. Drop `iris-lint`, `iris lint`, and lint
  suppressions from this plan.
- One or two: report them as warnings from `iris check` and the LSP, in
  `iris-sema`, without a separate crate or command.
- More: build the linter as described below.

A candidate that points at confusing 0.1 behavior may belong in
`docs/1.0-ideas.md` instead, so that 1.0 removes the trap.

The rest of this section describes the linter if we build it.

- **Errors and lints are different things.** An error (§4.4) makes a spec
  invalid; it has a code, is normative, and is tested by the conformance
  suite. A lint flags a valid spec that is probably wrong, fragile, or hard
  to read. Lints are not part of the Specification or the conformance
  suite, and only the Rust tooling has them.
- **Every lint has** a stable kebab-case name (such as
  `unreachable-arm`), a default level (`warn` or `allow`), and a page on
  the site with an example and the fix. Names, not codes, because
  suppressions are read by people.
- **Ideas to test against real specs**, not a commitment:
  - *Likely bugs:* a sections arm that never matches because an earlier
    arm matches every line it would; a `check` whose condition is
    constant, so it never fails or always fails; a range with constant
    bounds that is empty; a named type or `each` name that is never used.
  - *Versions (from 1.0):* a construct whose meaning changes in 1.0 or
    that 1.0 deprecates, such as the `gecode.*` host constants (§6.3),
    with the `iris migrate` rewrite as its fix (§10.2).
  - Style is the formatter's job, not the linter's.
- **Fixes:** a lint may offer a fix. `iris lint --fix` applies the fixes
  that are safe to apply without review, and the LSP offers every fix as a
  quick fix.
- **Configuration:** only directive comments, such as
  `# iris: allow unreachable-arm` before a statement or at the top of the
  file. No configuration file in 0.1: specs are single files, often
  embedded in a host program, and a lint setting should travel with the
  spec.
- **CLI:** `iris check` keeps reporting errors only. `iris lint SPEC...`
  reports errors and lints, exits 1 on an error or a lint at level `deny`,
  and turns warnings into failures with `--deny warnings`. `--json` prints
  diagnostics for tools.
- **Tests:** a directory of specs per lint with expected diagnostics and
  expected fixed output; every fix that is not a version migration must
  read the same data on the format library samples. CI keeps the format
  library free of warnings.

### 8.14 JetBrains plugin (`editors/jetbrains/`)

For IntelliJ IDEA, CLion (where Gecode's C++ users are), PyCharm,
RustRover, and the other JetBrains IDEs.

- **A thin client, like the VS Code extension.** The plugin registers the
  `.iris` file type and icon, highlights with the TextMate grammar
  (§8.6), and gets everything else from `iris lsp`: diagnostics, lints and
  their fixes, completion, hover, navigation, rename, formatting, semantic
  tokens, inlay hints, and live reading.
- **LSP client:** the IntelliJ Platform's own LSP API, which JetBrains
  made available in all its IDEs, including the unified IntelliJ IDEA,
  from 2025.3. Rename needs 2026.1.1 and range formatting 2026.1, so 2025.3
  users go without them. LSP4IJ, the open-source client that also runs in
  Android Studio, is the fallback if the platform API lacks a feature we
  need (§12, decided).
- **Not a native parser.** A Grammar-Kit parser and PSI would be a fourth
  parser to keep in step with the Specification. The cost is that JetBrains
  language injection, which needs PSI, cannot mark reader specs inside C++,
  Kotlin, or Python strings. Revisit after 0.1 if users ask for it.
- **The server binary:** the plugin uses `iris` from a configured path or
  from `PATH`, and otherwise offers to download the release binary (§8.1)
  that matches its version, with a checksum check (§12, decided).
  The download offer requires:

  - A version-to-release mapping, supported OS/architecture assets and
    checksums published with the Iris release; match the server host
    rather than the frontend in remote development. Keep configured
    commands and `PATH` usable for offline/manual installation.
  - An explicit offer naming the version, source, and target, with
    download, configure-existing, and decline actions. The size is not
    known before the plugin contacts the source, so the IDE reports it
    during the download. Do not download or execute a command taken from
    a reader spec.
  - HTTPS using the IDE's proxy settings, a trusted expected SHA-256,
    safe extraction, executable permissions, and a versioned user-writable
    cache outside the plugin directory. Verify before atomic installation
    and execution; never run a partial or mismatched file.
  - Cancellable background progress, cleanup on failure/cancellation,
    and useful retry/manual-install messages for offline, unsupported,
    checksum, and permission failures. JetBrains documents
    [background progress and cancellation](https://plugins.jetbrains.com/docs/intellij/background-processes.html).
  - A server compatibility check, reuse of a verified cached version,
    and a clear update/removal policy. Include the binary's license
    notices and account for platform signing requirements when publishing.
- Later: a run configuration that runs `iris read --summary` on a sample
  file and shows the data.
- Kotlin, built with the IntelliJ Platform Gradle Plugin 2.x, and
  published to JetBrains Marketplace as "Iris". Supports the IDE releases
  of the past year.
- **Tests:** the IntelliJ Plugin Verifier on every supported IDE release,
  and a headless test that opens a format library spec and expects
  highlighting and no diagnostics.

### 8.15 Spec builders

Many users want to write reader specs in their own language for type
safety: entry names and value types checked by the compiler or type
checker, not discovered when data is read. Every implementation gets a
spec builder, and so does the TypeScript package (§8.16).
[docs/builders.md](docs/builders.md) is the design brief, with sketches in
every language. In short:

- A builder prints Iris text for one language version and parses and checks
  it with the ordinary `Spec::parse`. Builder construct and syntax
  correspond one to one: no builder-only semantics, no second syntax-tree
  contract, no raw-text escape hatch. `source()` returns the text.
- Defining statements return typed handles, or define handles declared
  beforehand, as Java does. A handle carries the name, value type, depth,
  and possible absence. `data.get(handle)` returns host types and
  cannot fail on a name or a type. A scope's repeat depth fixes each
  handle's depth.
- Each language uses its own idioms for blocks, operators, ranges, and line
  patterns. A host operator builds an Iris operator only when the two mean
  the same thing.
- Errors carry the language error code, the emitted line, and the host
  call site.
- The same handle types serve typed views of text specs: `iris gen --lang
  L SPEC` and the `iris!` macro generate them from `iris-sema`'s facts.
- Placement: a `build` module in the Rust workspace re-exported by
  `gecode-iris`; pure Python in `gecode_iris.build`, with stubs;
  `iris/build.hpp`; the package `dev.gecode.iris.build`, with the Kotlin
  DSL in `iris-kotlin`; and pure TypeScript in the TypeScript package.
- **Tests:** a generator, `iris gen builder --lang L`, turns every checked
  conformance and format spec into builder code. Running that code must
  reproduce the spec, compared as syntax trees by an `iris-conformance`
  subcommand. Each format also gets an idiomatic hand-written builder, and
  type tests check that misuse fails to compile.

### 8.16 TypeScript (`typescript/`)

- Distribution name `gecode-iris` on npm, matching crates.io and PyPI.
  ESM only, for Node 22.12 or later, browsers, Deno, Bun, and edge
  runtimes such as Cloudflare Workers.
- The package binds the Rust crate through WebAssembly, as Python binds
  it through PyO3 (ADR 0017). The crate `rust/iris-js`, built with
  wasm-bindgen 0.2.129 like `iris-wasm`, is the boundary; `iris-wasm`
  stays the playground's crate with its own contract and size budget.
  WebAssembly adds no semantics: parsing, reading, errors, JSON, and dzn
  come from `iris-core`, so a conformance difference is a binding bug.
  The API does not depend on the backend, so a pure-TypeScript backend
  could replace WebAssembly later without breaking users.
- API:
  ```ts
  import { Spec } from "gecode-iris";
  const spec = Spec.parse(text, "jobshop.iris");   // parsed once
  const data = spec.read(input, "la01.txt", { params: { instance: "la01" } });
  data.entries;               // [{ name, valueType, depth }, ...]
  data.entry("n_jobs");       // value type, depth, presence
  data.int("n_jobs");         // number
  data.ints("machine", [3]);  // number[]
  data.toJSON(); data.toDzn();
  ```
  `read` and `catalog` accept a `Uint8Array` or a string. Looking up an
  entry reports its value type, depth, and presence, so the spec
  builder's typed layer (§8.15) sits on `Data` without parsing JSON.
  `toJSON()` returns the canonical text and is never parsed internally.
- **64-bit integers:** accessors that return `number` throw `RangeError`
  outside `Number.MAX_SAFE_INTEGER`; the `bigint` and `BigInt64Array`
  variants are always exact.
- **Memory:** `Data` is copied into JavaScript typed arrays with offsets
  per level, so it needs no `free()`. Only `Spec` holds a WebAssembly
  handle; it supports `[Symbol.dispose]` with a `FinalizationRegistry`
  fallback and never requires `free()`. `iris-core`'s validated column
  constructors carry data across the boundary in a binary layout, and
  JSON and dzn are rendered by `iris-core` only on request.
- **Errors:** `IrisError extends Error` with `code`, `message`,
  `inputLocation` and `specLocation` (each `{ origin, line, column }` or
  `null`), `context`, `recordPath`, the diagnostic text, and `toJSON()`.
  Python keeps one location; TypeScript keeps both because spec builders
  map the spec location back to the host call site, also for
  `E-CHECK-FAILED` while reading (`docs/builders.md`, principle 8).
- **Packaging:** export conditions for `node` (synchronous load, so
  `require()` works), `browser` (top-level `await`), and
  `workerd`/`edge-light` (a static `.wasm` import), plus an `inline` entry
  with the bytes embedded and compiled synchronously. In every entry
  `Spec.parse` works as soon as an importing module evaluates, because
  spec builders define specs at module scope. `initialize()` remains for
  callers that supply their own module or URL. Pages with a strict
  Content-Security-Policy need `'wasm-unsafe-eval'`. A `gecode-iris/build`
  entry is left for the spec builder.
- **Resilience:** the crate builds with `panic = "abort"`, so a trap
  surfaces as `RuntimeError: unreachable`. The package turns it into an
  `IrisInternalError` and recreates the instance; it also recreates the
  instance after a large read, because WebAssembly memory never shrinks.
- **Budgets and deadlines:** synchronous WebAssembly cannot be cancelled
  from its own thread. The language's fixed work budgets (Specification
  chapter 10, `E-BUDGET`) bound every read; the package adds no budget
  settings, and its guide recommends a Worker for wall-clock deadlines,
  as the playground does.
- **Tests:** the conformance suite through a Node driver that uses the
  package, `node:test` unit tests, type tests of the published
  declarations with `tsc`, one browser smoke test with the site's
  Playwright setup, and one Workers smoke test in `workerd`.
- **Toolchain and code style:** stable Rust with `wasm32-unknown-unknown`,
  wasm-bindgen-cli 0.2.129, Binaryen's `wasm-opt`, and Node. The
  formatter's output is the style: oxfmt and type-aware oxlint, pinned as
  devDependencies, run in check mode in CI with warnings denied.

---

## 9. Engineering practices

- **License:** MIT, matching Gecode.
- **Versioning:** language versions (0.1, 1.0) and package versions are
  separate. Each package states which language versions it supports
  (`iris --version` prints both). Packages follow semver: 0.x while they
  implement Iris 0.1, and 1.0.0 with Iris 1.0.
- **Releases:** one tag per component (`rust-v0.1.0`, `python-v0.1.0`,
  ...). Releases are automated from tags, and changelogs come from
  conventional commit scopes. History stays linear, with rebase only.
- **CI:** GitHub Actions with path filters for each component, and a
  `conformance` workflow that runs the suite against every implementation
  and Gecode 6.5.0, and publishes a conformance matrix on the site.
- **Decisions:** ADRs in `docs/adr/`, starting with:
  1. Iris 0.1 is the Gecode 6.5.0 language, and Iris 1.0 is the language
     of Gecode 7.
  2. The name is written Iris or `iris`, never in capitals.
  3. Rust is the reference implementation, with a lossless parser.
  4. C++ and Java are native implementations, and Python binds Rust.
  5. The conformance suite and canonical JSON are the contract between
     implementations.
  6. Errors have stable codes, and message wording is not normative.
  7. Gecode 6.5 keeps its own implementation. The C++ library is designed
     so that Gecode 7 can adopt it.
  8. Monorepo layout.
  9. C++23 and Java 27 are the language baselines.
  10. The MIT license.
  11. Iris 0.1 input and strings are UTF-8.

---

## 10. Toward Iris 1.0

### 10.1 Where the changes come from

- Formats in the library (§8.9) that 0.1 cannot express or expresses
  awkwardly.
- Gaps found while writing the 0.1 Specification, where the 0.1 behavior is
  correct but surprising.
- What Gecode 7 needs, including whether Gecode adopts the C++ library.
- Neutral host constants (§6.3), and other candidates collected in
  `docs/1.0-ideas.md` during the 0.1 work.

Every change is an ADR, a change to `spec/1.0/`, and new conformance cases,
in that order.

### 10.2 Compatibility between 0.1 and 1.0

Gecode 6.5 users embed reader specs in their programs and will carry them to
Gecode 7, so 1.0 should be **compatible by default**:

- A valid 0.1 spec reads the same data under 1.0, unless the 1.0
  Specification lists the change as breaking.
- Every breaking change has a mechanical rewrite in `iris migrate`, and
  the LSP offers it as a quick fix.
- A spec can state its version (for example `iris 0.1` at the top), and
  implementations read it with that version's rules. A spec without the
  line uses the newest version the implementation supports.

How strong this promise should be is §12, open question 1.

---

## 11. Milestones

Dependencies flow downward. Work in the same row can run in parallel.

### Iris 0.1 (the Gecode 6.5.0 language)

| # | Milestone | Contents | Done when |
|---|---|---|---|
| M0 | Foundations | Repository skeleton, `CONTEXT.md`, ADRs 1 to 11, license, CI scaffolding, brand | The layout exists and builds empty, and the logo and icons are in `brand/` |
| M1 | 0.1 Specification | Chapters from §6, grammar, data model, JSON schemas, error code registry, answers to §6.2 | Every item in §6.2 is decided, with a case planned for each |
| M2 | 0.1 conformance suite | Gecode tests ported, example formats, the runner, the Gecode adapter | The suite runs green against Gecode 6.5.0, apart from documented Gecode bugs |
| M3 | Rust core and CLI | `iris-syntax`, `iris-sema`, `iris-core`, `iris-cli` | 100% conformance, performance target met, fuzzing in CI |
| M4a | Python | Wheels, stubs, docs | 100% conformance, wheels on TestPyPI |
| M4b | Tooling | Tree-sitter, TextMate, LSP, VS Code, JetBrains, and Zed packages | Every syntactically valid conformance spec parses cleanly, the LSP features in §8.7 work, and the JetBrains plugin passes the Plugin Verifier |
| M4c | C++ | Port, CMake package | 100% conformance, sanitizer runs clean, differential fuzzing against Rust |
| M4d | Java and Kotlin | Core library, Kotlin module | 100% conformance, JMH baseline recorded |
| M4e | Formatter | `iris fmt`, `.editorconfig` line width, the style guide | The checks of §8.12 pass on every conformance case, and the format library is formatted |
| M4f | TypeScript | The npm package `gecode-iris` over `rust/iris-js` (§8.16), Node driver, browser and Workers packaging, site guide | 100% conformance through the Node driver; the package loads in Node (ESM and `require()`), a browser, and `workerd`; the published declarations type-check |
| M5 | Site and playground | Starlight site, the Specification rendered, guides, WASM playground, format pages | Deployed at `iris.gecode.dev` |
| M6 | Skill | `skill/iris`, evals | The eval set passes, for example 8 of 10 formats read correctly on the first attempt |
| M6b | Lint decision | Review `docs/lint-candidates.md` (§8.13, §12 open question 3), then build what the decision calls for | The decision is recorded in §12; any lints that ship have tests, fixes where possible, and site pages. Not required for M7 |
| M7 | 0.1.0 release | The 0.1 Specification frozen; Rust, Python, TypeScript, Java and Kotlin, C++, the VS Code extension, and the JetBrains plugin published as 0.1.0; announcement | Those registries and marketplaces have 0.1.0, and the docs are live |
| M7a | Remaining tooling | Tree-sitter and the Zed extension (finishing M4b), the skill's evals (finishing M6) | Those packages are published, and the skill's eval set passes. Not required for M7; follows it in 0.x package releases |
| M7b | Spec builders | Builders in every implementation (§8.15), `iris gen`, the round-trip generator and comparison, a builder page in each API guide | Every checked conformance and format spec round-trips through each language's builder, and every format has an idiomatic builder whose cases pass. Not required for M7; ships in 0.x package releases |

M4a to M4f can start once M2 is done, and run in parallel, which is where
splitting the work between agents pays off. Some need parts of M3 to start
or to finish: Python and TypeScript bind the Rust core; the LSP and the formatter start
on the Rust parser and resolver and need the complete reader only for live
reading and the same-result check; C++'s differential fuzzing needs the
Rust driver. The LSP's formatting needs M4e. M6b needs the
format library and the skill evals, since they supply the evidence.
The site can start during M1, so the Specification is readable while it is
written. The formatting style guide can start with the format library,
before M4e.

### Iris 1.0 (the Gecode 7 language)

| # | Milestone | Contents | Done when |
|---|---|---|---|
| M8 | 1.0 design | Ideas from §10.1 reviewed, ADRs, `spec/1.0/` draft, the compatibility rules of §10.2 | The 1.0 Specification is feature-complete as a draft |
| M9 | 1.0 implementations | 1.0 in Rust, C++, Java, Python, and TypeScript, with `iris migrate` and tooling updates, all supporting 0.1 and 1.0 | 100% on the 1.0 suite and on the 0.1 suite (with listed exceptions) |
| M10 | Gecode 7 integration | Decide whether Gecode 7 adopts the C++ library; if so, integrate it and run Gecode's examples and tests on it | Gecode 7 reads Iris 1.0 specs |
| M11 | 1.0.0 release | 1.0 Specification frozen, all packages at 1.0.0 | Released together with Gecode 7 |

---

## 12. Decisions

### Decided

- **C++ and Java are native implementations.** Python binds Rust.
- **TypeScript binds Rust through WebAssembly:** the npm package
  `gecode-iris` wraps the crate `rust/iris-js`, separate from the
  playground's `iris-wasm` (§8.16, ADR 0017). Confirmed on 2026-11-03. One
  implementation keeps every fix and Iris 1.0 change in step with Rust,
  and one artifact runs in browsers, Node, Deno, Bun, and edge workers;
  the playground's optimized module measured 645 KiB raw and 271 KiB
  gzipped. A native Node addon and a pure-TypeScript reader were
  rejected; a pure-TypeScript backend may later replace WebAssembly
  behind the same API.
- **Names:** the umbrella name `gecode-iris` on crates.io, PyPI, and npm.
  Python imports `gecode_iris`, Maven uses `dev.gecode:iris`, the C++
  namespace is `iris`, the CLI is `iris`, files use `.iris`, the site is
  `iris.gecode.dev`, and the repository is `Gecode/iris`. A check on
  2026-10-31 found `gecode-iris` free on crates.io, PyPI, and npm,
  `tree-sitter-iris` free on npm, and `Gecode/iris` not yet in use.
  Publishing `dev.gecode` on Maven Central requires verifying ownership of
  the `gecode.dev` domain. Published crates take the `gecode-iris-` prefix
  (`gecode-iris-syntax`, `-sema`, `-core`, `-macros`, `-build`, `-gen`,
  `-lsp`, `-conformance`, `-cli`), because a check on 2026-11-05 found
  `iris-core`, `iris-cli`, and `iris-macros` taken on crates.io. The JetBrains
  plugin has the ID `dev.gecode.iris` and the name `Gecode Iris`, because an
  unrelated plugin is called Iris. The VS Code extension is `gecode.iris` and
  the Homebrew formula is `gecode/tap/iris`. `iris-driver` is a conformance
  tool and stays out of `cargo install` and the release archives.
- **Domain:** the project's domain is `gecode.dev`, and `dev.gecode` where
  reverse order is used. `gecode.org` is no longer the project's, so nothing
  here links to it or uses `org.gecode`.
- **Gecode:** 6.5.0 keeps its own implementation, which defines Iris 0.1.
  Gecode 7 may switch to the Iris C++ library.
- **Versions:** Iris 0.1 is the Gecode 6.5.0 language. Iris 1.0 develops it
  further and is the language of Gecode 7.
- **Writing the name:** Iris or `iris`, never in capitals.
- **C++ standard:** C++23. Revisit if Gecode 7 adopts the library and
  settles on C++20.
- **Java baseline:** Java 27, the newest feature release. Revisit if users
  need an LTS.
- **License:** MIT, as Gecode.
- **Input is UTF-8:** Iris 0.1 strings are Unicode text, and the whole
  input must be valid UTF-8, including comments, skipped lines, and
  ignored tails. A `word` or `rest` value that is not valid UTF-8 is also
  an error, since token settings work on bytes and can split a character.
  Gecode 6.5.0 accepts any bytes, so this is a deliberate difference,
  recorded in the Specification's differences appendix. It keeps the
  canonical JSON plain text.
- **Reader specs are UTF-8:** the whole source of a reader spec must be
  valid UTF-8, including string literals and comments. Gecode 6.5.0
  accepts any bytes inside strings and comments; this is the same
  deliberate difference as for input.
- **Float underflow:** a float token whose value is nonzero but rounds to
  a subnormal number or to zero is rejected, as the reference build does.
  This gives every implementation the same portable rule.
- **Formatter settings:** the line width is the only setting, read from
  `max_line_length` in `.editorconfig`, with 100 as the default (§8.12).
- **Logo:** the flower mark in `brand/` (§8.11), drawn with the Gecode
  cards; `brand/README.md` has the files and the rules for use.
- **Work tracking:** zdev, with personal records (`.zdev/` is not
  committed).
- **JetBrains LSP client:** the IntelliJ Platform's own LSP API, decided
  on 2026-11-01 from the jetbrains-001 spike. It needs no second plugin and
  covers every feature in §8.7. On 2025.3 it lacks rename (from 2026.1.1)
  and range formatting (from 2026.1); users of that release go without
  them. Android Studio and open-source IDEA builds are not supported.
  LSP4IJ remains the fallback if a needed feature turns out to be missing.
  Sources: [the SDK's LSP page](https://plugins.jetbrains.com/docs/intellij/language-server-protocol.html)
  and [LSP4IJ's feature support](https://github.com/redhat-developer/lsp4ij/blob/main/docs/LSPSupport.md).
- **How the JetBrains plugin gets `iris`:** a configured path, else `iris`
  on `PATH`, else an offer to download the matching release binary with a
  checksum check (§8.14), decided on 2026-11-01. Bundling was estimated at
  about 5 MiB of plugin download per platform, from a comparable Rust
  language server.
- **No memory-mapped input:** iris-core reads input files into memory
  with an ordinary buffered read, decided on 2026-11-01 from the rust-013
  measurements. Reading a generated 100 MB DIMACS file took 15 ms of a
  16 s run, so a map would gain almost nothing. Mapping the file itself
  would need `unsafe` code, and a file changed during a read could crash
  the process. Revisit if a profile shows acquisition mattering.

### Open

1. **How compatible 1.0 is with 0.1.** *Recommendation:* as in §10.2,
   compatible by default, with breaking changes listed and handled by
   `iris migrate`.
2. **Python floor.** *Recommendation:* 3.10+ with `abi3`.
3. **Whether to build a linter.** Decided at M6b from the evidence in
   `docs/lint-candidates.md`, by the rule in §8.13. *Recommendation:* no
   linter unless at least three candidates meet that rule.
