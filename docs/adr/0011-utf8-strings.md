# 0011. UTF-8 input and strings

## Status

Accepted, 2026-10-31.

## Context

Gecode 6.5.0 keeps raw bytes in strings. Iris needs strings that can be
represented as plain text in canonical JSON
([PLAN.md §6.1](../../PLAN.md#61-sources),
[§12](../../PLAN.md#12-decisions)).

## Decision

Iris 0.1 strings are Unicode text encoded as UTF-8. The whole input must
be valid UTF-8, including comments, skipped lines, and bytes that are never
read. A `word` or `rest` value that is not valid UTF-8 is also an error,
because token settings operate on bytes and can split a character. This is
a deliberate difference from Gecode 6.5.0 and belongs in the
Specification's differences appendix.

## Consequences

Canonical JSON keeps strings as plain text. Gecode 6.5.0 accepts input
that Iris 0.1 rejects. The input chapter of the Specification defines when
the input is validated, where the error is reported, and how this error is
ordered relative to others.

The scope was decided on 2026-10-31, after the input chapter's research
showed that Gecode accepts any bytes in comments, skipped lines, literal
pattern text, and ignored tails. Validating only string values was the
narrower alternative.

## Alternative considered

Keeping Gecode 6.5.0's raw-byte strings would preserve its behavior but
would not provide the chosen Unicode text model for canonical JSON.
