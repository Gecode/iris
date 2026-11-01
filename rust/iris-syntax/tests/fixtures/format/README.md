# Formatter rule snapshots

`rules.iris` combines the [formatting rules](../../../../../docs/formatting.md)
in one valid reader spec. `rules.40.txt`, `rules.80.txt` and `rules.100.txt` hold
reviewed output at those widths. Tests compare exact text; they never update
these files.

| Rules | Coverage in the reader spec |
|---|---|
| R1–R3 | File comment, trailing comment interrupting a declaration, blank lines, and a comment between arms. Source-edge and CR cases also have direct assertions in `tests/format.rs`. |
| R4–R7 | Original numeric and string spellings, unary operators and power, declarations, dimensions, bounded ranges, field and index suffixes, and lexical field-dot exceptions. |
| R8–R10 | Settings, glued format name, list sentinel, repeated alphabet options, repeat and sections headers, bodyless arms, modifiers, and conditional joins. |
| R11–R14 | Width-dependent messages, option groups, vertical string lists, precedence breaks, calls, comprehensions, nested calls, and complex indices. Unicode scalar and exact-fit boundaries also have direct assertions. |

The same-tree check retains recursive node boundaries and comment tokens.
A separate comparison protects complete strings, patterns and comments,
including whitespace leaves inside pattern captures. Every snapshot also
checks idempotence and syntax acceptance.
