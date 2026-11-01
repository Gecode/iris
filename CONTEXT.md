# Iris glossary

Project terms follow [PLAN.md](PLAN.md). Language terms describe Iris 0.1,
the reader language of Gecode 6.5.0; their precise semantics are defined in
the Specification.

## Project terms

| Term | Meaning | Avoid |
|---|---|---|
| Iris | The declarative language for reading text instance files and the standalone project around it. The name is written Iris in prose and `iris` in code, commands, package names, and the `.iris` file extension. | The name in all capitals. |
| reader spec; spec | A program in the Iris language, such as `dimacs-cnf.iris`, that describes how to read an instance format. This is the meaning of spec in Gecode's `Spec::parse` API and `-spec FILE` option. | Using spec for the language document. |
| Iris Language Specification; the Specification | The normative document for one language version, kept in `spec/`. | Shortening it to spec; lowercase specification when naming the document. |
| instance; input | The text file being read, or its text supplied directly to an implementation. A file described by `instances` can contain several separately selectable instances. | Confusing an input with a reader spec or the resulting data. |
| data | The result of reading: named, typed scalars and possibly ragged arrays, exposed as entries in definition order. | Using data for the input text. |
| implementation | A library that parses reader specs and reads inputs. Rust is the reference implementation, Python binds Rust, and C++ and Java are native implementations, with a Kotlin module for Java. | Treating the Python binding as an independent implementation. |
| host | The program that embeds an implementation, such as Gecode or a Python script. | Using host for the reader spec. |
| language version | A version of the language defined by the Specification: Iris 0.1 is the Gecode 6.5.0 language, apart from the deliberate differences in the Specification's appendix C, and Iris 1.0 develops it further for Gecode 7. | Treating a language version as a package release number. |
| package version | The release version of a particular library or tool, separate from the language versions it supports. The first planned package releases are 0.1.0 and implement Iris 0.1. | Assuming language and package versions are interchangeable. |

## Language terms

| Term | Meaning | Avoid |
|---|---|---|
| statement | An instruction in a reader spec, executed in order against the input. Statements read values, structure reading, define names, or check data. | Assuming every statement consumes input. |
| setting | A top-level declaration that configures the format name or input handling, such as `format`, `comment`, `stop`, `tokens`, `separators`, or `ignore trailing`. | Calling a setting a caller-supplied parameter. |
| value type | The scalar kind of a data entry: `int` for integers, `float` for floating point numbers, or `string` for strings. Its depth describes the array structure separately. | Confusing a value type with a type used to read input. |
| type | A description of how to read and validate input, such as `int`, `float`, `num`, `word`, `rest`, a range, an alphabet, or a named type. | Treating `string` as a built-in reading type. |
| depth | The number of array levels in an entry: 0 for a scalar, 1 for an array, 2 for an array of arrays, and so on. | Using depth for an array's length. |
| entry | A named item in the data, with a value type and a depth. Entries appear in the order their names were defined. | Variable for a data name. |
| path | A sequence of indices, counted from 0, that selects a row or a single value within an entry. An empty path refers to the whole entry. | Confusing an index path with a file path. |
| ragged array | An array whose rows can have different lengths. | Assuming every array is rectangular. |
| record | One iteration's collection of fields in a `repeat`, including lists of nested records. Its fields are stored column-wise as arrays; records themselves are not values in the data. | Row when referring to the repeat iteration rather than an array row. |
| record name | A name that denotes one record when a check ranges over a repeat, such as a field name defined in that repeat or an explicit name introduced by `each`. The contexts where it denotes a record or a whole array are defined in the Specification. | Assuming every record name is a separate data entry. |
| repeat | A statement that executes its body a given number of times, until a terminating token, or until the end of input. Names defined in its body become arrays with one element per repetition. | Treating repeated fields as separate entries for each iteration. |
| list | A statement that reads values until a sentinel or, with `until eol`, the end of the line. The sentinel is consumed but not stored. | Calling every array a list statement. |
| sentinel | A value that terminates a list and is consumed without being stored. | Confusing it with a repeat's optionally stored terminating token. |
| layout | The TSPLIB cell order used when reading a matrix, selected by `layout`, including full, triangular, row-wise, and column-wise forms. The exact cell orders are defined in the Specification. | Confusing input layout with data storage order. |
| offset | An amount added to each number read by a declaration, after checking its range. Triangular layouts mirror the offset values; omitted diagonal cells stay 0. | Treating an offset as part of the range check. |
| range | An integer reading type with expression bounds, such as inclusive `lo..hi` or upper-exclusive `lo..<hi`. The range checks numbers before an offset and does not check a list's sentinel. | Using range for an array's shape. |
| named type | A reading type given a name by `type name = ...`, available to later statements in the same block and its nested blocks. Its bounds are evaluated at its definition. | Treating it as a data entry. |
| alphabet | A reading type for a two-dimensional character array that maps symbols to integer values, with options for blanks, stripped characters, and a base. | Treating the result as an array of strings. |
| line pattern | Literal text with captures and current-value references, used by `line`, `find`, and sections arms to match the rest of a line. Matching details are defined in the Specification. | Calling it a regular expression. |
| capture | A part of a line pattern that reads a typed value, such as `{x:int}`; `{x}` reads a word. An unnamed capture checks the text without storing it. | Calling `{$x}` a capture; it matches a current value. |
| sections | A statement that handles lines in any order through matching arms, until a terminator, end of input, or a `last` arm. Unknown lines are errors unless `skip other` is present. | Assuming arms run in declaration order. |
| arm | A pattern and body within `sections`; the first matching arm handles a line. An arm normally matches exactly once, while `optional` allows no match and `last` ends the sections statement after that arm runs. | Branch when naming a sections arm. |
| check | A statement that evaluates a condition and reports a failure with its message and input location. Checks can range over records; the rules are defined in the Specification. | Assuming a check only sees the last record read. |
| param; parameter | A value supplied by the caller and introduced by a `param` statement, optionally with a default expression. | Confusing a parameter with a setting or a host constant. |
| instances | A statement whose body describes one instance per pass through a file containing several instances. The `instance` parameter selects one by number, counted from 1, or by its `name`; without it, the file contains a single instance. | Assuming reading returns all instances together. |
| catalog | The operation that lists a file's instances rather than retaining one instance's data. Gecode's `Spec::catalog` reports their count, names when all instances define them, and each instance's first failure from a check or a missing or repeated sections line. | Assuming catalog can recover from every reading error. |
| host constant | A predefined value associated with the host: in Iris 0.1, `gecode.int_max` and `gecode.int_min` are fixed at 2147483646 and -2147483646. General host-supplied constants are an Iris 1.0 topic. | Treating these as the limits of Iris's 64-bit integers. |
| work budget | A limit on values visited by expressions, including checks over records, or on repeat iterations and empty rows created without reading input. The accounting and limits are defined in the Specification. | Treating a work budget as a wall-clock timeout. |

## Tooling terms

The following terms describe the planned tooling and conformance contract in
[PLAN.md](PLAN.md#7-conformance-suite).

| Term | Meaning | Avoid |
|---|---|---|
| error code | A stable identifier for a kind of error, separate from its message wording, with codes defined in the Specification. The conformance suite compares the code and location, including the record path for check failures. | Confusing it with a CLI exit code or claiming Gecode already reports these codes. |
| canonical JSON | The common encoding of data and errors, defined in the Specification, that the CLI emits and the conformance suite compares. | Treating an implementation's arbitrary JSON output as canonical. |
| conformance case | A language-neutral test for a particular language version, with a reader spec, any input and parameters, and expected data or an error. Cases can test parsing alone or catalog results as well as reading. | Using case for a sections arm. |
| conformance driver | An implementation-specific command that accepts the suite's `parse`, `read`, `catalog`, and `dzn` requests and returns canonical JSON, or dzn text for `dzn`. | Confusing the driver with the runner. |
| conformance runner | The shared tool that runs conformance cases against a driver and compares the results with their expectations. | Giving each implementation a different definition of conformance. |
| conformance suite | The versioned collection of cases that forms the contract between implementations, including Gecode. | Treating the suite as the normative language document. |
| formatter | The tool, `iris fmt`, that gives a reader spec its one formatted form for a line width by changing only whitespace and line breaks. The line width comes from `.editorconfig`. | Expecting it to rewrite constructs; that is a lint fix or `iris migrate`. |
| lint | A named warning about a valid reader spec that is probably wrong, fragile, or hard to read, reported by the language server and by `iris lint` or `iris check`. The linter is built only if useful lints exist (PLAN.md §8.13). Lints are not normative and have names, not error codes. | Calling a lint an error, or giving it an error code. |
| directive comment | A comment starting with `# iris:` that instructs tooling, such as naming a sample file for live reading or allowing a lint. Implementations ignore it like any comment. | Treating a directive as language syntax. |
