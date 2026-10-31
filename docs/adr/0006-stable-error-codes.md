# 0006. Stable error codes

## Status

Accepted, 2026-10-31.

## Context

Gecode reports errors as messages. Exact message wording would make
conformance depend on prose rather than the error and its location
([PLAN.md §4.4](../../PLAN.md#44-errors-get-stable-codes)).

## Decision

The Iris 0.1 Specification defines stable error codes, such as `E-RANGE`
and `E-CHECK-FAILED`. Conformance compares the code, line, and column,
plus the record path for check failures.

The Specification recommends a message for each code, but message wording
is not normative and is not tested. The Gecode adapter maps Gecode's
messages to codes.

## Consequences

Implementations can provide idiomatic diagnostics while reporting the
same kind of error at the same location. The adapter supplies codes
without changing Gecode 6.5.0. If Gecode 7 adopts the Iris C++ library,
it gets these codes as well.
