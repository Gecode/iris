# Gecode conformance adapter

This C++17 driver implements the [conformance protocol](../../README.md#four-driver-commands)
using the pinned Gecode 6.5.0 public instance API. It belongs to
[PLAN §7.3](../../../PLAN.md#73-gecode-as-an-implementation), milestone M2.
The [smoke cases](../../cases/0.1/smoke/) cover canonical data, dzn, catalog,
parse-only execution, parameters, and diagnostic locations and witnesses.

## Build and run

Dependencies are Bash, Git, CMake 3.21+, a C++17 compiler with floating point
`std::to_chars`, and optionally Ninja. The build first invokes
[gecode-reference.sh](../../../scripts/gecode-reference.sh), then consumes its
installed `Gecode::gecodeinstance` CMake target. No JSON library is vendored or
fetched: the local parser accepts exactly the protocol's ordered string map.

From the repository root:

```sh
adapter=$(conformance/adapters/gecode/build.sh)
ctest --test-dir "$(dirname "$adapter")" --output-on-failure
(cd rust && cargo run --locked -p iris-conformance -- run \
  --driver "\"$adapter\"" --driver-id gecode-6.5.0-599f977b7855 \
  ../conformance/cases/0.1/smoke)
```

The build writes logs to stderr and only the absolute executable path to
stdout. It works from any directory. The reference and adapter share the
`IRIS_CACHE_DIR` convention (default: the repository's `.cache/`); a relative
cache path is resolved against the invocation directory. Adapter products live
under `conformance/gecode/<reference-sha>/build/` within that cache. Existing
CMake generators are preserved; new builds prefer Ninja when available.
`CMAKE_BUILD_PARALLEL_LEVEL` controls build parallelism.

The executable accepts these commands, with options after positional arguments:

```text
iris-gecode-driver parse SPEC [--params JSON] [--version 0.1]
iris-gecode-driver read SPEC INPUT [--params JSON] [--version 0.1]
iris-gecode-driver catalog SPEC INPUT [--params JSON] [--version 0.1] [--format json|dzn]
iris-gecode-driver dzn SPEC INPUT [--params JSON] [--version 0.1]
```

For example, `"$adapter" read /absolute/spec.iris /absolute/input.bin` emits
canonical JSON. `parse` only parses and resolves; it executes no defaults.
`catalog --format dzn` renders the catalog result, while `dzn` reads one
instance and renders its data. Omitted params and `{}` both mean an empty
map. Values are decoded once and passed pair by pair, preserving member
order, commas, whitespace, and numeric text.

## Diagnostics and reference limits

The driver returns 0 for success, 1 for canonical language-error JSON,
2 for request misuse, and 3 for host or adapter failure. It prepares the
complete result before writing stdout, including for dzn. A write failure
cannot retract bytes already accepted by the output device.

[errors.md](../../../spec/0.1/errors.md) owns the diagnostic patterns. The
adapter table keeps all alternatives with their phase and code; CTest compares
those complete triples with the registry. It also checks params decoding,
canonical float and string encoding, diagnostic envelopes, and check witnesses.
Unknown messages produce `E-UNMAPPED` with the full reference diagnostic;
the runner always treats that sentinel as an adapter failure. Ambiguous or
truncated unescaped diagnostic envelopes fail with status 3 and retain the
available reference diagnostic on stderr, except for the verified NUL case
below.

### NUL in a quoted input value

At the pin, `Gecode::Instance::Error` retains its full diagnostic in a
protected `std::string`, but `what()` exposes a C string. A NUL in a quoted
input value therefore cuts off `what()` before the closing quote, excerpt,
and footer. The adapter copies the exception into a derived object to read
the retained string through legal protected access. It uses that string only
under this narrow rule:

1. The reference threw a read-time language error, with an input location.
2. The text before the first NUL matches exactly one template and one
   opening quote for the input-valued slot in the table below. All text up
   to that slot must match; the remaining text is the partial quoted value.
3. At the reported byte line and column, the actual input starts with that
   partial value immediately followed by NUL. Comparison accounts for the
   reference's `\t` spelling of tab; it cannot cross a physical line.
4. The retained diagnostic has exactly one complete excerpt/footer decoding
   and maps to the same code as the matched template.

| Template | Input-valued slot |
|---|---|
| `expected {D}, found {F}` | The quoted `found` token, including named types and ranges. |
| `expected {Q}, found {F}` | The quoted `found` token, not the expected literal. |
| `unexpected symbol {Q}, expected one of {Q}` | The first quote, containing the offending byte. |
| `unexpected input after the end of the spec: {Q}` | The remaining line, with trailing line blanks trimmed. |

The adapter preserves the complete retained message, input location, context,
and reader-spec location. This includes the full token and suffixes such as
the expected alphabet. Canonical JSON escaping encodes NUL as `\u0000`;
the reference's existing message spelling of tab and newline is preserved.
No token boundaries or missing text are inferred from the reader spec.

The quoted-slot audit excludes parameter values, matrix-layout expressions,
instance selectors, expected literals, alphabets, and line patterns from
this recovery: those slots describe parameters or reader-spec values, not
input at the reported location. They may contain strings originally read
as words, but that alone does not establish the required byte match. NUL
only in an excerpt or footer, an unknown template, a mismatching input
prefix, an ambiguous quote or envelope, and a truncated diagnostic without
the retained complete string still fail with status 3 and empty stdout.
CTest exercises these rejection boundaries and all four accepted templates.

The adapter preserves Gecode behavior at the pin. It does not implement the
Iris-only whole-source, whole-input, or value-level UTF-8 checks in
[appendix C.1–C.2](../../../spec/0.1/appendix-c-differences.md#c1-unicode-strings).
If a returned string cannot be serialized as Unicode, both JSON and dzn
requests fail with status 3 and empty stdout. Valid NUL survives serialization.
`E-SPEC-UTF8` and `E-UTF8` are the only registered codes without Gecode patterns.

The reference's [budget arithmetic limit](../../../spec/0.1/appendix-c-differences.md#c3-exact-budget-arithmetic)
and [dzn float printer gap](../../../spec/0.1/appendix-c-differences.md#c4-dzn-floats-just-below-1e17)
also remain unchanged. Smoke values avoid the printer gap. An adapter-only
sanitizer build does not instrument the reference archive or establish safety
at the enormous undefined arithmetic boundary.
