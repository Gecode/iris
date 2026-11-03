---
title: Tutorial
description: Build reader specs for DIMACS CNF, TSPLIB coordinates, and job shop instances.
editUrl: false
prev: false
next: false
---

Write three reader specs, starting with a header and ending with nested records
and checks. Each step shows the complete reader spec at that stage, the input
it accepts, and the data returned by the real CLI.

1. [Read DIMACS CNF](/tutorial/dimacs/): capture a header, skip comments, and
   read clauses terminated by zero.
2. [Read TSPLIB sections](/tutorial/tsplib/): match keyword/value headers in
   any order and read a coordinate section.
3. [Read job shop records](/tutorial/job-shop/): collect nested records and
   check each job's machine use.

## Before you start

[Build the CLI](/tools/cli/#build-from-source) and put `iris` on your `PATH`.
All commands run from the Iris repository root. The intermediate reader specs
and inputs are files in `site/examples/tutorial/`. You can edit those files in
a local checkout or copy the displayed text into files of your own and adjust
the command paths. Inputs and reader specs must be UTF-8.

Each step finishes with the actual reader spec from the [format library](/formats/),
including the checks and input conventions used by Gecode. The site runs every
example when it is built, so the output shown is the CLI's.

## Find a particular construct

Use the [guides](/guides/) for focused examples of patterns, sections, records,
parameters and several instances. The [Specification](/specification/0.1/)
defines the exact rules, including cases the tutorial does not cover.
