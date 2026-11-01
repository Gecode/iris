use lsp_types::{MarkupContent, MarkupKind};

const SPECIFICATION_URL: &str = "https://github.com/Gecode/iris/blob/main/spec/0.1/";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Statement,
    Setting,
    Option,
    Type,
    Function,
}

pub(crate) struct Documentation {
    pub(crate) name: &'static str,
    pub(crate) kind: Kind,
    pub(crate) summary: &'static str,
    /// A chapter filename and an existing section anchor in the Specification.
    pub(crate) sections: &'static [&'static str],
}

impl Documentation {
    pub(crate) fn markup(&self, markdown: bool) -> MarkupContent {
        let links = self
            .sections
            .iter()
            .map(|section| {
                let url = format!("{SPECIFICATION_URL}{section}");
                if markdown {
                    format!("[Specification]({url})")
                } else {
                    format!("Specification: {url}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        MarkupContent {
            kind: if markdown {
                MarkupKind::Markdown
            } else {
                MarkupKind::PlainText
            },
            value: if markdown {
                format!("`{}` — {}\n\n{links}", self.name, self.summary)
            } else {
                format!("{} — {}\n\n{links}", self.name, self.summary)
            },
        }
    }
}

macro_rules! docs {
    ($(($name:literal, $kind:ident, $summary:literal, $section:literal $(, $related:literal)*)),* $(,)?) => {
        pub(crate) const DOCUMENTATION: &[Documentation] = &[
            $(Documentation { name: $name, kind: Kind::$kind, summary: $summary, sections: &[$section $(, $related)*] }),*
        ];
    };
}

docs! {
    ("format", Setting, "Sets the format name as metadata; the final declaration wins.", "05-reader-specs.md#59-format-metadata"),
    ("comment", Setting, "Adds input line-comment prefixes, recognized after indentation during normal skipping.", "04-input.md#47-comment-and-stop-prefixes"),
    ("stop", Setting, "Adds prefixes that discard all remaining input during normal skipping.", "04-input.md#47-comment-and-stop-prefixes"),
    ("tokens", Setting, "Makes each listed byte a separate input token unless it is a separator.", "04-input.md#43-tokens-and-separators"),
    ("separators", Setting, "Adds the listed bytes to the input separators.", "04-input.md#43-tokens-and-separators"),
    ("ignore", Setting, "With trailing, suppresses the final check for unread input.", "04-input.md#48-end-of-input-and-trailing-input"),
    ("trailing", Option, "Suppresses the final unread-input check in ignore trailing.", "04-input.md#48-end-of-input-and-trailing-input"),
    ("type", Statement, "Defines a reading type for later statements in this block and nested blocks; range bounds are saved at definition.", "06-types-and-values.md#65-named-types-and-visibility"),
    ("int", Type, "Reads a signed 64-bit integer; produces value type int.", "06-types-and-values.md#62-scalar-reading-and-numeric-promotion"),
    ("float", Type, "Reads a finite binary64 number, including integer-looking tokens; produces value type float.", "06-types-and-values.md#62-scalar-reading-and-numeric-promotion"),
    ("num", Type, "Tries integer conversion before float conversion; stored arrays promote when a float is needed.", "06-types-and-values.md#62-scalar-reading-and-numeric-promotion"),
    ("word", Type, "Reads the next input token; produces value type string.", "06-types-and-values.md#62-scalar-reading-and-numeric-promotion"),
    ("rest", Type, "Reads the remaining line after initial skipping and trims trailing line blanks; scalar reads only.", "06-types-and-values.md#62-scalar-reading-and-numeric-promotion"),
    ("alphabet", Type, "Maps input bytes to int values in a depth-2 array; base defaults to 1.", "06-types-and-values.md#610-alphabets"),
    ("blank", Option, "Lists alphabet bytes that produce zero, after stripping and before alphabet mapping.", "06-types-and-values.md#610-alphabets"),
    ("strip", Option, "Lists bytes discarded before alphabet mapping; takes priority over blank.", "06-types-and-values.md#610-alphabets"),
    ("base", Option, "Sets the integer added to a symbol's first zero-based byte index in the alphabet.", "06-types-and-values.md#610-alphabets"),
    ("layout", Option, "Selects a matrix cell order by string; requires a depth-2 numeric declaration.", "06-types-and-values.md#68-matrix-layouts"),
    ("offset", Option, "Adds an integer scalar to stored numbers after range checks; sentinels are not adjusted.", "06-types-and-values.md#67-offsets-and-evaluation-order"),
    ("list", Statement, "Reads a depth-1 array until a consumed, unstored sentinel or until eol.", "06-types-and-values.md#69-sentinel-and-end-of-line-lists"),
    ("until", Option, "Introduces termination: a list sentinel or eol, repeat tokens or eof, or sections tokens.", "06-types-and-values.md#69-sentinel-and-end-of-line-lists", "07-structure.md#74-repeat-forms-and-termination", "07-structure.md#78-sections"),
    ("eol", Option, "Ends a list at the current data line's end; blank lines do not produce empty lists.", "06-types-and-values.md#69-sentinel-and-end-of-line-lists"),
    ("line", Statement, "Requires a line-pattern match and commits captures; in skip line, skips one data line.", "07-structure.md#73-line-find-skip-and-expect"),
    ("find", Statement, "Discards lines until a line pattern matches, then consumes that line and commits captures.", "07-structure.md#73-line-find-skip-and-expect"),
    ("skip", Statement, "Skips one line with skip line, or a count with skip expression lines; normal skipping applies first.", "07-structure.md#73-line-find-skip-and-expect"),
    ("lines", Option, "Completes skip expression lines; skipped blank and comment lines do not count separately.", "07-structure.md#73-line-find-skip-and-expect"),
    ("expect", Statement, "Consumes literal strings sequentially as exact ordinary input tokens.", "07-structure.md#73-line-find-skip-and-expect"),
    ("repeat", Statement, "Executes a body a fixed number of times, until a terminating token, or until eof; collects local values as columns.", "07-structure.md#74-repeat-forms-and-termination"),
    ("eof", Option, "Ends an indefinite repeat when no input remains after normal skipping.", "07-structure.md#74-repeat-forms-and-termination"),
    ("store", Option, "Stores a matched repeat or sections terminator as a string scalar in the enclosing frame.", "07-structure.md#74-repeat-forms-and-termination", "07-structure.md#78-sections"),
    ("as", Option, "Names a repeat's list of logical records without creating a data entry.", "07-structure.md#76-logical-records-and-names"),
    ("each", Statement, "Names the current logical record inside a repeat without reading input or creating an entry.", "07-structure.md#76-logical-records-and-names"),
    ("index", Statement, "Reads the innermost repeat's zero-based iteration number plus the given nonnegative base.", "07-structure.md#77-index-statements"),
    ("sections", Statement, "Handles matching lines in input order; each required arm must match exactly once.", "07-structure.md#78-sections"),
    ("optional", Option, "Allows a sections arm to match zero or one time.", "07-structure.md#78-sections"),
    ("last", Option, "Ends sections after the selected arm's body, without waiving other required arms.", "07-structure.md#78-sections"),
    ("other", Option, "With skip, discards lines that match no sections arm.", "07-structure.md#78-sections"),
    ("if", Statement, "Evaluates a condition once and executes the selected body, without introducing a value frame.", "07-structure.md#79-conditional-execution-and-let"),
    ("else", Option, "Executes its body when the preceding if condition is false.", "07-structure.md#79-conditional-execution-and-let"),
    ("let", Statement, "Evaluates an expression and defines or replaces its name in the current frame without reading input.", "07-structure.md#79-conditional-execution-and-let"),
    ("check", Statement, "Checks a condition where written; false reports a failure, with an optional message and record witness.", "08-expressions-and-checks.md#813-checks-and-failure-witnesses"),
    ("in", Option, "Introduces a comprehension domain; its binding is available only in the body after the colon.", "08-expressions-and-checks.md#810-comprehensions-and-bindings"),
    ("defined", Function, "Returns 1 when a named value or record list is available, otherwise 0; does not evaluate an unavailable name.", "08-expressions-and-checks.md#88-functions"),
    ("size", Function, "Returns the outer length of an array or the number of logical records in a record list.", "08-expressions-and-checks.md#88-functions"),
    ("count", Function, "Counts outer array elements or records; comprehension count counts true bodies.", "08-expressions-and-checks.md#88-functions", "08-expressions-and-checks.md#810-comprehensions-and-bindings"),
    ("min", Function, "Returns the minimum integer leaf across its arguments; fails when there are no leaves.", "08-expressions-and-checks.md#88-functions"),
    ("max", Function, "Returns the maximum integer leaf across its arguments; fails when there are no leaves.", "08-expressions-and-checks.md#88-functions"),
    ("sum", Function, "Sums integer leaves with checked addition; comprehension sum adds integer scalar bodies.", "08-expressions-and-checks.md#88-functions", "08-expressions-and-checks.md#810-comprehensions-and-bindings"),
    ("distinct", Function, "Returns 1 exactly when all integer leaves across its arguments differ.", "08-expressions-and-checks.md#88-functions"),
    ("all", Function, "Returns 1 when every comprehension body is true; stops at the first false body, and returns 1 for an empty domain.", "08-expressions-and-checks.md#810-comprehensions-and-bindings"),
    ("any", Function, "Returns 1 when some comprehension body is true; stops at the first true body, and returns 0 for an empty domain.", "08-expressions-and-checks.md#810-comprehensions-and-bindings"),
    ("param", Statement, "Introduces a caller-supplied value with an optional default; consumes no input.", "09-params-and-instances.md#91-caller-parameters"),
    ("instances", Statement, "Reads one instance per body pass in a fresh frame; retained local entries replace or extend prefix data.", "09-params-and-instances.md#93-instance-blocks-and-frames"),
}

pub(crate) fn documentation(name: &str) -> Option<&'static Documentation> {
    DOCUMENTATION.iter().find(|entry| entry.name == name)
}

pub(crate) const LAYOUTS: &[&str] = &[
    "FULL_MATRIX",
    "UPPER_ROW",
    "LOWER_ROW",
    "UPPER_DIAG_ROW",
    "LOWER_DIAG_ROW",
    "UPPER_COL",
    "LOWER_COL",
    "UPPER_DIAG_COL",
    "LOWER_DIAG_COL",
];
