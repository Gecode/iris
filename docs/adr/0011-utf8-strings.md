# 0011. UTF-8 strings

## Status

Accepted, 2026-10-31.

## Context

Gecode 6.5.0 keeps raw bytes in strings. Iris needs strings that can be
represented as plain text in canonical JSON
([PLAN.md §6.1](../../PLAN.md#61-sources),
[§12](../../PLAN.md#12-decisions)).

## Decision

Iris 0.1 strings are Unicode text encoded as UTF-8. A `word` or `rest`
value that is not valid UTF-8 is an error. This is a deliberate difference
from Gecode 6.5.0 and belongs in the Specification's differences appendix.

## Consequences

Canonical JSON keeps strings as plain text. Gecode 6.5.0 can accept string
values that Iris 0.1 rejects. The input chapter of the Specification still
needs to decide whether UTF-8 validity covers all input or only string
values; this record does not settle that scope.

## Alternative considered

Keeping Gecode 6.5.0's raw-byte strings would preserve its behavior but
would not provide the chosen Unicode text model for canonical JSON.
