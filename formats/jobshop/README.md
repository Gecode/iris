# Job shop

[jobshop.iris](jobshop.iris) reads job and machine counts, then one
`(machine, duration)` pair per operation in each job. Machine numbers
start at 0. Free text, `#` comments, and `+++` separators are skipped.
Each job must use every machine once. Data contains the counts and
two-dimensional `machine` and `duration` arrays, plus `name` when present.

The format sources are the
[OR-Library job shop documentation](https://people.brunel.ac.uk/~mastjjb/jeb/orlib/jobshopinfo.html)
and the format description at the start of
[jobshop1.txt](https://people.brunel.ac.uk/~mastjjb/jeb/orlib/files/jobshop1.txt),
also used by [JSPLIB](https://github.com/tamy0612/JSPLIB). The reader spec
is copied verbatim from
[Gecode's job-shop.cpp](https://github.com/Gecode/gecode/blob/599f977b7855dba2a1a5f67db1892722372067d3/examples/job-shop.cpp#L94).

- [or-library](cases/or-library/) has three jobs and three machines,
  a description, separators, and `instance handmade-or`. Its
  `params.json` selects that instance by name.
- [jsplib](cases/jsplib/) has a different 3×3 instance with comment
  headers and no instance name.
- [duplicate-machine](cases/duplicate-machine/) uses machine 0 twice
  in the first job. The reference reports `E-CHECK-FAILED` with record
  path `[0]`.

The `instances` statement also supports files with several instances;
these samples each contain one. These are hand-made samples under the
repository's MIT license. See the [library README](../README.md) for case
layout, validation, and Gecode notices.
