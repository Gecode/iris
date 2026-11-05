//! The builder's own checks, each reported by `build()`.
use gecode_iris_build::{
    BuildError, BuildErrorKind, D0, D1, DynField, Field, Float, Int, Maybe, Num, Record,
    SpecBuilder, Str, all, alphabet, bind, defined, discard, float, int, lit, over, pattern,
    reference, size, word,
};
use gecode_iris_core::Params;

fn failure_of(builder: SpecBuilder) -> BuildError {
    builder.build().expect_err("the builder should fail")
}

macro_rules! assert_kind {
    ($error:expr, $kind:pat) => {{
        let error = $error;
        assert!(matches!(error.kind(), $kind), "{error}");
        error
    }};
}

#[test]
fn a_statement_through_an_outer_scope_is_rejected() {
    let mut b = SpecBuilder::unnamed();
    b.instances(|s| {
        let n = s.capture("n", int());
        s.sections().body(|arms| {
            let _ = &arms;
            let arm = arms.arm(pattern!("{n}"));
            arm.body(|_| {
                s.read("x", int());
            });
        });
    });
    assert_kind!(failure_of(b), BuildErrorKind::Scope);
}

#[test]
fn an_arm_through_an_outer_block_is_rejected() {
    let mut b = SpecBuilder::unnamed();
    b.sections().body(|arms| {
        arms.arm("a").body(|_| {
            arms.arm("b");
        });
    });
    assert_kind!(failure_of(b), BuildErrorKind::Scope);
}

#[test]
fn a_guard_after_later_statements_is_rejected() {
    let mut b = SpecBuilder::unnamed();
    let n = b.read("n", int());
    let branch = b.if_(n, |_| {});
    b.read("y", int());
    branch.else_(|_| {});
    assert_kind!(failure_of(b), BuildErrorKind::Scope);

    let mut b = SpecBuilder::unnamed();
    let repeat = b.repeat_until("end");
    b.read("y", int());
    repeat.body(|_| {});
    assert_kind!(failure_of(b), BuildErrorKind::Scope);
}

#[test]
fn a_handle_from_another_builder_is_rejected() {
    let mut other = SpecBuilder::unnamed();
    let foreign = other.read("x", int());

    let mut b = SpecBuilder::unnamed();
    b.let_("y", foreign + 1);
    assert_kind!(failure_of(b), BuildErrorKind::Provenance);

    let mut b = SpecBuilder::unnamed();
    b.read(foreign, int());
    assert_kind!(failure_of(b), BuildErrorKind::Provenance);

    let mut b = SpecBuilder::unnamed();
    b.line(pattern!("{foreign}"));
    assert_kind!(failure_of(b), BuildErrorKind::Provenance);

    let record: Record = other.repeat(1, |r| r.record());
    let mut b = SpecBuilder::unnamed();
    b.check(record.list().at(0).get(foreign));
    assert_kind!(failure_of(b), BuildErrorKind::Provenance);

    let named = other.define_type("t", int());
    let mut b = SpecBuilder::unnamed();
    b.read("x", named);
    assert_kind!(failure_of(b), BuildErrorKind::Provenance);

    let mut b = SpecBuilder::unnamed();
    let x = b.capture("x", named);
    b.line(pattern!("{x}"));
    assert_kind!(failure_of(b), BuildErrorKind::Provenance);
}

#[test]
fn a_declared_handle_reads_but_does_not_build() {
    const X: Field<Int> = Field::declared("x");
    let mut b = SpecBuilder::unnamed();
    b.let_("y", X);
    assert_kind!(failure_of(b), BuildErrorKind::Provenance);
}

#[test]
fn defining_a_name_twice_needs_the_handle() {
    let mut b = SpecBuilder::unnamed();
    b.read("x", int());
    b.let_("x", lit(1));
    let error = assert_kind!(failure_of(b), BuildErrorKind::Redefinition);
    assert!(error.to_string().contains("`x`"));

    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    b.set(x, lit(2));
    b.let_(x, lit(1.5));
    assert!(b.build().is_ok());
}

#[test]
fn values_with_no_spelling_are_rejected() {
    let unprintable = |build: fn(&mut SpecBuilder)| {
        let mut b = SpecBuilder::unnamed();
        build(&mut b);
        assert_kind!(failure_of(b), BuildErrorKind::Unprintable);
    };
    unprintable(|b| b.line("a { b"));
    unprintable(|b| b.line("a ... b"));
    unprintable(|b| {
        b.read("format", int());
    });
    unprintable(|b| {
        b.read("_", int());
    });
    unprintable(|b| {
        b.list("_", int().until(0));
    });
    unprintable(|b| {
        b.let_("_", lit(1));
    });
    unprintable(|b| {
        b.param("_");
    });
    unprintable(|b| {
        b.define_type("_", int());
    });
    unprintable(|b| {
        b.capture("_", int());
    });
    unprintable(|b| {
        b.optional_capture("_", int());
    });
    unprintable(|b| {
        b.repeat_until("x").store("_");
    });
    unprintable(|b| {
        b.repeat_until("x").store(discard());
    });
    unprintable(|b| {
        b.sections().until("x").store(discard());
    });
    unprintable(|b| {
        b.read("gecode.x", int());
    });
    unprintable(|b| {
        b.read("not a name", int());
    });
    unprintable(|b| {
        b.read("", int());
    });
    unprintable(|b| {
        let a = b.read("a", int().dim(1));
        b.let_("v", all(over(a).named("not a name"), |x| x));
    });
    unprintable(|b| {
        b.let_("x", lit(f64::NAN));
    });
    unprintable(|b| {
        b.let_("x", lit(f64::INFINITY));
    });
    unprintable(|b| {
        b.read("x", lit(0)..);
        b.format("second");
    });
    unprintable(|b| {
        b.define_type("t", lit(0)..);
        b.format("second");
    });
    unprintable(|b| {
        b.read("x", alphabet("ab").base(i64::MIN));
    });
    unprintable(|b| {
        b.list("x", int().layout(1).until(0));
    });
    unprintable(|b| {
        let t = lit("}").dynamic();
        b.line(gecode_iris_build::Pattern::new().part(gecode_iris_build::validate(t..)));
    });
}

#[test]
fn a_bound_after_an_open_range_is_fine_before_other_statements() {
    let mut b = SpecBuilder::unnamed();
    b.read("x", lit(0)..);
    b.read("y", lit(0)..);
    b.instances(|_| {});
    b.format("second");
    assert!(b.build().is_ok());
}

#[test]
fn a_capture_binds_exactly_once_in_its_scope() {
    let mut b = SpecBuilder::unnamed();
    let n = b.capture("n", int());
    b.line(pattern!("{n}"));
    b.line(pattern!("{n}"));
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("twice"), "{error}");

    let mut b = SpecBuilder::unnamed();
    b.capture("n", int());
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("never"), "{error}");

    let mut b = SpecBuilder::unnamed();
    let n = b.capture("n", int());
    b.repeat(1, |r| r.line(pattern!("{n}")));
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("different scope"), "{error}");
}

#[test]
fn the_presence_of_a_capture_follows_its_arm() {
    let mut b = SpecBuilder::unnamed();
    let n = b.capture("n", int());
    b.sections().body(|arms| {
        arms.arm(pattern!("{n}")).optional();
    });
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("optional_capture"), "{error}");

    let mut b = SpecBuilder::unnamed();
    let n = b.optional_capture("n", int());
    b.line(pattern!("{n}"));
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("`capture`"), "{error}");

    let mut b = SpecBuilder::unnamed();
    let n = b.optional_capture("n", int());
    b.sections().body(|arms| {
        arms.arm(pattern!("{n}")).optional();
    });
    assert!(b.build().is_ok());
}

#[test]
fn a_handle_that_is_not_a_capture_is_not_guessed() {
    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    b.line(pattern!("{x}"));
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("`reference`"), "{error}");

    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    b.line(pattern!("{x}", x = reference(x)));
    assert!(b.build().is_ok());
}

#[test]
fn a_forward_handle_must_be_bound() {
    let mut b = SpecBuilder::unnamed();
    let _: Field<Int> = b.forward("x");
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("never"), "{error}");

    let mut b = SpecBuilder::unnamed();
    let _ = b.forward_record("each_one", "all_of_them");
    assert_kind!(failure_of(b), BuildErrorKind::Binding);
}

#[test]
fn a_forward_handle_must_be_bound_as_declared() {
    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.read(x, float());
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.repeat(2, |r| {
        r.read(x, int());
    });
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.if_(1, |t| {
        t.read(x, int());
    });
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let x: Field<Int, D1> = b.forward("x");
    b.repeat(2, |r| {
        r.read(x, int());
    });
    assert!(b.build().is_ok());
}

#[test]
fn a_later_definition_of_another_type_is_a_redefinition() {
    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.read(x, int());
    let again: DynField = b.read(x, float());
    let data = b
        .build()
        .unwrap()
        .read(b"1 2.5", "input", &Params::new())
        .unwrap();
    assert_eq!(data.get(again).unwrap().as_float().unwrap(), 2.5);
    assert!(data.try_get(x).is_err(), "the forward handle is stale");

    // Only a definition of the promised type counts as its binding.
    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.if_(1, |t| {
        t.read(x, int());
    });
    b.if_(1, |t| {
        t.read(x, int());
        t.read(x, float());
    });
    assert_kind!(failure_of(b), BuildErrorKind::Binding);
}

#[test]
fn a_redefinition_needs_a_binding_that_always_precedes_it() {
    let mut b = SpecBuilder::unnamed();
    let x: Field<Int, D0, Maybe> = b.forward("x");
    b.if_(1, |t| {
        t.read(x, int());
    })
    .else_(|e| {
        e.read(x, float());
    });
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("float at depth 0"), "{error}");

    let mut b = SpecBuilder::unnamed();
    let x: Field<Int, D0, Maybe> = b.forward("x");
    b.if_(1, |t| {
        t.read(x, int());
    });
    b.read(x, float());
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let x: Field<Int, D0, Maybe> = b.forward("x");
    b.sections().body(|arms| {
        arms.arm(pattern!("{x}", x = bind(x, int()))).optional();
        arms.arm(pattern!("{x}", x = bind(x, float()))).optional();
    });
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    // A binding inside a block precedes what follows it in that block.
    let mut b = SpecBuilder::unnamed();
    let x: Field<Int, D0, Maybe> = b.forward("x");
    b.if_(1, |t| {
        t.read(x, int());
        t.if_(1, |nested| {
            nested.read(x, float());
        });
    });
    assert!(b.build().is_ok());
}

#[test]
fn an_unknown_shape_does_not_bind_a_static_forward_handle() {
    type Build = fn(&mut SpecBuilder, Field<Int, D1>);
    let array_into = |build: Build| {
        let mut b = SpecBuilder::unnamed();
        let a = b.read("a", int().dim(2));
        build(&mut b, a);
        b.build()
    };

    // The operand's type knows it is an array.
    let error = array_into(|b, a| {
        let x: Field<Int> = b.forward("x");
        b.let_(x, a);
    })
    .unwrap_err();
    let error = assert_kind!(error, BuildErrorKind::Binding);
    assert!(error.to_string().contains("int at depth 1"), "{error}");

    // The operand's type does not know, whatever `ex()` asserts.
    let builds: [Build; 3] = [
        |b, a| {
            let x: Field<Int> = b.forward("x");
            b.let_(x, a.at(0));
        },
        |b, a| {
            let x: Field<Int> = b.forward("x");
            b.let_(x, a.ex());
        },
        |b, a| {
            let x: Field<Int> = b.forward("x");
            b.let_(x, a.dynamic());
        },
    ];
    for build in builds {
        let error = array_into(build).unwrap_err();
        assert_kind!(error, BuildErrorKind::Binding);
    }

    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.let_(x, lit("text").dynamic());
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.param(x);
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    // Inside a repeat a bare handle can be a record's value or a whole array.
    let mut b = SpecBuilder::unnamed();
    let y: Field<Int, D1> = b.forward("y");
    b.repeat(1, |r| {
        let v = r.read("v", int());
        r.let_(y, v);
    });
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("unknown depth"), "{error}");
}

#[test]
fn a_known_shape_binds_a_static_forward_handle() {
    let mut b = SpecBuilder::unnamed();
    let a = b.read("a", int().dim(2));
    let n = b.read("n", int());
    let whole: Field<Int, D1> = b.forward("whole");
    b.let_(whole, a);
    let scalar: Field<Int> = b.forward("scalar");
    b.let_(scalar, n);
    let computed: Field<Int> = b.forward("computed");
    b.let_(computed, n + 1);
    let rows: Field<Int, D1> = b.forward("rows");
    b.repeat(2, |r| {
        r.let_(rows, n + 2);
    });
    let spec = b.build().unwrap();
    let data = spec.read(b"1 2 3", "input", &Params::new()).unwrap();
    assert_eq!(data.get(whole), &[1, 2]);
    assert_eq!(data.get(scalar), 3);
    assert_eq!(data.get(computed), 4);
    assert_eq!(data.get(rows), &[5, 5]);

    // A dynamic forward handle takes what a static one cannot certify.
    let mut b = SpecBuilder::unnamed();
    let a = b.read("a", int().dim(2));
    let x: DynField = b.forward("x");
    b.let_(x, a.at(0));
    let y: DynField = b.forward("y");
    b.repeat(1, |r| {
        r.let_(y, a);
    });
    let data = b
        .build()
        .unwrap()
        .read(b"7 8", "input", &Params::new())
        .unwrap();
    assert_eq!(data.get(x).unwrap().as_int().unwrap(), 7);
    assert!(data.get(y).is_some());
}

#[test]
fn set_keeps_a_shape_the_types_cannot_compare_until_the_walk() {
    // The depth of a bare handle inside a repeat is open, so `set` compiles
    // and a forward handle still needs its binding to be certain.
    let mut b = SpecBuilder::unnamed();
    let a = b.read("a", int().dim(2));
    let y: Field<Int, D1> = b.forward("y");
    b.repeat(1, |r| r.set(y, a));
    assert_kind!(failure_of(b), BuildErrorKind::Binding);
}

#[test]
fn a_named_type_handle_belongs_to_its_declaration() {
    let mut b = SpecBuilder::unnamed();
    let old = b.if_(1, |s| s.define_type("t", int())).finish();
    b.if_(1, |s| {
        s.define_type("t", float());
        s.read("x", old);
    });
    let error = assert_kind!(failure_of(b), BuildErrorKind::Scope);
    assert!(error.to_string().contains("`t`"), "{error}");

    // Through a capture, whose type a later pattern prints.
    let mut b = SpecBuilder::unnamed();
    let old = b.if_(1, |s| s.define_type("t", int())).finish();
    b.if_(1, |s| {
        s.define_type("t", float());
        let x = s.optional_capture("x", old);
        s.line(pattern!("{x}"));
    });
    assert_kind!(failure_of(b), BuildErrorKind::Scope);

    // Through a pattern and a type definition.
    let mut b = SpecBuilder::unnamed();
    let old = b.if_(1, |s| s.define_type("t", int())).finish();
    b.if_(1, |s| {
        s.define_type("t", float());
        s.line(pattern!("{x}", x = gecode_iris_build::validate(old)));
    });
    assert_kind!(failure_of(b), BuildErrorKind::Scope);

    // After its block no declaration is visible.
    let mut b = SpecBuilder::unnamed();
    let old = b.if_(1, |s| s.define_type("t", int())).finish();
    b.read("x", old);
    assert_kind!(failure_of(b), BuildErrorKind::Scope);

    // Each handle works where its own declaration is visible.
    let mut b = SpecBuilder::unnamed();
    let (mut first, mut second) = (None, None);
    b.if_(1, |s| {
        let t = s.define_type("t", int());
        first = Some(s.read("first", t));
        s.if_(1, |nested| nested.read("nested", t));
    });
    b.if_(1, |s| {
        let t = s.define_type("t", float());
        second = Some(s.read("second", t));
    });
    let data = b
        .build()
        .unwrap()
        .read(b"1 2 2.5", "input", &Params::new())
        .unwrap();
    assert_eq!(data.get(first.unwrap()), Some(1));
    let second: Field<Float, D0, Maybe> = second.unwrap();
    assert_eq!(data.get(second), Some(2.5));
}

#[test]
fn only_a_store_defines_the_entry_named_underscore() {
    let binding = |build: fn(&mut SpecBuilder, DynField)| {
        let mut b = SpecBuilder::unnamed();
        let stored: DynField = b.forward("_");
        b.repeat_until("END").store(stored);
        build(&mut b, stored);
        let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
        assert!(error.to_string().contains("`store`"), "{error}");
    };
    binding(|b, stored| {
        b.read(stored, int());
    });
    binding(|b, stored| {
        b.list(stored, int().until(0));
    });
    binding(|b, stored| {
        b.let_(stored, lit(1));
    });
    binding(|b, stored| {
        b.param(stored);
    });
    binding(|b, stored| {
        b.line(pattern!("{x}", x = bind(stored, int())));
    });

    let mut b = SpecBuilder::unnamed();
    let never: DynField = b.forward("_");
    b.check(defined(never));
    assert_kind!(failure_of(b), BuildErrorKind::Binding);
}

#[test]
fn a_store_of_underscore_reads_as_an_entry_at_the_top_level() {
    let mut b = SpecBuilder::unnamed();
    let stored: Field<Str> = b.forward("_");
    b.repeat_until("END").store(stored);
    let copy = b.let_("s", stored);
    b.line(pattern!("{again}", again = reference(stored)));
    let spec = b.build().unwrap();
    assert_eq!(
        spec.source(),
        "repeat until \"END\" store _ {}\nlet s = _\nline \"{$_}\"\n"
    );
    let data = spec.read(b"END\nEND", "input", &Params::new()).unwrap();
    assert_eq!(data.get(stored), "END");
    assert_eq!(data.get(copy).as_str().unwrap(), "END");

    let mut b = SpecBuilder::unnamed();
    let stored: Field<Int> = b.forward("_");
    b.repeat_until("END").store(stored);
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let stored: Field<Str, D0, Maybe> = b.forward("_");
    b.sections().until("END").store(stored).body(|_| {});
    let data = b
        .build()
        .unwrap()
        .read(b"END", "input", &Params::new())
        .unwrap();
    assert_eq!(data.get(stored), Some("END"));
}

#[test]
fn a_store_of_underscore_in_a_repeat_leaves_no_entry() {
    // No repeat collects `_` into a column, so no array handle can read it.
    let mut b = SpecBuilder::unnamed();
    let stored: Field<Str, D1> = b.forward("_");
    b.repeat(1, |r| r.repeat_until("END").store(stored).body(|_| {}));
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let stored: Field<Str> = b.forward("_");
    b.repeat(1, |r| r.repeat_until("END").store(stored).body(|_| {}));
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("Maybe"), "{error}");

    // Inside the iteration the value is there; after it the handle reads none.
    let mut b = SpecBuilder::unnamed();
    let stored: DynField = b.forward("_");
    let copy = b.repeat(2, |r| {
        r.repeat_until("END").store(stored);
        r.let_("copy", stored)
    });
    let spec = b.build().unwrap();
    let data = spec.read(b"END END", "input", &Params::new()).unwrap();
    assert!(data.get(stored).is_none());
    assert_eq!(data.get(copy).depth(), 1);

    let mut b = SpecBuilder::unnamed();
    let stored: Field<Str, D0, Maybe> = b.forward("_");
    b.repeat(1, |r| r.repeat_until("END").store(stored).body(|_| {}));
    let data = b
        .build()
        .unwrap()
        .read(b"END", "input", &Params::new())
        .unwrap();
    assert_eq!(data.get(stored), None);
}

#[test]
fn underscore_names_records() {
    let mut b = SpecBuilder::unnamed();
    let x = b.repeat(1, |r| {
        r.as_name("_");
        r.each_name("row");
        r.read("x", int())
    });
    let _ = x;
    let spec = b.build().unwrap();
    assert!(
        spec.source().contains("repeat 1 as _ {"),
        "{}",
        spec.source()
    );

    let mut b = SpecBuilder::unnamed();
    let (record, x) = b.repeat(1, |r| {
        r.each_name("_");
        (r.record(), r.read("x", int()))
    });
    b.check(record.get(x).eq(1));
    let spec = b.build().unwrap();
    assert!(
        spec.source().contains("check _.x == 1"),
        "{}",
        spec.source()
    );
    assert!(spec.read(b"1", "input", &Params::new()).is_ok());

    let mut b = SpecBuilder::unnamed();
    let record = b.forward_record("_", "rows");
    b.repeat(1, |r| {
        r.record_as(record);
        r.read("x", int());
    });
    let n = b.let_("n", size(record));
    let data = b
        .build()
        .unwrap()
        .read(b"1", "input", &Params::new())
        .unwrap();
    assert_eq!(data.get(n), 1);
}

#[test]
fn a_conditional_and_a_later_definition_are_not_optional() {
    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.if_(1, |t| {
        t.read(x, int());
    });
    b.let_(x, lit(1));
    assert!(b.build().is_ok());
}

#[test]
fn a_pattern_binds_a_forward_handle() {
    let mut b = SpecBuilder::unnamed();
    let x: Field<Str> = b.forward("x");
    b.line(pattern!("x = {x}", x = bind(x, word())));
    assert!(b.build().is_ok());

    let mut b = SpecBuilder::unnamed();
    let x: Field<Int> = b.forward("x");
    b.line(pattern!("x = {x}", x = bind(x, word())));
    assert_kind!(failure_of(b), BuildErrorKind::Binding);
}

#[test]
fn an_undefined_name_cannot_be_defined() {
    let mut b = SpecBuilder::unnamed();
    let zz = b.undefined("zz");
    b.check(!defined(zz));
    assert!(b.source().is_ok());
    b.let_(zz, lit(1));
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("undefined"), "{error}");

    let mut b = SpecBuilder::unnamed();
    let underscore = b.undefined("_");
    b.repeat_until("END").store(underscore);
    let error = assert_kind!(failure_of(b), BuildErrorKind::Binding);
    assert!(error.to_string().contains("undefined"), "{error}");
}

#[test]
fn a_store_defines_a_forward_handle() {
    let mut b = SpecBuilder::unnamed();
    let end: Field<Str> = b.forward("end");
    b.repeat_until("END").store(end).body(|r| {
        r.read("x", int());
    });
    assert!(b.build().is_ok());

    // End of file and `last` end sections without storing.
    let mut b = SpecBuilder::unnamed();
    let end: Field<Str> = b.forward("end");
    b.sections().until("END").store(end).body(|_| {});
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let end: Field<Str, gecode_iris_build::D0, Maybe> = b.forward("end");
    b.sections().until("END").store(end).body(|_| {});
    assert!(b.build().is_ok());

    let mut b = SpecBuilder::unnamed();
    b.sections().skip_other().store("end").body(|_| {});
    assert_kind!(failure_of(b), BuildErrorKind::Unprintable);
}

#[test]
fn a_store_does_not_precede_the_body_of_its_repeat() {
    // `store` runs when the repeat ends, so the `let` is the first definition.
    let mut b = SpecBuilder::unnamed();
    let x: Field<Str> = b.forward("x");
    b.repeat_until("END").store(x).body(|r| {
        r.let_(x, lit(1));
        r.read(discard(), int());
    });
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    // After the repeat, the store does precede what follows.
    let mut b = SpecBuilder::unnamed();
    let x: Field<Str> = b.forward("x");
    b.repeat_until("END").store(x).body(|r| {
        r.read(discard(), int());
    });
    b.read(x, int());
    assert!(b.build().is_ok());
}

#[test]
fn a_num_forward_handle_takes_an_int_or_a_float() {
    let mut b = SpecBuilder::unnamed();
    let n: Field<Num> = b.forward("n");
    b.read(n, int());
    let spec = b.build().expect("an int is a number");
    let data = spec.read(b"7", "input", &Params::new()).unwrap();
    assert!(data.try_get(n).is_ok());

    let mut b = SpecBuilder::unnamed();
    let n: Field<Num> = b.forward("n");
    b.let_(n, lit(1.5));
    assert!(b.build().is_ok());

    let mut b = SpecBuilder::unnamed();
    let n: Field<Num> = b.forward("n");
    b.read(n, word());
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let n: Field<Num> = b.forward("n");
    b.read(n, int().dim(2));
    assert_kind!(failure_of(b), BuildErrorKind::Binding);
}

#[test]
fn a_forward_record_binds_once() {
    let mut b = SpecBuilder::unnamed();
    let job = b.forward_record("job", "jobs");
    b.repeat(1, |r| r.record_as(job));
    b.repeat(1, |r| r.record_as(job));
    assert_kind!(failure_of(b), BuildErrorKind::Binding);

    let mut b = SpecBuilder::unnamed();
    let own = b.repeat(1, |r| r.record());
    b.repeat(1, |r| r.record_as(own));
    assert_kind!(failure_of(b), BuildErrorKind::Binding);
}

#[test]
fn the_language_decides_the_rest() {
    let mut b = SpecBuilder::unnamed();
    let x: DynField = b.undefined("x");
    b.let_("y", x);
    b.read(discard(), int());
    let spec = b.build().expect("an undefined name is a read-time error");
    assert!(spec.source().contains("let y = x"));

    let mut b = SpecBuilder::unnamed();
    b.instances(|_| {});
    b.read("after", int());
    let error = assert_kind!(failure_of(b), BuildErrorKind::Spec(_));
    assert!(error.failure().is_some());
    assert!(error.text().unwrap().contains("after: int"));
}
