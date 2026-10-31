# A Formal grammar

## A.1 Grammar source and notation

[grammar.ebnf](grammar.ebnf) is the normative formal grammar for Iris 0.1.
It uses the notation of [§2.2](02-notation.md#22-grammar-notation):
productions, concatenation, alternatives, optional groups, repetitions,
quoted terminals, comments, and named special sequences. This appendix
explains how to read its three layers; the productions occur only in that
file. [Chapter 5](05-reader-specs.md) supplies the lexical, placement, and
limit rules that accompany them.

Implementations MUST apply the named predicates and constraints in addition
to the ordinary EBNF productions. Each special sequence has an explaining
comment in the grammar. Lookahead predicates consume nothing. Token and
character special sequences consume the token or character they identify;
the capture-type sequence consumes the described decoded text.

## A.2 Source and tokens

The lexical layer's `source` production describes source bytes after
[strict UTF-8 validation](05-reader-specs.md#51-reader-specs-and-parsing).
Its scalar-character sequences consume complete UTF-8 encodings. Only
strings and source comments admit non-ASCII characters. Whitespace and
comments are ignored when constructing the token stream. The lexical
repetitions and punctuation choice MUST consume maximally, subject to the
numeric-candidate and host-name rules in the grammar.

The token-level entry production is `reader_spec`. Here a quoted keyword
matches an identifier token of that spelling, and quoted punctuation
matches a punctuation token. Neither can match a string token with the
same text. `ident`, `integer`, `float`, and `string` each consume one token
of their respective class; `end` requires no remaining tokens.

The source-validity predicate refers to the whole source, before tokenization.
A token retains whether it is glued to its predecessor. Both pieces of
information are needed in addition to token kind and text. Reader-spec
line numbers count physical LF bytes; token-level parsing does not discard
their diagnostic positions.

## A.3 Dispatch and lookahead

The following predicates make the reference's context-dependent choices
explicit. Their exact definitions are comments beside the productions.

| Predicate group | Purpose |
|---|---|
| Declaration ahead / no declaration | Recognize an identifier, balanced dimension brackets, and colon; select declarations before statement keywords. |
| Ordinary top-level dispatch | Reserve top-level `format` for metadata even before a colon, and dispatch an actual `instances` separately. |
| Option ahead | End options before a declaration with an option word as its name. |
| Base, alphabet, named, and range type ahead | Select the type form in priority order, using named-type visibility at that source position. |
| Upper bound ahead / no upper bound ahead | Decide whether a range has an upper expression using token kind, declaration lookahead, and the exact keyword stop set. |
| Glued format join | Join only adjacent `-`/`.` and identifier/integer components to the format name. |
| Not eol / line / until / eof | Preserve keyword priority in list endings, skip forms, and repeat forms. |
| Else if ahead | After `else`, permit ordinary declaration-first statement dispatch only for an initial identifier spelled `if`. |
| Host constant / ordinary expression name ahead | Prevent host-constant tokens from acting as callees. |
| Comprehension ahead / no comprehension ahead | Select comprehension syntax only for `all`, `any`, `sum`, or `count` followed by identifier and `in` after `(`. |
| Maximal expression and expression counter | Continue all expression operators, calls, and postfixes greedily and apply the expression counter. |

The grammar's top-level and ordinary-block productions express placement:
settings occur only at the top level, `instances` ends executable statements,
and `each` is optional only at the beginning of a repeat body. The block
and else-if counter sequences add the operational constraints in
[§5.12](05-reader-specs.md#512-parser-limits). The sections outer braces
have no counter entry.

The maximal-expression constraint matters at statement boundaries. EBNF
optionality alone MUST NOT permit an expression to stop early to make a
later statement parse. For example, `let x=1` followed on another line by
`+2` still contains the expression `1+2`. Similarly, declaration-first
dispatch MUST NOT fall back to a keyword statement when a declaration's
contents are malformed.

## A.4 Decoded line patterns

A `pattern` consumes one string token, decodes its four source escapes,
and recognizes the complete resulting value with `decoded_pattern`.
This layer has no implicit whitespace skipping. Its whitespace is exactly
space, tab, and CR. A pattern literal can contain LF produced by a source
escape, and a closing brace on its own is literal text.

An opening brace begins a capture or current-value reference and MUST end
at the first closing brace. Names in these positions use ordinary ASCII
name syntax without trimming. A missing capture name is permitted. A
missing colon selects `word`; a colon requires a nonempty type token stream.
The captured type text MUST be re-lexed and parsed with the same type
production and visible types as the containing reader spec. Its starting
line is the outer pattern token's source line; decoded LF in the type text
can advance that line. Alphabet reading types, including named aliases,
MUST be rejected in captures with `E-SPEC-PATTERN`.

A terminal `...` permits only space and tab after it. CR is a pattern blank
elsewhere but is not permitted after `...`. A `{` inside quoted capture-type
text still starts part of the text that ends at the first `}`; quoting
inside a type does not introduce balanced-brace scanning.

[§7.2](07-structure.md#72-line-patterns-and-captures) defines matching,
whitespace requirements, capture conversion, and current-value substitution.
Pattern syntax in the grammar does not define those execution effects.

## A.5 Syntax and other parse-time checks

A grammar recognizer checks token structure, the named predicates, pattern
syntax, placement, and the parser counters. Successful recognition is
necessary but not sufficient for a reader spec to parse and resolve.
Definitions with reserved names, function requirements, type compatibility,
name conflicts, and record-scope rules can still fail. Their codes are in
[chapter 11](errors.md#114-parse-time-codes); their semantic definitions
belong to [chapter 6](06-types-and-values.md),
[chapter 7](07-structure.md), [chapter 8](08-expressions-and-checks.md),
and [chapter 9](09-params-and-instances.md).

An unknown bare type name fails during type dispatch rather than forming
a valid range. A grammar checker can therefore reject that source while
its diagnostic remains `E-SPEC-UNKNOWN-TYPE`. Conversely, an ordinary call
with an unknown function name has recognizable call syntax. Conformance
checks MUST distinguish these cases from structural syntax negatives.
