---
title: Guides
description: Focused examples of Iris patterns, sections, records, instances, parameters, diagnostics and budgets.
editUrl: false
prev: false
next: false
---

Each guide explains one part of a reader spec with an input and the real CLI
result. Commands run from the repository root with `iris` on your `PATH`;
see [building the CLI](/tools/cli/#build-from-source).

- [Patterns](/guides/patterns/): match headers, capture typed fields and refer
  to a value already read.
- [Sections](/guides/sections/): read keyword lines in different orders and
  handle required, optional and repeated arms.
- [Records and checks](/guides/records-and-checks/): store repeated fields as
  columns and check each record or a whole collection.
- [Several instances](/guides/instances/): list a catalog and select by ordinal
  or name, including files with rejected instances.
- [Parameters](/guides/parameters/): supply caller values, choose defaults and
  handle conversion failures.
- [Errors](/guides/errors/): read the location, caret and execution context,
  and find an error code's definition.
- [Performance](/guides/performance/): understand input progress and work budgets.

For a continuous walkthrough, start with the [tutorial](/tutorial/).
For exact rules, use the [Specification](/specification/0.1/).
