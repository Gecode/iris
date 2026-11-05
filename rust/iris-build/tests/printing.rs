//! The printed text: every grammar production, the parenthesis and lookahead
//! tables, and specs far larger than a program would write by hand.
use iris_build::{
    D0, DynDepth, DynField, Field, Int, Maybe, Num, Pattern, SpecBuilder, Str, all, alphabet, any,
    bind, blank, count, count_over, defined, discard, discard_as, distinct, eol, float, gecode,
    int, lit, max, min, num, over, pattern, reference, rest, rest_of_line, size, sum, sum_over, ty,
    untyped, validate, word,
};
use iris_core::Params;
use iris_syntax::{FormatOptions, LanguageVersion, format};

/// Check that `text` is what `iris fmt` writes with no line limit.
fn assert_formatted(text: &str) {
    let options = FormatOptions {
        line_width: 1_000_000,
    };
    let formatted = format(
        text.as_bytes(),
        "builder.iris",
        LanguageVersion::V0_1,
        options,
    )
    .unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    assert_eq!(formatted, text);
}

/// Check the text of a spec that the language's static checks need not accept.
#[track_caller]
fn assert_text(build: impl FnOnce(&mut SpecBuilder), expected: &str) -> SpecBuilder {
    let mut b = SpecBuilder::unnamed();
    build(&mut b);
    let text = b.source().unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(text, expected);
    assert_formatted(&text);
    b
}

#[track_caller]
fn assert_prints(build: impl FnOnce(&mut SpecBuilder), expected: &str) {
    if let Err(error) = assert_text(build, expected).build() {
        panic!("{error}\n{expected}");
    }
}

/// `let v = expression`, with `x`, `y` and `z` defined first.
#[track_caller]
fn assert_expression<E: iris_build::Operand>(
    build: impl FnOnce(Field<Int>, Field<Int>, Field<Int>) -> E,
    expected: &str,
) {
    assert_prints(
        |b| {
            let x = b.read("x", int());
            let y = b.read("y", int());
            let z = b.read("z", int());
            b.let_("v", build(x, y, z));
        },
        &format!("x: int\ny: int\nz: int\nlet v = {expected}\n"),
    );
}

#[test]
fn settings() {
    let mut b = SpecBuilder::new("alpha-beta.1");
    b.format("two words");
    b.comment("#");
    b.stop(["a", "b"]);
    b.tokens(",;");
    b.separators(":");
    b.ignore_trailing();
    let text = b.source().unwrap();
    assert_eq!(
        text,
        "format alpha-beta.1\nformat \"two words\"\ncomment \"#\"\nstop \"a\" \"b\"\n\
         tokens \",;\"\nseparators \":\"\nignore trailing\n"
    );
    assert_formatted(&text);
    b.build().unwrap();
}

#[test]
fn format_names_are_bare_only_when_they_have_that_spelling() {
    for (name, expected) in [
        ("a", "format a\n"),
        ("a-b", "format a-b\n"),
        ("a.b-2", "format a.b-2\n"),
        ("a-1.2", "format \"a-1.2\"\n"),
        ("a-1e5", "format \"a-1e5\"\n"),
        ("1a", "format \"1a\"\n"),
        ("a-", "format \"a-\"\n"),
        ("", "format \"\"\n"),
        ("a b", "format \"a b\"\n"),
    ] {
        let b = SpecBuilder::new(name);
        let text = b.source().unwrap();
        assert_eq!(text, expected);
        assert_formatted(&text);
        b.build().unwrap();
    }
}

#[test]
fn declarations() {
    assert_prints(
        |b| {
            let n = b.read("n", int());
            b.read("a", float());
            b.read("b", num());
            b.read("c", word());
            b.read("d", rest());
            b.read("e", int().dim(2).dim(n));
            b.read("f", int().dim(2).dim(2).layout(1).offset(n));
            b.read(
                "g",
                alphabet("ab").blank(" ").strip("\t").base(-1).dim(2).dim(2),
            );
            b.read("h", alphabet("ab").dim(2).dim(n));
            b.read(discard(), int());
        },
        "n: int\na: float\nb: num\nc: word\nd: rest\ne[2][n]: int\n\
         f[2][2]: int layout 1 offset n\ng[2][2]: alphabet \"ab\" blank \" \" strip \"\\t\" base -1\n\
         h[2][n]: alphabet \"ab\"\n_: int\n",
    );
}

#[test]
fn ranges() {
    assert_prints(
        |b| {
            let n = b.read("n", int());
            b.read("a", 0..10);
            b.read("b", 0..=10);
            b.read("c", lit(0)..);
            b.read("d", ..lit(10));
            b.read("e", ..=lit(10));
            b.read("f", lit(0)..n.ex());
            b.read("g", ty(0..10).dim(n));
            b.read("h", (n + 1).dynamic()..);
        },
        "n: int\na: 0..<10\nb: 0..10\nc: 0..\nd: ..<10\ne: ..10\nf: 0..<n\ng[n]: 0..<10\nh: n + 1..\n",
    );
}

#[test]
fn lists_types_and_params() {
    assert_prints(
        |b| {
            b.list("a", int().until(0));
            b.list("b", word().until("end"));
            b.list("c", int().until(eol()).offset(1));
            let t = b.define_type("t", lit(0)..);
            b.read("x", t);
            let u = b.define_type("u", t);
            b.list("d", u.into_until());
            b.param("p");
            b.param_default("q", 3);
        },
        "list a: int until 0\nlist b: word until \"end\"\nlist c: int until eol offset 1\n\
         type t = 0..\nx: t\ntype u = t\nlist d: u until 1\nparam p\nparam q = 3\n",
    );
}

trait IntoUntil {
    fn into_until(self) -> iris_build::ListReading<Int>;
}
impl IntoUntil for iris_build::NamedType<Int> {
    fn into_until(self) -> iris_build::ListReading<Int> {
        ty(self).until(1)
    }
}

#[test]
fn line_statements() {
    assert_prints(
        |b| {
            let n = b.read("n", int());
            b.line("plain");
            b.find("a \"quoted\" \\ line\n");
            b.skip_line();
            b.skip_lines(n);
            b.index(n + 1);
            b.expect("x");
            b.expect(["y", "z"]);
        },
        "n: int\nline \"plain\"\nfind \"a \\\"quoted\\\" \\\\ line\\n\"\nskip line\nskip n lines\n\
         index n + 1\nexpect \"x\"\nexpect \"y\" \"z\"\n",
    );
}

#[test]
fn patterns() {
    assert_prints(
        |b| {
            let x = b.capture("x", int());
            let y = b.capture("y", untyped());
            let z = b.capture("z", lit(1)..);
            let w: Field<Str> = b.forward("w");
            b.line(pattern!("a {x} b {y}"));
            b.line(pattern!("{z}"));
            b.line(pattern!("{part}", part = reference(x)));
            b.line(pattern!("{part}", part = validate(float())));
            b.line(pattern!("{part} {p}", part = blank(), p = discard()));
            b.line(pattern!("{part}", part = discard_as(word())));
            b.line(pattern!("{part}", part = bind(w, word())));
            b.line(pattern!("tail {part}", part = rest_of_line()));
            b.line(Pattern::new().text("a").part(rest_of_line()).text("  "));
            b.line(pattern!("lone } brace"));
        },
        "line \"a {x:int} b {y}\"\nline \"{z:1..}\"\nline \"{$x}\"\nline \"{:float}\"\n\
         line \"{} {_}\"\nline \"{_:word}\"\nline \"{w:word}\"\nline \"tail ...\"\n\
         line \"a...  \"\nline \"lone } brace\"\n",
    );
}

#[test]
fn repeats() {
    assert_prints(
        |b| {
            let n = b.read("n", int());
            b.repeat(n, |_| {});
            b.repeat(n, |r| {
                r.read("a", int());
            });
            b.repeat_until_eof().body(|r| {
                r.read("b", int());
            });
            b.repeat_until_eof();
            b.repeat_until("end");
            let stored: Field<Str> = b.forward("stored");
            b.repeat_until(["x", "y"]).store(stored).body(|r| {
                r.read("c", int());
            });
            let underscore: Field<Str> = b.forward("_");
            b.repeat_until("z").store(underscore);
        },
        "n: int\nrepeat n {}\nrepeat n {\n  a: int\n}\nrepeat until eof {\n  b: int\n}\n\
         repeat until eof {}\nrepeat until \"end\" {}\nrepeat until \"x\" \"y\" store stored {\n  \
         c: int\n}\nrepeat until \"z\" store _ {}\n",
    );
}

#[test]
fn user_names_for_records_always_print() {
    assert_prints(
        |b| {
            b.repeat(2, |r| {
                r.each_name("job");
                r.as_name("jobs");
            });
        },
        "repeat 2 as jobs {\n  each job\n}\n",
    );
}

#[test]
fn a_forward_record_serves_use_before_the_repeat() {
    assert_prints(
        |b| {
            let job = b.forward_record("job", "jobs");
            let x: Field<Int, iris_build::D1> = b.forward("x");
            b.let_("n", size(job));
            b.let_("total", sum_over(job, |j| j.get(x)));
            b.repeat(2, |r| {
                r.record_as(job);
                r.read(x, int());
            });
        },
        "let n = size(jobs)\nlet total = sum(element_1 in jobs: element_1.x)\n\
         repeat 2 as jobs {\n  each job\n  x: int\n}\n",
    );
}

#[test]
fn sections() {
    assert_prints(
        |b| {
            let stored: Field<Str, D0, Maybe> = b.forward("stored");
            let a = b.optional_capture("a", int());
            b.sections().body(|_| {});
            b.sections()
                .until("END")
                .store(stored)
                .skip_other()
                .until(["x", "y"])
                .body(|s| {
                    s.arm("plain");
                    s.arm(pattern!("a {a}")).optional();
                    s.arm("b").last();
                    s.arm("c").optional().last();
                    s.arm("d").body(|_| {});
                    s.arm("e").optional().last().body(|e| {
                        e.read("x", int());
                    });
                });
        },
        "sections {}\nsections until \"END\" store stored skip other until \"x\" \"y\" {\n  \
         \"plain\"\n  \"a {a:int}\" optional\n  \"b\" last\n  \"c\" optional last\n  \"d\" {}\n  \
         \"e\" optional last {\n    x: int\n  }\n}\n",
    );
}

#[test]
fn branches() {
    assert_prints(
        |b| {
            let n = b.read("n", int());
            b.if_(n, |_| {});
            b.if_(n, |t| {
                t.read("a", int());
            })
            .finish();
            b.if_(n, |t| {
                t.read("b", int());
            })
            .else_(|e| {
                e.read("c", int());
            });
            b.if_(n.gt(1), |_| {})
                .else_if(n.gt(0), |_| {})
                .else_(|_| {});
            let branch: DynField = b.forward("if");
            b.if_(n, |_| {}).else_read_if(branch, int());
            b.if_(n, |t| {
                t.read("d", int());
            })
            .else_if(n.lt(0), |t| {
                t.read("e", int());
            })
            .finish();
        },
        "n: int\nif n {}\nif n {\n  a: int\n}\nif n {\n  b: int\n} else {\n  c: int\n}\n\
         if n > 1 {} else if n > 0 {} else {}\nif n {} else if: int\n\
         if n {\n  d: int\n} else if n < 0 {\n  e: int\n}\n",
    );
}

#[test]
fn let_and_check() {
    assert_prints(
        |b| {
            let x = b.let_("x", lit(1));
            b.set(x, lit(2));
            b.check(x);
            b.check(x.gt(0)).message("a \"quoted\" message");
        },
        "let x = 1\nlet x = 2\ncheck x\ncheck x > 0 \"a \\\"quoted\\\" message\"\n",
    );
}

#[test]
fn instances_and_a_later_format() {
    let mut b = SpecBuilder::new("one");
    b.instances(|i| {
        i.read("x", int());
    });
    b.format("two");
    let text = b.source().unwrap();
    assert_eq!(text, "format one\ninstances {\n  x: int\n}\nformat two\n");
    assert_formatted(&text);
    b.build().unwrap();
}

#[test]
fn literals() {
    assert_expression(|_, _, _| lit(1), "1");
    assert_expression(|_, _, _| lit(-1), "-1");
    assert_expression(|_, _, _| lit(1.0), "1.0");
    assert_expression(|_, _, _| lit(-1.5), "-1.5");
    assert_expression(|_, _, _| lit(1e300), "1e300");
    assert_expression(|_, _, _| lit(9007199254740992.0), "9007199254740992.0");
    assert_expression(|_, _, _| lit("a\tb"), "\"a\\tb\"");
    assert_expression(|_, _, _| lit(String::from("s")), "\"s\"");
    assert_expression(|_, _, _| lit(4294967295u32), "4294967295");
    assert_expression(|_, _, _| lit(i64::MIN), "(-9223372036854775807 - 1)");
    assert_expression(|_, _, _| lit(i64::MAX), "9223372036854775807");
    assert_expression(|_, _, _| gecode::INT_MAX + 0, "gecode.int_max + 0");
    assert_expression(|_, _, _| -gecode::INT_MIN, "-gecode.int_min");
}

#[test]
fn parentheses_follow_the_precedence_levels() {
    assert_expression(|x, y, z| x + y + z, "x + y + z");
    assert_expression(|x, y, z| x + (y + z), "x + (y + z)");
    assert_expression(|x, y, z| x - (y - z), "x - (y - z)");
    assert_expression(|x, y, z| x * y + z, "x * y + z");
    assert_expression(|x, y, z| x * (y + z), "x * (y + z)");
    assert_expression(|x, y, z| (x + y) / z, "(x + y) / z");
    assert_expression(|x, y, z| x % (y * z), "x % (y * z)");
    assert_expression(|x, y, z| x.lt(y.lt(z)), "x < (y < z)");
    assert_expression(|x, y, z| x.lt(y).lt(z), "x < y < z");
    assert_expression(|x, y, z| x.eq(y).ne(z), "x == y != z");
    assert_expression(|x, y, z| x.eq(y.ne(z)), "x == (y != z)");
    assert_expression(|x, y, z| x.le(y).ge(z).gt(1), "x <= y >= z > 1");
    assert_expression(|x, y, z| x.lt(y).eq(z), "x < y == z");
    assert_expression(|x, y, z| x & y | z, "x && y || z");
    assert_expression(|x, y, z| x & (y | z), "x && (y || z)");
    assert_expression(|x, y, z| (x & y) & z, "x && y && z");
    assert_expression(|x, y, z| x & (y & z), "x && (y && z)");
    assert_expression(|x, y, _| !(x & y), "!(x && y)");
    assert_expression(|x, y, _| !x & y, "!x && y");
    assert_expression(|x, y, _| -(x + y), "-(x + y)");
    assert_expression(|x, _, _| -(-x), "--x");
    assert_expression(|x, _, _| !(!x), "!!x");
    assert_expression(|x, _, _| -(!x), "-!x");
    assert_expression(|x, y, _| x - (-y), "x - -y");
}

#[test]
fn powers_bind_tighter_than_unary_on_the_left_only() {
    assert_expression(|_, _, _| -(lit(2).pow(2)), "-2 ^ 2");
    assert_expression(|_, _, _| (-lit(2)).pow(2), "(-2) ^ 2");
    assert_expression(|_, _, _| lit(-2).pow(2), "(-2) ^ 2");
    assert_expression(|_, _, _| lit(2).pow(lit(3).pow(4)), "2 ^ 3 ^ 4");
    assert_expression(|_, _, _| lit(2).pow(3).pow(4), "(2 ^ 3) ^ 4");
    assert_expression(|x, _, _| lit(2).pow(-x), "2 ^ -x");
    assert_expression(|x, y, _| (x + y).pow(2), "(x + y) ^ 2");
    assert_expression(|x, y, _| lit(2).pow(x + y), "2 ^ (x + y)");
    assert_expression(|x, y, z| x * y.pow(z), "x * y ^ z");
}

#[test]
fn operators_take_literals_on_either_side() {
    assert_expression(|x, _, _| 1 + x, "1 + x");
    assert_expression(|x, _, _| 2i64 * (x - 1), "2 * (x - 1)");
    assert_expression(|x, _, _| 7u32 / x, "7 / x");
    assert_expression(|x, _, _| 7 % x.ex(), "7 % x");
    assert_expression(|x, _, _| 1 + gecode::INT_MAX - x, "1 + gecode.int_max - x");
}

#[test]
fn calls_and_comprehensions() {
    assert_prints(
        |b| {
            let a = b.read("a", int().dim(2));
            b.let_("v1", size(a));
            b.let_("v2", count(a));
            b.let_("v3", min(a));
            b.let_("v4", max((a, 1)));
            b.let_("v5", sum([lit(1), lit(2), lit(3)]));
            b.let_("v6", distinct(vec![lit(1), lit(2)]));
            b.let_("v7", defined(a));
            b.let_("v8", all(a, |x| x.gt(0)));
            b.let_("v9", any(a, |x| x.gt(0)));
            b.let_("v10", sum_over(a, |x| x));
            b.let_("v11", count_over(over(a).named("item"), |x| x.gt(1)));
            b.let_("v12", sum_over(a, |x| sum_over(a, |y| x.dynamic() * y)));
            b.let_("v13", a.at(0) + a.at(1).ex());
        },
        "a[2]: int\nlet v1 = size(a)\nlet v2 = count(a)\nlet v3 = min(a)\nlet v4 = max(a, 1)\n\
         let v5 = sum(1, 2, 3)\nlet v6 = distinct(1, 2)\nlet v7 = defined(a)\n\
         let v8 = all(element_1 in a: element_1 > 0)\nlet v9 = any(element_2 in a: element_2 > 0)\n\
         let v10 = sum(element_3 in a: element_3)\nlet v11 = count(item in a: item > 1)\n\
         let v12 = sum(element_4 in a: sum(element_5 in a: element_4 * element_5))\n\
         let v13 = a[0] + a[1]\n",
    );
}

#[test]
fn internal_names_skip_every_name_in_the_spec() {
    assert_prints(
        |b| {
            b.read("record_1", int());
            b.read("records_1", int());
            let a = b.read("element_1", int().dim(1));
            let (job, x) = b.repeat(1, |r| (r.record(), r.read("x", int())));
            b.let_("v", sum_over(job, |j| j.get(x)));
            b.check(job.get(x));
            b.let_("u", all(a, |e| e));
        },
        "record_1: int\nrecords_1: int\nelement_1[1]: int\nrepeat 1 as records_2 {\n  \
         each record_2\n  x: int\n}\nlet v = sum(element_2 in records_2: element_2.x)\n\
         check record_2.x\nlet u = all(element_3 in element_1: element_3)\n",
    );
}

#[test]
fn records_project_and_index() {
    assert_prints(
        |b| {
            let (outer, inner, x) = b.repeat(2, |r| {
                let (inner, x) = r.repeat(2, |s| (s.record(), s.read("x", int())));
                (r.record(), inner, x)
            });
            b.let_("a", outer.at(0).get(x).at(1));
            b.check(size(outer.records(inner)));
            b.let_("c", outer.at(1).records(inner).at(0).get(x));
            b.let_("d", size(outer.list()));
            b.check(defined(outer));
            b.let_(
                "f",
                sum_over(outer, |o| sum_over(o.records(inner), |i| i.get(x))),
            );
        },
        "repeat 2 as records_1 {\n  each record_1\n  repeat 2 as records_2 {\n    x: int\n  }\n}\nlet a = records_1[0].x[1]\ncheck size(record_1.records_2)\n\
         let c = records_1[1].records_2[0].x\nlet d = size(records_1)\ncheck defined(records_1)\n\
         let f = sum(element_1 in records_1: sum(element_2 in element_1.records_2: element_2.x))\n",
    );
}

#[test]
fn a_member_never_glues_to_a_literal_or_a_name_gecode() {
    assert_text(
        |b| {
            let x = b.read("x", int());
            let g: DynField = b.undefined("gecode");
            b.let_("a", lit(1).dynamic().get(x));
            b.let_("b", g.at(0).get(x));
            b.let_("c", g.dynamic().get(x));
            b.let_("d", g.dynamic().get(g).get(x));
            b.let_("e", (lit(1).dynamic() + 2).dynamic().get(x));
        },
        "x: int\nlet a = (1).x\nlet b = gecode[0].x\nlet c = gecode . x\nlet d = gecode . gecode . x\n\
         let e = (1 + 2).x\n",
    );
}

#[test]
fn keywords_at_the_start_of_an_expression_take_parentheses() {
    let names = ["until", "line", "eol", "int", "layout", "alphabet", "t"];
    assert_prints(
        |b| {
            let [until, line, eol_name, int_name, layout, alpha, t] =
                names.map(|name| b.read(name, int()));
            b.repeat(until, |_| {});
            b.repeat(until + 1, |_| {});
            b.skip_lines(line);
            b.skip_lines(line * 2);
            b.list("a", int().until(eol_name));
            b.list("b", int().until(eol_name + 1));
            b.read("c", lit(0)..=layout.ex());
            b.read("d", lit(0)..layout.ex());
            b.read("e", ..=(layout + 1));
            b.read("f", int_name.ex()..=t.ex());
            b.read("g", (int_name + 1)..);
            b.read("h", alpha.ex()..);
            b.read("j", t.ex()..);
            b.line(pattern!("{x}", x = validate(lit(0)..=layout.ex())));
        },
        "until: int\nline: int\neol: int\nint: int\nlayout: int\nalphabet: int\nt: int\n\
         repeat (until) {}\nrepeat (until + 1) {}\nskip (line) lines\n\
         skip (line * 2) lines\nlist a: int until (eol)\nlist b: int until (eol + 1)\n\
         c: 0..(layout)\nd: 0..<(layout)\ne: ..(layout + 1)\nf: (int)..t\ng: (int + 1)..\n\
         h: (alphabet)..\nj: t..\nline \"{:0..(layout)}\"\n",
    );
}

#[test]
fn a_named_type_takes_parentheses_as_a_lower_bound() {
    assert_text(
        |b| {
            let named: DynField = b.undefined("named");
            b.define_type("named", int());
            b.read("x", named.dynamic()..);
        },
        "type named = int\nx: (named)..\n",
    );
}

#[test]
fn every_stop_word_is_guarded_as_an_upper_bound() {
    for word in [
        "comment",
        "stop",
        "expect",
        "tokens",
        "separators",
        "ignore",
        "param",
        "list",
        "line",
        "find",
        "skip",
        "index",
        "repeat",
        "each",
        "instances",
        "sections",
        "if",
        "let",
        "check",
        "type",
        "layout",
        "offset",
        "until",
    ] {
        let mut b = SpecBuilder::unnamed();
        let bound = b.let_(word, lit(5));
        b.read("x", lit(0)..=bound.ex());
        let text = b.source().unwrap();
        assert!(text.ends_with(&format!("x: 0..({word})\n")), "{text}");
        assert_formatted(&text);
        let data = b.build().unwrap().read(b"3", "input", &Params::new());
        assert_eq!(data.unwrap().int("x").unwrap(), 3);
    }
}

#[test]
fn numbers_round_trip() {
    let mut b = SpecBuilder::unnamed();
    let values = [0.1, 1e-7, 1.7976931348623157e308, 123456789012345680.0];
    let mut fields = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        fields.push(b.let_(format!("v{index}"), lit(value)));
    }
    let minimum = b.let_("minimum", lit(i64::MIN));
    let spec = b.build().unwrap();
    let data = spec.read(b"", "input", &Params::new()).unwrap();
    for (field, value) in fields.into_iter().zip(values) {
        assert_eq!(data.get(field).to_bits(), value.to_bits());
    }
    assert_eq!(data.get(minimum), i64::MIN);
}

#[test]
fn a_million_term_chain_prints_and_drops() {
    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    let mut chain = x + 1;
    for _ in 0..1_000_000 {
        chain = chain + x;
    }
    let dropped = chain.clone();
    drop(dropped);
    b.let_("v", chain);
    let text = b.source().unwrap();
    assert_eq!(text.matches('+').count(), 1_000_001);
}

#[test]
fn a_deep_right_chain_prints_without_recursion() {
    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    let mut chain = -x;
    for _ in 0..20_000 {
        chain = -chain;
    }
    b.let_("v", chain);
    let text = b.source().unwrap();
    assert_eq!(text.matches('-').count(), 20_001);
}

#[test]
fn three_hundred_dimensions_read_through_a_value_ref() {
    let mut b = SpecBuilder::unnamed();
    let deep = b.read("deep", int().dims(std::iter::repeat_n(1, 300)));
    let spec = b.build().unwrap();
    let data = spec.read(b"7", "input", &Params::new()).unwrap();
    let mut value = data.get(deep);
    for _ in 0..299 {
        value = value.row(0);
    }
    assert_eq!(value.as_ints().unwrap(), &[7]);
}

#[test]
fn ten_nested_repeats_stay_dynamic_past_the_static_depths() {
    let mut b = SpecBuilder::unnamed();
    let leaf = b.repeat(1, |s| {
        s.repeat(1, |s| {
            s.repeat(1, |s| {
                s.repeat(1, |s| {
                    s.repeat(1, |s| {
                        s.repeat(1, |s| {
                            s.repeat(1, |s| {
                                s.repeat(1, |s| {
                                    s.repeat(1, |s| s.repeat(1, |s| s.read("leaf", int())))
                                })
                            })
                        })
                    })
                })
            })
        })
    });
    let leaf: Field<Int, DynDepth> = leaf;
    let spec = b.build().unwrap();
    let data = spec.read(b"9", "input", &Params::new()).unwrap();
    let mut value = data.get(leaf);
    for _ in 0..9 {
        value = value.row(0);
    }
    assert_eq!(value.as_ints().unwrap(), &[9]);
}

#[test]
fn num_entries_read_as_numbers() {
    let mut b = SpecBuilder::unnamed();
    let x: Field<Num> = b.read("x", num());
    let spec = b.build().unwrap();
    let data = spec.read(b"2.5", "input", &Params::new()).unwrap();
    assert_eq!(data.get(x), iris_build::Number::Float(2.5));
}
