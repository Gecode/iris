# 0014. Integers beyond 64 bits and in other radixes

## Status

Proposed, 2026-11-03. For the Iris 1.0 design
([PLAN.md §10](../../PLAN.md#10-toward-iris-10)).

## Context

Iris 0.1 integers are signed 64-bit and decimal
([§3.4](../../spec/0.1/03-data-model.md#34-integers),
[§4.4](../../spec/0.1/04-input.md#44-integer-tokens)). In a survey of the
2500 most-solved Open Kattis problems, 34 problems have larger integers
([idea entry](../1.0-ideas.md#read-integers-beyond-64-bits)): up to 10^100,
500 or 100000 decimal digits, a million binary digits, base-62 numerals.
A few use octal or other radixes within 64 bits.

Iris 0.1 reads such a number in two ways, and neither checks it. `x: word`
returns the token as a string, whatever it holds. `tokens "0123456789"`
makes each digit a token, and `list digit: 0..9 until eol` reads one
number per line as a digit array; it fails when two numbers share a line,
because the digits run together, and the setting stays on for the rest of
the input.

Gecode and MiniZinc models cannot compute with such numbers either. The
program consuming the data does, so Iris must deliver them exactly and
validate their form, not do arithmetic on them.

## Decision

Add a per-token symbol array, a decimal string type, and a radix option.
The data model does not change.

**`symbols "..."`** reads one ordinary token and maps each character to
its index in the string plus `base`, as an alphabet maps a cell
([§6.10](../../spec/0.1/06-types-and-values.md#610-alphabets)). Characters
are Unicode scalar values, as for [pictures](0013-raw-lines-and-character-matrices.md),
and a repeated character maps by its first occurrence. The result is an
`int` array one depth deeper than the declaration, most significant symbol
first; any other character, including a sign, is `E-SYMBOL`. `base`
defaults to 1, as for an alphabet; digits use `base 0`. Because it reads
one token, several numbers on one line stay apart:

```iris
type decimal = symbols "0123456789" base 0
list n: decimal until eol
check all(d in n: d[0] != 0 || size(d) == 1) "no leading zeros"
```

```input
12345678901234567890123 0 7
```

```data
n = [[1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3], [0], [7]]
```

The same type reads binary (`"01"`), base 62
(`"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"`), and
strings over a small alphabet, such as DNA over `"ACGT"`. Checks see the
digits, so `size` bounds the length. A string sentinel for a list of
symbol arrays is compared with the token before mapping, as in
[§6.9](../../spec/0.1/06-types-and-values.md#69-sentinel-and-end-of-line-lists),
and a numeric sentinel is `E-SPEC-TYPE`; the other terminators work as
for any token list.

**`bigint`** reads one token in the form of a 0.1 integer token,
`[+-]?[0-9]+`, of any length, and reports `E-EXPECTED-VALUE` for any other
token. The value is a `string` holding the number in canonical form: no
`+`, no leading zeros, and `0` for every zero, so `+007` reads as `"7"` and
`-0` as `"0"`. The program converts it with its own big-number type. The
option `digits lo..hi` bounds the number of digits in the canonical form,
without the sign; a count outside it is `E-EXPECTED-VALUE`:

```iris
x: bigint digits 1..101
```

So `bigint` adds to `word` the validation, the digit bound, and one
spelling per number. A format that forbids leading zeros checks them with
`symbols`. A string sentinel for a `bigint` list is compared with the
token before conversion, and a numeric sentinel is `E-SPEC-TYPE`.

**`radix r`** reads an `int` or a range in radix r, an integer literal
from 2 to 36. A token has an optional sign and then digits, with letters
in either case for digits above 9; leading zeros are accepted, as in
0.1. The value must fit in 64 bits, as in 0.1, and is stored as an
ordinary integer, so ranges, sentinels, and offsets apply to it
unchanged: `state: 0..19682 radix 3`.

`bigint` and radix reads can be capture types in patterns; `symbols`,
whose value is an array, cannot.

**Compatibility.** `bigint` is a reading type only where 0.1 would report
`E-SPEC-UNKNOWN-TYPE`, as for `raw` in ADR 0013, and `digits` follows only
it. `symbols` is a reading type only when a string literal follows it,
which is a syntax error in 0.1. `radix` is an option only when an integer
literal follows it, so `x: 1.. radix` keeps its 0.1 meaning, a range up to
a value named `radix`. A spec without these reads exactly as in 0.1.

## Consequences

Every big-integer problem in the survey reads exactly, including several
numbers on a line and binary and base-62 numerals, without the `tokens`
workaround and its effect on later reads.

Checks cannot compare big values numerically. A bound such as at most
10^100 becomes a digit count, or a test on the digit array. If the survey's
consumers need numeric comparison, a later design can add it to `bigint`.

`symbols` is the per-token, one-dimensional companion of an alphabet, and
it decides characters over bytes ahead of 0.1 alphabets.

The Specification needs the three reading types and options, their
errors, the canonical form, the compatibility rules, and their column
rules.

## Alternative considered

A big-integer value type in the data model, serialized as a JSON number of
any length. JSON allows it, but many decoders round such numbers, dzn
consumers cannot use them, and Gecode's instance API has no type for
them. A string keeps the value exact everywhere.

Keeping the token's spelling in `bigint`, and rejecting `+` and leading
zeros. It would differ from 0.1 `int` on the same tokens, and leave each
program to compare spellings of one number.
