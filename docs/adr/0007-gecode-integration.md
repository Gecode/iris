# 0007. Gecode integration

## Status

Accepted, 2026-10-31.

## Context

Gecode 6.5.0 defines Iris 0.1 and already has its own implementation.
Gecode 7 may adopt the standalone C++ library
([PLAN.md §7.3](../../PLAN.md#73-gecode-as-an-implementation),
[§8.4](../../PLAN.md#84-c-cpp), [§12](../../PLAN.md#12-decisions)).

## Decision

Gecode 6.5 keeps its implementation. A conformance adapter links
`libgecodeinstance` and implements the driver protocol without changes to
Gecode itself.

The Iris C++ library is designed for possible adoption by Gecode 7. It
provides an allocation hook for Gecode's heap and errors carrying the
information that `Instance::Error` shows. In Iris 1.0, Gecode supplies its
host constants through the host constant mechanism. CMake supports both
installed use and a vendored subdirectory.

## Consequences

Gecode 6.5.0 remains an independent reference for conformance work.
Adoption by Gecode 7 remains a decision for milestone M10
([PLAN.md §11](../../PLAN.md#11-milestones)); the library must fit Gecode's
needs without depending on Gecode. The standard's revisit condition is
recorded in [ADR 0009](0009-cpp23-and-java27-baselines.md).
