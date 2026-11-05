//! Layout forms for the width sweeps of the Rust backend.
//!
//! Each form is a reader spec with holes for what the sweeps vary: `@n` is a
//! name, `@t` is the text of a string, and `@s` is an alphabet as long as `@t`.

/// Every form that the Rust backend lays out, with a title for failures.
pub const FORMS: &[(&str, &str)] = &[
    ("let", "let @n = 1\n"),
    ("let-string", "let x = \"@t\"\n"),
    ("let-both", "let @n = \"@t\"\n"),
    ("param", "param @n\n"),
    ("param-default", "param @n = \"@t\"\n"),
    ("read", "@n: int\n"),
    ("read-dims", "@n: int\nx[@n][@n]: int\n"),
    ("read-range", "@n: int\nx: 1..@n\n"),
    ("read-range-both", "@n: int\nx: @n..<@n\n"),
    ("read-alphabet", "x[2][2]: alphabet \"@s\" strip \"@t\"\n"),
    ("read-offset", "@n: int\nx[2]: int offset @n\n"),
    ("list-until", "@n: int\nlist x: int until @n\n"),
    ("undefined", "check !defined(@n)\n"),
    ("format", "format \"@t\"\nskip line\n"),
    ("comment", "comment \"@t\" \"@t\"\n"),
    ("comment-one", "comment \"@t\"\n"),
    ("expect", "expect \"@t\" \"a\"\n"),
    ("tokens", "tokens \"@t\"\n"),
    ("type", "type @n = 1..9\nx: @n\n"),
    ("type-unused", "type @n = 1..9\n"),
    ("type-alphabet", "type @n = alphabet \"@s\"\nx[2][2]: @n\n"),
    ("call", "@n[2]: int\nlet y = sum(@n)\n"),
    ("call-many", "@n: int\nlet y = max(@n, @n, 1)\n"),
    ("call-defined", "@n: int\nlet y = defined(@n) + 1\n"),
    ("call-string", "let y = size(\"@t\")\n"),
    ("index", "@n[2]: int\nlet y = @n[@n[0]]\n"),
    ("power", "@n: int\nlet y = @n ^ @n\n"),
    ("field", "repeat 1 as rs {\n@n: int\n}\nlet y = rs[0].@n\n"),
    ("record", "repeat 1 as @n {\nx: int\n}\nlet y = size(@n)\n"),
    (
        "record-each",
        "repeat 1 as r {\neach @n\nx: int\ncheck @n.x > 0\n}\n",
    ),
    ("check", "@n: int\ncheck @n > 0\n"),
    ("check-name", "@n: int\ncheck @n \"@t\"\n"),
    ("check-bare", "@n: int\ncheck @n\n"),
    ("format-only", "format \"@t\"\n"),
    ("check-message", "@n: int\ncheck @n > 0 \"@t\"\n"),
    ("check-short-message", "@n: int\ncheck @n > 0 \"m\"\n"),
    ("check-long-message", "x: int\ncheck x > 0 \"@t\"\n"),
    ("binary", "@n: int\nlet y = @n + @n * 2\n"),
    ("binary-wide", "@n: int\nlet y = @n + @n - @n * @n / 2\n"),
    ("comparison", "@n: int\nlet y = @n == \"@t\"\n"),
    ("unary", "@n: int\nlet y = -@n + !@n\n"),
    ("repeat", "@n: int\nrepeat @n {\nx: int\n}\n"),
    ("repeat-empty", "@n: int\nrepeat @n {\n}\n"),
    ("repeat-sum", "@n: int\nrepeat @n + @n {\nx: int\n}\n"),
    ("repeat-names", "repeat 2 as @n {\neach @ne\nx: int\n}\n"),
    ("repeat-until", "repeat until \"@t\" {\nx: int\n}\n"),
    ("repeat-until-empty", "repeat until \"@t\" {\n}\n"),
    (
        "repeat-until-store",
        "repeat until \"@t\" store @n {\nx: int\n}\n",
    ),
    (
        "repeat-until-name",
        "repeat until \"e\" store @n {\nx: int\n}\n",
    ),
    (
        "sections",
        "sections until \"@t\" {\n\"a\" {\nx: int\n}\n}\n",
    ),
    (
        "sections-store",
        "sections until \"@t\" store @n skip other {\n\"a\"\n}\n",
    ),
    ("arm", "sections {\n\"@t\" optional last\n}\n"),
    ("arm-plain", "sections {\n\"@t\"\n}\n"),
    ("arm-block", "sections {\n\"@t\" optional {\nx: int\n}\n}\n"),
    ("arm-capture", "sections {\n\"{@n:int}\" {\nx: int\n}\n}\n"),
    (
        "arm-capture-last",
        "sections {\n\"a{@n:int}\" optional last\n}\n",
    ),
    ("pattern-text", "line \"@t\"\n"),
    ("pattern-capture", "line \"{@n:int}\"\n"),
    ("pattern-untyped", "line \"{@n}\"\n"),
    ("pattern-two", "line \"a {@n:int} b\"\n"),
    ("pattern-captures", "line \"{@n:int} {@nb:float} {@nc}\"\n"),
    ("pattern-range", "@n: int\nline \"{x:1..@n}\"\n"),
    ("pattern-validate", "@n: int\nline \"{:1..@n} {_:1..@n}\"\n"),
    ("pattern-text-capture", "line \"@t{@n:int}\"\n"),
    ("pattern-reference", "@n: int\nline \"{$@n} x\"\n"),
    ("pattern-find", "find \"a {@n:int}\"\n"),
    ("if", "@n: int\nif @n > 0 {\nx: int\n}\n"),
    ("if-empty", "@n: int\nif @n > 0 {\n}\n"),
    (
        "if-else",
        "@n: int\nif @n > 0 {\nx: int\n} else {\ny: int\n}\n",
    ),
    (
        "if-empty-else",
        "@n: int\nif @n > 0 {\n} else {\ny: int\n}\n",
    ),
    ("if-empty-else-empty", "@n: int\nif @n > 0 {\n} else {\n}\n"),
    (
        "else-if",
        "@n: int\nif @n > 0 {\nx: int\n} else if @n < 0 {\ny: int\n}\n",
    ),
    (
        "else-if-empty",
        "@n: int\nif @n > 0 {\n} else if @n < 0 {\n} else {\n}\n",
    ),
    (
        "else-read",
        "@n: int\nif @n > 0 {\nx: int\n} else if: 1..@n\n",
    ),
    ("else-read-empty", "@n: int\nif @n > 0 {\n} else if: int\n"),
    ("if-string", "x: int\nif x == \"@t\" {\ny: int\n}\n"),
    (
        "comprehension-body-name",
        "@n: int\nxs[2]: int\nlet y = sum(v in xs: @n)\n",
    ),
    (
        "comprehension-name",
        "xs[2]: int\nlet y = sum(@n in xs: @n)\n",
    ),
    (
        "comprehension-unused",
        "xs[2]: int\nlet y = sum(@n in xs: 1)\n",
    ),
    (
        "comprehension-domain",
        "@n[2]: int\nlet y = all(v in @n: v > 0)\n",
    ),
    (
        "comprehension-body",
        "xs[2]: int\nlet y = sum(v in xs: \"@t\")\n",
    ),
    (
        "comprehension-all",
        "xs[2]: int\nlet y = any(@n in xs: @n == \"@t\")\n",
    ),
    (
        "comprehension-binary",
        "@n[2]: int\nlet y = count(v in @n: v + @n[0] > @n[1])\n",
    ),
    (
        "comprehension-nested",
        "xs[2]: int\nlet y = sum(@n in xs: sum(w in xs: @n + w))\n",
    ),
    (
        "comprehension-nested-domain",
        "@n[2]: int\nxs[2]: int\nlet y = sum(v in xs: sum(w in @n: w))\n",
    ),
    (
        "comprehension-nested-string",
        "xs[2]: int\nlet y = sum(v in xs: sum(w in xs: \"@t\"))\n",
    ),
    (
        "comprehension-block",
        "@n: int\nxs[2]: int\nlet y = sum(v in xs: @n + @n + @n + @n)\n",
    ),
    (
        "comprehension-tail",
        "@n: int\nxs[2]: int\nlet y = sum(v in xs: v + @n + @n)\n",
    ),
    (
        "comprehension-records",
        "repeat 1 as @n {\nx: int\n}\nlet y = sum(v in @n: v.x)\n",
    ),
    (
        "comprehension-in-check",
        "@n[2]: int\ncheck all(v in @n: v > 0) \"@t\"\n",
    ),
    (
        "comprehension-in-if",
        "@n[2]: int\nif all(v in @n: v > 0) {\nx: int\n}\n",
    ),
    (
        "comprehension-in-repeat",
        "@n[2]: int\nrepeat sum(v in @n: v) {\nx: int\n}\n",
    ),
];

/// Whether a form can stand in a block, which a setting cannot.
pub fn nests(form: &str) -> bool {
    !["format ", "comment ", "tokens "]
        .iter()
        .any(|setting| form.starts_with(setting))
}

/// The spec of a form, inside `depth` nested repeats.
pub fn spec(form: &str, name: &str, text: &str, depth: usize) -> String {
    let symbols: String = ('#'..='~')
        .filter(|symbol| !matches!(symbol, '\\' | 't'))
        .take(text.chars().count())
        .collect();
    let body = form
        .replace("@n", name)
        .replace("@t", text)
        .replace("@s", &symbols);
    let (open, close) = ("repeat 1 {\n".repeat(depth), "}\n".repeat(depth));
    format!("{open}{body}{close}")
}
