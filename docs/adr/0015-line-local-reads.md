# 0015. Line blocks for reads confined to one line

## Status

Proposed, 2026-11-03. For the Iris 1.0 design
([PLAN.md §10](../../PLAN.md#10-toward-iris-10)).

## Context

Line-aware reads in Iris 0.1 move on when the line is used up: `list ...
until eol` and `rest` read the next line with data instead of returning an
empty value
([§6.9](../../spec/0.1/06-types-and-values.md#69-sentinel-and-end-of-line-lists)).
Everything else reads across lines. So 0.1 cannot read:

- an empty same-line list, such as a target `gmp:` with no dependencies
  (3 problems in a survey of the 2500 most-solved Open Kattis problems:
  `builddeps`, `managingpackaging`, `martensdfs`; [idea
  entry](../1.0-ideas.md#allow-an-empty-same-line-list));
- a varying number of records on one line, such as set scores `6:2 6:4`
  or a line of tokens that are each a number or an operator, except as a
  flat list when every token has one type;
- an optional field in the middle of a line, or one followed by such a
  list.

An optional field at the end of a whole line already reads in 0.1, with a
`sections` statement in a repeat and one `optional last` arm per form
(`"{time} {speed:1..}"`, then `"{time}"` with `let speed = 0`). The arms
repeat the shared fields, and the idiom needs the line to be a record of
its own.

## Decision

Add a `line` block. `line "pattern"` already matches one line against a
pattern; `line { ... }` reads one line with statements:

```iris
tokens ":"
n: 1..
repeat n {
  line {
    target: word
    expect ":"
    list deps: word until eol
  }
}
```

```input
3
gmp:
mpfr: gmp
gecode: gmp mpfr
```

```data
n = 3
target = ["gmp", "mpfr", "gecode"]
deps = [[], ["gmp"], ["gmp", "mpfr"]]
```

**The line.** If data remains on the current line, the block reads the
rest of it. Otherwise it reads the next line: the line that starts at the
position at the start of the input or right after an LF, as for a [raw
read](0013-raw-lines-and-character-matrices.md), and else the line after
the current one. It passes comment lines; a blank line is read as an empty
line. A stop line
([§4.7](../../spec/0.1/04-input.md#47-comment-and-stop-prefixes)) or the
end of the input means no line, which is `E-EXPECTED-VALUE`.

**Reads inside.** No read goes past the end of the selected line:

- a read that finds no data reports `E-EXPECTED-VALUE` at the line's end,
  and so does a [variant read](0012-variant-types.md);
- `list ... until eol` and `rest` return an empty list or `""` when the
  line is used up;
- a sentinel list that reaches the line's end reports the error that 0.1
  reports at the end of input, at the line's end;
- `repeat until eol` runs its body until no data remains on the line,
  possibly zero times; a body that reads nothing is an error, as for
  `repeat until`;
- the function `more()` is 1 while data remains on the line, and 0
  otherwise.

At the end of the block, data left on the line is a new error,
`E-LINE-TRAILING`, unless the block ends with `...`, as a pattern can.
Then the rest of the line, including its LF, is consumed.

These read the other cases:

```iris
repeat until eof {
  line {
    time: word
    if more() {
      speed: 1..
    } else {
      let speed = 0
    }
  }
}
```

```iris
tokens ":"
n: 1..
repeat n {
  line {
    repeat until eol {
      won: 0..7
      expect ":"
      lost: 0..7
    }
  }
}
```

**Placement.** A line block can contain declarations, lists, `expect`,
`let`, `check`, `if`, counted and `until eol` repeats, and variant reads,
so a line of tokens that are each a number or an operator reads as
`repeat until eol { t: token }` with a variant type `token`. Statements
that choose their own line are rejected with `E-SPEC-PLACEMENT`: `line
"..."`, sections, `find`, `skip`, raw and picture reads, nested line
blocks, the other `repeat until` forms, lists `until blank` or `until
eof`, settings, and reads of a variant
type with a line-shape constructor. A variant type's constructor
statements are checked where the type is read, as if they stood there.
`repeat until eol` and `more()` outside a line block are
`E-SPEC-PLACEMENT`.

**Compatibility.** `line {`, `repeat until eol`, and a `...` statement are
syntax errors in 0.1. `more()` is a function call, so it cannot clash with
a value named `more`. A spec without line blocks reads exactly as in 0.1.

## Consequences

Empty lists, empty `rest`, optional fields anywhere on a line, and
per-line records become readable, and the line structure that a format
states is checked: data left over is an error instead of being read as the
next record.

Reading the next line, blank or not, makes a line block sensitive to blank
lines where the rest of the spec is not. A format whose records may be
separated by stray blank lines should not use line blocks for them.

The Specification needs the block, the line selection rule, the confined
read rules, `more()`, `repeat until eol`, the placement rules, and
`E-LINE-TRAILING`.

## Alternative considered

A separate list terminator that may be empty, such as `until line end`,
fixes empty lists alone. It does not give optional fields or per-line
records, and leaves `rest` as it is.

A condition `more` without parentheses reads better, but a 0.1 spec can
already define a value named `more`.
