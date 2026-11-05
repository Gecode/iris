# TSPTW

[tsptw.iris](tsptw.iris) reads the node count (including the depot), a
square travel-time matrix, then each node's ready and due times. Node 0
is the depot. The spec requires each time window to be ordered and the
depot to open at time 0. Data contains `n`, `dist`, `ready`, and `due`.

The format source is López-Ibáñez and Blum's
[TSPTW instance format](https://lopez-ibanez.eu/tsptw-instances).
The reader spec is copied verbatim from
[Gecode's tsptw.cpp](https://github.com/Gecode/gecode/blob/599f977b7855dba2a1a5f67db1892722372067d3/examples/tsptw.cpp#L126).

- [three-nodes](cases/three-nodes/) has a depot and two customers, with
  asymmetric travel times and valid windows.
- [reversed-window](cases/reversed-window/) gives the first customer
  ready time 8 and due time 2. The reference reports `E-CHECK-FAILED` at
  line 6, column 1, with record path `[1]`.

These are hand-made samples under the repository's MIT license. See the
[library README](../README.md) for case layout, validation, and Gecode notices.
