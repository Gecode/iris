# 0009. C++23 and Java 27 baselines

## Status

Accepted, 2026-10-31.

## Context

The native libraries need explicit language baselines. Gecode 7's planning
leans toward C++20 for conservative build environments, including academic
HPC systems. Java 27 is the feature release chosen by the plan
([PLAN.md §8.3](../../PLAN.md#83-java-and-kotlin-java),
[§8.4](../../PLAN.md#84-c-cpp), [§12](../../PLAN.md#12-decisions)).

## Decision

Use C++23 for the C++ library and Java 27 for the Java library.

## Consequences

The C++ API can use facilities such as `std::expected`; its consumers need
C++23 support. The Java library can use the modern Java features listed in
PLAN.md §8.3, and Java 27 is its baseline.

## Alternatives considered

C++20 may be needed for Gecode 7's build environments. An LTS Java
baseline may be needed by users.

## Conditions to revisit

Revisit C++23 if Gecode 7 adopts the library and settles on C++20.
Revisit Java 27 if users need an LTS.
