# TSPLIB

[tsplib.iris](tsplib.iris) reads explicit edge weights in TSPLIB matrix
layouts and coordinates for `EUC_2D`, `CEIL_2D`, `ATT`, and `GEO`. The
dimension must precede sections that use it. `COMMENT` lines are skipped;
the reader accepts only the header keys and sections listed in the spec.
This is a TSP reader, not the complete family of TSPLIB problem formats.

The format source is Gerhard Reinelt's [TSPLIB 95 documentation](https://comopt.ifi.uni-heidelberg.de/software/TSPLIB95/tsp/tsp95.pdf),
particularly the specification and data parts. The reader spec is copied
verbatim from
[Gecode's tsp.cpp](https://github.com/Gecode/gecode/blob/5e7bbd5c26b0702c6fd652356b2289da0710ac3c/examples/tsp.cpp#L246).

- [explicit-five](cases/explicit-five/) stores a symmetric 5×5
  `FULL_MATRIX` as the `weight` entry.
- [euc-2d-five](cases/euc-2d-five/) stores five node IDs and the float
  arrays `x` and `y`. Expected data contains coordinates, including `1.5`;
  distance construction belongs to the Gecode example's host code.
- [missing-weights](cases/missing-weights/) declares `EXPLICIT` without
  an `EDGE_WEIGHT_SECTION`. The reference reports `E-CHECK-FAILED` with
  an empty record path.

These are hand-made samples under the repository's MIT license. See the
[library README](../README.md) for case layout, validation, and Gecode notices.
