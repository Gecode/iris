# Iris

Iris is a declarative language for reading text instance files, such as
benchmark files for constraint and SAT solvers. A program in Iris, called a
reader spec, describes the layout of a file. An implementation reads the file
with it and returns named, typed arrays of data, or an error with its line and
column.

The language comes from Gecode 6.5.0, which added it as `gecode/instance`
([Gecode/gecode#270](https://github.com/Gecode/gecode/pull/270)). This
repository makes it a standalone project with its own versions:

- **Iris 0.1** is the Gecode 6.5.0 language, written down precisely, with
  the few deliberate differences listed in the Specification's appendix C.
- **Iris 1.0** develops the language further and is the language of Gecode 7.

## Status

Iris is in planning. Nothing is implemented and there are no releases yet.
[`PLAN.md`](PLAN.md) describes the goals, the architecture, every
deliverable, and the milestones.

## An example

This reader spec reads DIMACS CNF files, the input format of SAT solvers:

```
format dimacs-cnf

comment "c"
stop "%"
line "p cnf {vars:0..} {clauses:0..}"
repeat clauses {
  list clause: -vars..vars until 0
}
```

A line that starts with `c` is a comment, unless a letter or digit follows the
`c` (so `c foo` is a comment and `cfoo` is not). A line that starts with `%`
ends the data, as in the SATLIB files. The problem line gives `vars` and
`clauses`. Each clause is a list of literals between
`-vars` and `vars`, ended by a `0` that is not stored. The data holds the
integers `vars` and `clauses` and the ragged array `clause`, with one row per
clause.

## Repository layout

| Directory | Contents |
|---|---|
| [`spec/`](spec/) | The Iris Language Specification (normative) |
| [`conformance/`](conformance/) | The conformance suite, its runner, and the Gecode adapter |
| [`formats/`](formats/) | Reader specs for standard benchmark formats |
| [`rust/`](rust/) | The Rust reference implementation, the `iris` CLI, and the language server |
| [`python/`](python/) | The Python package, a binding to the Rust crate |
| [`typescript/`](typescript/) | The TypeScript package, a WebAssembly binding to the Rust crate |
| [`cpp/`](cpp/) | The C++ library |
| [`java/`](java/) | The Java library and its Kotlin module |
| [`tree-sitter-iris/`](tree-sitter-iris/) | The tree-sitter grammar |
| [`editors/`](editors/) | The TextMate grammar and editor packages |
| [`skill/`](skill/) | The agent skill for writing reader specs |
| [`site/`](site/) | The documentation site |
| [`brand/`](brand/) | The logo and favicon |
| [`docs/adr/`](docs/adr/) | Architecture decision records |

`CONTEXT.md` defines the terms used across the project.
[`docs/1.0-ideas.md`](docs/1.0-ideas.md) collects candidate changes for Iris
1.0. [`AGENTS.md`](AGENTS.md) has the conventions for contributors and coding
agents.
[`RELEASING.md`](RELEASING.md) is the entry point for component releases and
site deployment.

## License

Iris is released under the [MIT License](LICENSE), as is Gecode.
