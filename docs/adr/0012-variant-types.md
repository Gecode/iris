# 0012. Variant types for values with several forms

## Status

Proposed, 2026-11-03. For the Iris 1.0 design
([PLAN.md §10](../../PLAN.md#10-toward-iris-10)).

## Context

Many inputs give one position several forms: a command keyword followed by
its own arguments (`fd 10`, `lt 90`), an integer or `undo 2`, a size `2`
or `2L`, a range `3-9` or a single number `7`, a name before or after the
numbers on its line. In a survey of the 2500 most-solved Open Kattis
problems, 96 of 553 specced formats have such values ([idea
entry](../1.0-ideas.md#add-variant-types-for-values-with-several-forms)).
What tells the forms apart:

| Told apart by | Problems | Example |
|---|---:|---|
| A keyword | 59 | `fd 10`, `lt 90` |
| The shape of a line or token | 32 | `3-9` or `7`; `2L` or `2` |
| The type of the first token | 18 | `-3` or `undo 2`; `(`, `)` or `17` |
| Position on the line | 8 | `bob 3 4` or `3 4 bob` |

Iris 0.1 reads keyword forms with `op: word` and `if`. When each form
fills a whole line, a `sections` statement with one `optional last` arm
per form, inside a repeat, reads one line per iteration in whichever form
it takes:

```iris
n: 1..30
k: 1..100
repeat k {
  sections {
    "undo {count:1..}" optional last {
      let position = 0
    }
    "{position:-10000..10000}" optional last {
      let count = 0
    }
  }
}
```

```input
5 3
-3
undo 1
7
```

```data
n = 5
k = 3
count = [0, 1, 0]
position = [-3, 0, 7]
```

Both idioms have the same cost: every field that only one form reads must
be padded in every other branch or arm, because a scalar defined in only
some repetitions of a repeat is an error
([§7.5](../../spec/0.1/07-structure.md#75-column-construction-and-missing-rows)).
That rule caused 4 of the 6 failed first attempts in one survey batch. No
entry records which form a line took unless each arm also defines one.

Forms that share a line with other values cannot be read with types at
all: a list of tokens that are each `3-9` or `7`, sizes `2L` and `2` among
other values, the tokens of an expression. An `if` can only test what has
already been read, a pattern matches a whole line, and the reader cannot
look at a token without consuming it. These read only as `word`, without
conversion or range checks.

The 0.1 data model has no record or optional values: an entry is an `int`,
`float`, or `string` value at some depth
([§3.2](../../spec/0.1/03-data-model.md#32-depth-arrays-and-paths)).

## Decision

A `type` definition can list constructors. Each constructor has a name, a
recognizer, and a body of fields:

```iris
type command =
  | undo "undo" { count: 1.. }
  | throw { position: -10000..10000 }
  default count = 0
  default position = 0

n: 1..30
k: 1..100
repeat k {
  c: command
  check c != "undo" || count <= k "cannot undo more commands than there are"
}
```

With the input above:

```data
n = 5
k = 3
c = ["throw", "undo", "throw"]
count = [0, 1, 0]
position = [-3, 0, 7]
```

**Reading.** A declaration of a variant type tries the constructors in
source order and commits to the first whose recognizer matches. After it
commits, the constructor's body runs; a failure there is a hard error, and
no later constructor is tried. If data remains but no recognizer matches,
reading reports a new error, `E-VARIANT`, listing what each recognizer
expected. If no data remains, at the end of input or at the end of the
line in a [line block](0015-line-local-reads.md), it reports
`E-EXPECTED-VALUE`.

**Recognizers.** Each looks at no more than the next token or the rest of
the current line, after normal skipping, and consumes nothing unless it
matches:

| Recognizer | Form | Matches when | Consumes |
|---|---|---|---|
| Keyword | `undo "undo" { ... }` | The next token equals one of the listed strings, as `repeat until` compares tokens. | The token |
| Token shape | `range token "{lo:int}-{hi:int}"` | The next token matches the pattern under [§7.2](../../spec/0.1/07-structure.md#72-line-patterns-and-captures). | The token; captures become fields |
| Line shape | `before line "{name} {a:int} {b:int}"` | The rest of the line matches the pattern, as for [`line`](../../spec/0.1/07-structure.md#73-line-find-skip-and-expect). | The line; captures become fields |
| First field | `throw { position: int }` | The next token converts to the first field's type. | Nothing; the body reads it |

A token-shape pattern containing a pattern blank is `E-SPEC-PATTERN`,
because a token holds none. A first-field recognizer needs a body that
starts with a scalar declaration of `int`, `float`, `num`, `word`, a
range, or a named type of one of these; otherwise parsing reports
`E-SPEC-TYPE`. A line-shape constructor is the sections idiom above in
one place: it reads like an `optional last` arm, and the tag and defaults
replace the padding.

Recognition uses conversion only, as a line pattern does: a token that
converts but fails a range commits the constructor and then fails. So
`| small { x: 1..5 } | large { x: 6..10 }` cannot tell `7` apart. Parsing
rejects a constructor that an earlier one makes unreachable, with
`E-SPEC-TYPE`, in these cases only:

- a keyword that an earlier constructor lists;
- a keyword, token-shape, or first-field constructor after a first field
  of type `word`;
- a first field after an earlier first field of the same
  [radix](0014-integers-beyond-64-bits.md) whose type converts every
  token the later one does: `float` or `num` before any numeric type, and
  `int` or a range before `int` or a range.

Line shapes are not compared, and token shapes only with `word`.

A constructor with no body is an enumeration value:
`| remove "#"`.

**Bodies.** A constructor body can contain declarations, lists, `expect`,
`let`, `check`, `if`, counted repeats, and reads of other variant types.
Statements that choose their own line or change how later input is read
are rejected with `E-SPEC-PLACEMENT`: `line`, sections, `find`, `skip`,
`repeat until`, settings, and the [raw and picture
reads](0013-raw-lines-and-character-matrices.md). A type is visible only
after its definition
([§6.5](../../spec/0.1/06-types-and-values.md#65-named-types-and-visibility)),
so a body cannot use its own type, and variants are not recursive.
Placement is checked where the type is read, as if its constructors'
statements stood there: a body can use `more()` only if every read of the
type is in a line block.

**Names.** The declared name holds the tag: the constructor's name, as a
`string`. A field is any name that a constructor defines: a capture of its
recognizer, or a declaration, list, `let`, or nested variant read in its
body. Fields are defined in the current frame, as captures are. Two
constructors can share a field when its value type and depth agree;
otherwise parsing reports `E-SPEC-TYPE`.

**Defaults.** After the constructors, `default name = expression` gives a
scalar field its default, once per field. The expression is constant and
evaluated when the type is defined. It has the field's value type but is
not range-checked, so `default count = 0` with `count: 1..` marks absence
with a value no input can give. When the selected constructor's run leaves
a field undefined, the field takes its default. A scalar field needs a
default unless every constructor defines it unconditionally, in its
recognizer or in its body outside any `if`. An array field has no default
and follows the 0.1 rules: outside a repeat the entry is absent, and in a
repeat the row is empty. Each violation is `E-SPEC-TYPE`.

A name that a variant read defines, the tag or a field, must not be
defined by any other statement in the same frame
([§7.1](../../spec/0.1/07-structure.md#71-statement-execution-and-frames)),
including a second read of the same type, unless the two are in different
branches of one `if`; otherwise parsing reports `E-SPEC-NAME-CONFLICT`.
This replaces 0.1's silent replacement
([§6.3](../../spec/0.1/06-types-and-values.md#63-names-discarded-reads-and-replacement))
for these names, because a default would overwrite a field that an
earlier read defined. Two commands on one line are read in a repeat.

**Columns and order.** Column discovery in a repeat
([§7.5](../../spec/0.1/07-structure.md#75-column-construction-and-missing-rows))
expands a variant read into the tag, then its fields in order of first
occurrence across the constructors in source order. A field's first
definition in the type supplies its fallback metadata. Outside a repeat,
entries are defined in the same order: the tag, then the fields.

**Output.** No change to the data model: the tag is a `string` entry and
each field is an ordinary entry, so JSON and dzn render them as in 0.1. In
a repeat they are columns of equal length, which dzn can index together.

**Checks.** Compare the tag with a string, as in the example above. `iris
check` reports a string compared with a tag that names no constructor.

**Uses.** A variant type can be used wherever a scalar declaration can,
including inside a repeat and in a line block. It cannot be a list
element, an array element, or a capture type. Lists of variants on one
line are read with `repeat until eol` in a line block. A type with a
line-shape constructor cannot be read in a line block, which confines
reads to a line it has already chosen; that is `E-SPEC-PLACEMENT`.

These cover the other kinds in the survey:

```iris
type size =
  | large token "{n:int}L"
  | small { n: int }

type order =
  | before line "{name} {a:int} {b:int}"
  | after line "{a:int} {b:int} {name}"
```

**Compatibility.** A type definition starting with `|`, a constructor
body, and a `default` clause, where `default` is followed by a name, are
syntax errors in 0.1. A spec without constructors reads exactly as in
0.1.

## Consequences

The padding rule disappears for the common case, and the dzn output stays
rectangular because the defaults are declared. Token-shape and
first-field forms that share a line with other values become readable
and typed, and every form gets a tag.

The name rule is stricter than 0.1 for names that variant reads define,
and reading two values of one variant type in a block needs a repeat. If
specs need it, a later design can add a prefix for field names.

The Specification needs the constructor grammar, the recognizer rules,
the unreachability rules, the body and placement rules, `E-VARIANT`, and
the column rules for tags and defaults. Every implementation needs
one-token lookahead without consuming, which the `repeat until` test
already requires, and line-pattern candidates, which `find` and sections
already use.

Recursive input, such as nested expressions and trees, stays out of reach:
4 survey problems need it, and their output would need a tree in the data
model.

## Alternative considered

Optional values in the data model, `null` in JSON and `<>` in dzn (MiniZinc
`opt` types), instead of declared defaults. It represents absence exactly,
but changes the data model, every binding, and Gecode's instance API.
Records as JSON objects were rejected for the same reason.

A `match` statement with arms, instead of a type, keeps everything in one
place but cannot be reused, and leaves the padding rule to each spec.

Keeping only the sections idiom, and documenting it. It covers whole-line
forms, but not forms that share a line, and each arm still pads every
field.

## Condition to revisit

Revisit the defaults if users of the dzn output ask for absent values, or
if Gecode 7 adds optional values to its instance API.
