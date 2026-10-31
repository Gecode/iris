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
| **Iris 0.1** | Exactly the language of Gecode 6.5.0, written down precisely. No new features. | Gecode 6.5.0 (its own implementation) |
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
     formatter, WASM playground, and Python package.
   - Python package built on the Rust crate.
   - Java library for current Java, designed to work well from Kotlin,
     plus a small Kotlin extensions module.
   - C++ library with no Gecode dependency, designed so that Gecode 7 can
     adopt it.
4. **Tooling:** the `iris` CLI, a tree-sitter grammar, a TextMate grammar,
   a language server, and editor packages (VS Code, Zed, Neovim, Helix).
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
iris-core     reader, expression evaluator, data model, work budgets,
     │        JSON and dzn output
     ├── iris-cli    `iris` binary
     ├── iris-lsp    language server
     ├── iris-wasm   wasm-bindgen API for the playground
     └── (python/)   PyO3 binding
```

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
│   ├── runner/              runs the cases against any driver (§7.2)
│   └── adapters/gecode/     driver that links Gecode's libgecodeinstance
├── formats/                 library of reader specs for standard formats
│   └── <format>/            <format>.iris, README.md, samples/, expected/
├── rust/                    Cargo workspace (crates from §4.2)
├── python/                  PyO3/maturin package
├── cpp/                     C++ library (CMake)
├── java/                    Gradle build: iris (Java), iris-kotlin
├── tree-sitter-iris/        grammar.js, queries/, tests, bindings
├── editors/
│   ├── textmate/            iris.tmLanguage.json (VS Code, Shiki, GitHub)
│   ├── vscode/              extension: grammar plus LSP client
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
takes `read|catalog|parse SPEC [INPUT] [--params JSON] [--version V]` and
prints canonical JSON for the data or the error. One runner (in Rust, part
of the `iris` CLI as `iris conformance`) runs every case against any driver
and prints a report. The same runner works for Rust, C++, Java, Python, and
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

- Crates: `iris-syntax`, `iris-sema`, `iris-core`, `iris-cli`, `iris-lsp`,
  `iris-wasm`, published under the umbrella crate `gecode-iris`.
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
- Optional `spec!` macro that checks an embedded spec at compile time
  (after 0.1).
- Performance target: at least as fast as Gecode on the 100 MB DIMACS
  benchmark from #270 (1.40 s), with memory-mapped input. A
  `criterion` benchmark suite tracks the format library inputs.
- CLI `iris`:
  `iris read SPEC FILE... [--summary|--json|--dzn] [--param k=v]`,
  `iris list`, `iris check SPEC`, `iris fmt`, `iris lsp`,
  `iris conformance DRIVER`, and from 1.0 `iris migrate` (§10.2). The exit
  codes match `gecode-instance` (2 for usage errors, 1 for spec or
  instance errors). Release binaries are built with `cargo-dist`, with a
  Homebrew tap and an install script.

### 8.2 Python (`python/`)

- Distribution name `gecode-iris`, import name `gecode_iris`. The name
  `iris` is taken on PyPI, and SciTools Iris imports as `iris`.
- PyO3 with the `abi3` stable ABI (Python 3.10+), built by maturin. Wheels
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
  - Factory methods with simple parameters, not builders.
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
- Tests: the tree-sitter corpus, plus CI that parses every conformance
  spec and every format library spec with no `ERROR` nodes.
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
- Semantic tokens, document symbols (an outline of repeats, sections, and
  instances), folding, and formatting through `iris fmt`.
- Inlay hints with the inferred shapes of declarations.
- **Live reading:** a directive comment such as `# iris: sample
  samples/ft06.txt` lets the server read a sample file on every change and
  report instance errors at the right place in the spec, together with a
  summary of the data. This is the feature that makes writing specs fast.
- Editor packages: a VS Code extension (TextMate grammar plus the LSP
  client, published to the Marketplace and Open VSX), a Zed extension, and
  documented Neovim and Helix configurations.

### 8.8 Agent skill (`skill/iris/`)

- A `SKILL.md` in the Agent Skills format, with progressive disclosure:
  - When to use it: writing, debugging, or explaining a reader spec, or
    turning a sample instance file into a spec.
  - The workflow: inspect the sample, look in the format library for a
    close match, draft the spec, run `iris check`, run
    `iris read --summary`, fix the errors, add `check` statements for the
    invariants the model relies on, and test with an invalid file.
  - A language cheat sheet and common patterns (header counts,
    `KEY : VALUE` sections, matrices with layouts, lists with sentinels,
    several instances per file, character grids).
  - How to read Iris errors, and how to embed specs in Gecode
    (`InstanceOptions`), Python, Java, and C++.
  - Which language version to write for: 0.1 for Gecode 6.5, 1.0 for
    Gecode 7 and the standalone libraries.
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
  6. **Using Iris from:** Rust, Python, Java, Kotlin, C++, Gecode. Each
     links to its generated API docs (docs.rs, a Python API reference built
     with mkdocstrings or pdoc and hosted on the site, Javadoc and Dokka,
     and Doxygen for C++).
  7. **Tools:** CLI, LSP, editors, and the agent skill.
  8. **Playground:** the Rust implementation compiled to WASM. Edit a spec
     and an input side by side, and see the data (as a table, JSON, or
     dzn) or the error with its caret, updated as you type. Links can be
     shared through state in the URL.
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
| Green card (gecode.dev brand color) | `#0B7646` |
| Ink | `#27393E` (the gecode.dev border color) |

**Concept:** the Gecode logo is a fan of four rounded diamond cards in red,
orange, blue, and green. For Iris, the same cards are arranged radially
around a center, as the petals of an iris. That reads both as the iris of
an eye (*reading*) and as the flower, and it is plainly related to Gecode.

Directions to sketch:

1. **Eye iris:** eight card-shaped petals (each color twice, in Gecode
   order) around a round pupil in ink or white. Use this for the main
   mark.
2. **Flower iris:** three upright petals and three falling petals, using
   the card shape. More botanical, but four colors do not divide evenly
   into three.
3. **Aperture:** four overlapping cards rotated around a square opening,
   like a camera shutter. Simplest, and best at 16 px.

My recommendation is direction 1 for the logo and direction 3 (or a
four-petal version of 1) for the favicon, since eight petals blur at
16×16.

**Deliverables:**
- `iris-mark.svg` (the symbol), `iris-logo.svg` (the symbol plus the
  lowercase wordmark "iris" set in Raleway, as on gecode.dev), and
  horizontal and stacked lockups, each in color, one-color, and reversed
  versions.
- `favicon.svg` with a `prefers-color-scheme` variant, `favicon.ico`
  (16, 32, 48), `apple-touch-icon.png` (180), `icon-192.png`,
  `icon-512.png`, and a web manifest.
- A social card (`og-image.png`, 1200×630).
- `brand/README.md` with the palette, clear space, minimum sizes, and
  rules for use, including how to write the name.
- Icons for the VS Code extension, the PyPI and crates.io READMEs, and the
  GitHub repository avatar.

Check every version against WCAG contrast for any text, at 16 px, and in
dark mode.

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
| M0 | Foundations | Repository skeleton, `CONTEXT.md`, ADRs 1 to 11, license, CI scaffolding, brand concepts | The layout exists and builds empty, and a logo direction is chosen |
| M1 | 0.1 Specification | Chapters from §6, grammar, data model, JSON schemas, error code registry, answers to §6.2 | Every item in §6.2 is decided, with a case planned for each |
| M2 | 0.1 conformance suite | Gecode tests ported, example formats, the runner, the Gecode adapter | The suite runs green against Gecode 6.5.0, apart from documented Gecode bugs |
| M3 | Rust core and CLI | `iris-syntax`, `iris-sema`, `iris-core`, `iris-cli` | 100% conformance, performance target met, fuzzing in CI |
| M4a | Python | Wheels, stubs, docs | 100% conformance, wheels on TestPyPI |
| M4b | Tooling | Tree-sitter, TextMate, LSP, VS Code and Zed packages | Every conformance spec parses cleanly, and the LSP features in §8.7 work |
| M4c | C++ | Port, CMake package | 100% conformance, sanitizer runs clean, differential fuzzing against Rust |
| M4d | Java and Kotlin | Core library, Kotlin module | 100% conformance, JMH baseline recorded |
| M5 | Site and playground | Starlight site, the Specification rendered, guides, WASM playground, format pages | Deployed at `iris.gecode.dev` |
| M6 | Skill | `skill/iris`, evals | The eval set passes, for example 8 of 10 formats read correctly on the first attempt |
| M7 | 0.1.0 release | The 0.1 Specification frozen, every package published as 0.1.0, announcement | Every registry has 0.1.0, and the docs are live |

M4a to M4d depend only on M2, plus M3 for Python and the LSP. They can run
in parallel, which is where splitting the work between agents pays off.
The site can start during M1, so the Specification is readable while it is
written.

### Iris 1.0 (the Gecode 7 language)

| # | Milestone | Contents | Done when |
|---|---|---|---|
| M8 | 1.0 design | Ideas from §10.1 reviewed, ADRs, `spec/1.0/` draft, the compatibility rules of §10.2 | The 1.0 Specification is feature-complete as a draft |
| M9 | 1.0 implementations | 1.0 in Rust, C++, Java, and Python, with `iris migrate` and tooling updates, all supporting 0.1 and 1.0 | 100% on the 1.0 suite and on the 0.1 suite (with listed exceptions) |
| M10 | Gecode 7 integration | Decide whether Gecode 7 adopts the C++ library; if so, integrate it and run Gecode's examples and tests on it | Gecode 7 reads Iris 1.0 specs |
| M11 | 1.0.0 release | 1.0 Specification frozen, all packages at 1.0.0 | Released together with Gecode 7 |

---

## 12. Decisions

### Decided

- **C++ and Java are native implementations.** Python binds Rust.
- **Names:** the umbrella name `gecode-iris` on crates.io, PyPI, and npm.
  Python imports `gecode_iris`, Maven uses `dev.gecode:iris`, the C++
  namespace is `iris`, the CLI is `iris`, files use `.iris`, the site is
  `iris.gecode.dev`, and the repository is `Gecode/iris`. A check on
  2026-10-31 found `gecode-iris` free on crates.io, PyPI, and npm,
  `tree-sitter-iris` free on npm, and `Gecode/iris` not yet in use.
  Publishing `dev.gecode` on Maven Central requires verifying ownership of
  the `gecode.dev` domain.
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
- **Float underflow:** a float token whose value is nonzero but rounds to
  a subnormal number or to zero is rejected, as the reference build does.
  This gives every implementation the same portable rule.
- **Work tracking:** zdev, with personal records (`.zdev/` is not
  committed).

### Open

1. **How compatible 1.0 is with 0.1.** *Recommendation:* as in §10.2,
   compatible by default, with breaking changes listed and handled by
   `iris migrate`.
2. **Python floor.** *Recommendation:* 3.10+ with `abi3`.
3. **Logo direction.** Choose among the concepts in `brand/concepts/`
   (§8.11).
