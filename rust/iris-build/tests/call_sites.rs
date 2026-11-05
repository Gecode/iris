//! Errors point at the host code: every statement records the line that called
//! it, and failures carry that line beside their location in the emitted text.
use gecode_iris_build::{
    BuildError, BuildErrorKind, D0, Field, Int, Maybe, Scope, SpecBuilder, alphabet, float, int,
    lit, pattern, rest, word,
};
use gecode_iris_core::{CallSite, ErrorCode, Failure, Params, Spec};
use std::cell::RefCell;

fn assert_here(site: Option<CallSite>, line: u32) {
    let site = site.expect("the failure has a call site");
    assert_eq!(site.file(), file!());
    assert_eq!(site.line(), line);
}

fn language_error(failure: &Failure) -> &gecode_iris_core::Error {
    match failure {
        Failure::Language(error) => error,
        other => panic!("not a language failure: {other}"),
    }
}

fn spec_error(error: &BuildError) -> &gecode_iris_core::Error {
    language_error(error.failure().expect("the emitted text is rejected"))
}

#[test]
fn a_parse_error_names_the_statement_after_instances() {
    let mut b = SpecBuilder::unnamed();
    b.instances(|_| {});
    let line = line!() + 1;
    b.read("after", int());
    let error = b.build().expect_err("instances end a spec");

    assert_here(error.call_site(), line);
    assert_here(spec_error(&error).call_site(), line);
    let location = spec_error(&error).spec_location().unwrap();
    assert_eq!(location.origin.as_deref(), Some("builder.iris"));
    assert_eq!(location.line, Some(2));
    assert_eq!(error.emitted_line(), Some("after: int"));
    assert_eq!(error.text(), Some("instances {}\nafter: int\n"));
}

#[test]
fn a_resolution_error_names_the_statement_that_uses_the_name() {
    let mut b = SpecBuilder::unnamed();
    let outside = b.read("outside", int());
    let record = b.repeat(2, |s| {
        s.read("inside", int());
        s.record()
    });
    let line = line!() + 1;
    b.let_("bad", record.get(outside));
    let error = b.build().expect_err("the record has no such field");

    let spec = spec_error(&error);
    assert_eq!(spec.code.as_str(), "E-SPEC-RECORD-SCOPE");
    assert_here(error.call_site(), line);
    assert_eq!(spec.spec_location().unwrap().line, Some(6));
    assert_eq!(error.emitted_line(), Some("let bad = record_1.outside"));
}

#[test]
fn the_display_shows_the_code_message_emitted_line_and_call_site() {
    let mut b = SpecBuilder::unnamed();
    b.instances(|_| {});
    let line = line!() + 1;
    b.read("after", int());
    let error = b.build().expect_err("instances end a spec");

    let message = spec_error(&error).message().to_owned();
    let code = spec_error(&error).code.as_str();
    assert_eq!(
        error.to_string(),
        format!(
            "{code}: builder.iris:2: {message}\n     2 | after: int\ncall site: {}:{line}:7",
            file!()
        )
    );
}

#[test]
fn a_failed_check_names_the_check_in_its_block() {
    let mut b = SpecBuilder::new("row");
    let n = b.read("n", int());
    let mut line = 0;
    b.repeat(n, |s| {
        let x = s.read("x", int());
        line = line!() + 1;
        s.check(x.gt(0)).message("not positive");
    });
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"2 1 -1", "input.txt", &Params::new())
        .expect_err("the check fails");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::CheckFailed);
    assert_here(error.call_site(), line);
    assert_eq!(error.spec_location().unwrap().line, Some(5));
    assert!(
        failure
            .to_string()
            .ends_with(&format!("\ncall site: {}:{line}:11", file!())),
        "{failure}"
    );
}

#[test]
fn a_read_failure_names_the_declaration_that_read() {
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    b.read("n", int());
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"word", "input.txt", &Params::new())
        .expect_err("not a number");

    assert_here(language_error(&failure).call_site(), line);
}

#[test]
fn a_catalog_failure_names_the_statement() {
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    b.read("n", int());
    b.instances(|_| {});
    let spec = b.build().unwrap();
    let failure = spec
        .catalog(b"word", "input.txt", &Params::new())
        .expect_err("not a number");

    assert_here(language_error(&failure).call_site(), line);
}

#[test]
fn a_deferred_rejection_keeps_its_call_site() {
    let mut b = SpecBuilder::unnamed();
    let mut line = 0;
    b.instances(|s| {
        let n = s.read("n", int());
        line = line!() + 1;
        s.check(n.gt(0));
    });
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"0", "input.txt", &Params::new())
        .expect_err("the only instance is rejected");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::CheckFailed);
    assert_here(error.call_site(), line);
}

#[test]
fn a_failure_without_a_statement_has_no_call_site() {
    let mut b = SpecBuilder::unnamed();
    b.read("n", int());
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"1 2", "input.txt", &Params::new())
        .expect_err("trailing input");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::TrailingInput);
    assert_eq!(error.call_site(), None);
    assert!(!failure.to_string().contains("call site"), "{failure}");
}

/// The same text, parsed and built, differs only in the call site.
#[test]
fn a_spec_parsed_from_text_has_no_call_sites() {
    let mut b = SpecBuilder::unnamed();
    let n = b.read("n", int());
    b.check(n.gt(0)).message("not positive");
    let built = b.build().unwrap();
    let parsed = Spec::parse(built.source().as_bytes(), "builder.iris").unwrap();
    assert_eq!(parsed.call_sites().at(0), None);

    let read = |spec: &Spec| spec.read(b"0", "input.txt", &Params::new()).unwrap_err();
    let (from_text, from_builder) = (read(&parsed), read(&built));
    let (text_error, builder_error) = (language_error(&from_text), language_error(&from_builder));

    assert_eq!(text_error.call_site(), None);
    assert_eq!(text_error.code, builder_error.code);
    assert_eq!(text_error.message(), builder_error.message());
    assert_eq!(text_error.spec_span(), builder_error.spec_span());
    let (text_location, builder_location) = (
        text_error.spec_location().unwrap(),
        builder_error.spec_location().unwrap(),
    );
    assert_eq!(text_location.line, builder_location.line);
    let site = builder_error.call_site().unwrap();
    assert_eq!(
        format!("{from_text}\ncall site: {site}"),
        from_builder.to_string()
    );

    let rejected = Spec::parse(b"instances {}\nafter: int\n", "builder.iris").unwrap_err();
    assert_eq!(language_error(&rejected).call_site(), None);
    assert!(!rejected.to_string().contains("call site"), "{rejected}");
}

#[test]
fn a_builder_check_names_the_statement_that_failed_it() {
    let mut b = SpecBuilder::unnamed();
    b.read("x", int());
    let line = line!() + 1;
    b.read("x", int());
    let error = b.build().unwrap_err();
    assert!(matches!(error.kind(), BuildErrorKind::Redefinition));
    assert_here(error.call_site(), line);
    assert!(
        error.to_string().ends_with(&format!(":{line}:7")),
        "{error}"
    );
}

#[test]
fn a_handle_from_another_builder_names_the_statement_that_used_it() {
    let mut other = SpecBuilder::unnamed();
    let foreign = other.read("x", int());
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    b.let_("y", foreign);
    let error = b.build().unwrap_err();
    assert!(matches!(error.kind(), BuildErrorKind::Provenance));
    assert_here(error.call_site(), line);
}

#[test]
fn a_handle_never_bound_names_the_call_that_made_it() {
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    let _: Field<Int> = b.forward("later");
    let error = b.build().unwrap_err();
    assert!(matches!(error.kind(), BuildErrorKind::Binding));
    assert_here(error.call_site(), line);

    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    b.capture("n", int());
    assert_here(b.build().unwrap_err().call_site(), line);

    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    b.forward_record("each", "list");
    assert_here(b.build().unwrap_err().call_site(), line);
}

#[test]
fn a_binding_failure_names_the_definition() {
    let mut b = SpecBuilder::unnamed();
    let name = b.undefined("name");
    let line = line!() + 1;
    b.read(name, int());
    let error = b.build().unwrap_err();
    assert!(matches!(error.kind(), BuildErrorKind::Binding));
    assert_here(error.call_site(), line);
}

#[test]
fn an_unprintable_value_names_the_statement_that_holds_it() {
    let mut b = SpecBuilder::unnamed();
    b.read("x", int());
    let line = line!() + 1;
    b.let_("y", lit(f64::NAN));
    let error = b.build().unwrap_err();
    assert!(matches!(error.kind(), BuildErrorKind::Unprintable));
    assert_here(error.call_site(), line);
}

#[test]
fn a_guard_used_late_names_the_guard_call() {
    let mut b = SpecBuilder::unnamed();
    let n = b.read("n", int());
    let check = b.check(n.gt(0));
    b.read("m", int());
    let line = line!() + 1;
    check.message("late");
    let error = b.build().unwrap_err();
    assert!(matches!(error.kind(), BuildErrorKind::Scope));
    assert_here(error.call_site(), line);
}

fn without_attribute(s: &mut Scope, line: &mut u32) {
    *line = line!() + 1;
    s.read("x", int());
}

#[track_caller]
fn with_attribute(s: &mut Scope) {
    s.read("x", int());
}

#[test]
fn a_helper_records_its_own_line_unless_it_tracks_its_caller() {
    let read = |spec: Spec| spec.read(b"word", "i", &Params::new()).unwrap_err();

    let mut b = SpecBuilder::unnamed();
    let mut line = 0;
    without_attribute(&mut b, &mut line);
    let failure = read(b.build().unwrap());
    assert_here(language_error(&failure).call_site(), line);

    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    with_attribute(&mut b);
    let failure = read(b.build().unwrap());
    assert_here(language_error(&failure).call_site(), line);
}

#[test]
fn a_method_used_as_a_function_value_records_its_definition() {
    let mut b = SpecBuilder::unnamed();
    let skip: fn(&mut Scope) = Scope::skip_line;
    skip(&mut b);
    let spec = b.build().unwrap();
    let site = spec.call_sites().at(0).expect("a statement");
    assert!(site.file().ends_with("scope.rs"), "{site}");
}

/// Records what a statement's text starts with and the line of the call.
#[derive(Default)]
struct Expected(RefCell<Vec<(&'static str, u32)>>);

/// Run a statement, on one source line, and expect the text `$text` from it.
macro_rules! at {
    ($expected:expr, $text:literal, $call:expr) => {{
        $expected.0.borrow_mut().push(($text, line!()));
        $call
    }};
}

/// Check each statement's call site, in text order.
fn assert_sites(spec: &Spec, expected: &Expected) {
    let text = spec.source();
    let mut cursor = 0;
    for (start, line) in expected.0.borrow().iter() {
        let offset = cursor
            + text[cursor..]
                .find(start)
                .unwrap_or_else(|| panic!("no `{start}` after byte {cursor} of:\n{text}"));
        let site = spec.call_sites().at(offset).expect("a statement");
        assert_eq!(
            (*start, site.file(), site.line()),
            (*start, file!(), *line),
            "{text}"
        );
        cursor = offset + start.len();
    }
}

#[test]
fn every_setting_and_plain_statement_records_its_call() {
    let e = Expected::default();
    #[rustfmt::skip]
    let spec = {
        let mut b = at!(e, "format first", SpecBuilder::new("first"));
        at!(e, "format second", b.format("second"));
        at!(e, "comment", b.comment("#"));
        at!(e, "stop", b.stop("END"));
        at!(e, "tokens", b.tokens(":"));
        at!(e, "separators", b.separators(","));
        at!(e, "ignore trailing", b.ignore_trailing());
        at!(e, "param p", b.param("p"));
        at!(e, "param q", b.param_default("q", lit(1)));
        at!(e, "type t", b.define_type("t", int()));
        let n = at!(e, "n: int", b.read("n", int()));
        at!(e, "list l", b.list("l", int().until(lit(0))));
        let v = at!(e, "let v", b.let_("v", lit(1)));
        at!(e, "let v", b.set(v, lit(2)));
        at!(e, "check", b.check(n.ge(0)));
        at!(e, "expect", b.expect("word"));
        at!(e, "skip line", b.skip_line());
        at!(e, "skip 1 lines", b.skip_lines(1));
        let m = b.capture("m", word());
        at!(e, "line", b.line(pattern!("m {m}")));
        at!(e, "find", b.find("x"));
        at!(e, "index", b.index(lit(0)));
        b.build().unwrap()
    };
    assert_sites(&spec, &e);
}

#[test]
fn every_block_statement_records_its_call() {
    let e = Expected::default();
    #[rustfmt::skip]
    let spec = {
        let mut b = SpecBuilder::unnamed();
        let n = at!(e, "n: int", b.read("n", int()));
        at!(e, "if n", b.if_(n.gt(0), |s| {
            at!(e, "a: int", s.read("a", int()));
        }))
        .else_if(n.lt(0), |s| {
            at!(e, "b: int", s.read("b", int()));
        })
        .else_(|s| {
            at!(e, "c: int", s.read("c", int()));
        });
        at!(e, "repeat 2", b.repeat(2, |s| {
            at!(e, "d: int", s.read("d", int()));
        }));
        at!(e, "repeat until \"END\"", b.repeat_until("END")).body(|s| {
            at!(e, "f: int", s.read("f", int()));
        });
        at!(e, "sections", b.sections()).body(|arms| {
            at!(e, "\"one\"", arms.arm("one")).body(|s| {
                at!(e, "h: int", s.read("h", int()));
            });
        });
        at!(e, "repeat until eof", b.repeat_until_eof()).body(|s| {
            at!(e, "i: float", s.read("i", float()));
        });
        at!(e, "instances", b.instances(|s| {
            at!(e, "j: int", s.read("j", int()));
        }));
        b.build().unwrap()
    };
    assert_sites(&spec, &e);
}

#[test]
fn the_guards_of_if_record_their_calls() {
    let e = Expected::default();
    let mut b = SpecBuilder::unnamed();
    let n = b.read("n", int());
    let branch = b.if_(n.gt(0), |_| {});
    let branch = at!(e, "else if n", branch.else_if(n.lt(0), |_| {}));
    at!(e, "else if:", branch.else_read_if("if", int()));
    assert_sites(&b.build().unwrap(), &e);
}

#[test]
fn a_closing_brace_belongs_to_the_call_that_wrote_the_block() {
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    b.repeat(2, |s| {
        s.read("x", int());
    });
    b.read("y", int());
    let spec = b.build().unwrap();
    let text = spec.source();
    let brace = text.find('}').unwrap();
    assert_here(spec.call_sites().at(brace), line);
}

#[test]
fn an_empty_block_gives_its_brace_to_the_call_that_wrote_it() {
    let mut b = SpecBuilder::unnamed();
    let guard = b.repeat_until("END");
    let line = line!() + 1;
    guard.body(|_| {});
    let spec = b.build().unwrap();
    let text = spec.source();
    assert_eq!(text, "repeat until \"END\" {}\n");
    assert_here(spec.call_sites().at(text.find('}').unwrap()), line);
}

/// A declaration's form is checked at the token after it, which may start
/// another statement. The location stays there; the call site does not.
fn assert_owner(error: &BuildError, line: u32, emitted_line: usize, emitted: Option<&str>) {
    let spec = spec_error(error);
    assert_eq!(spec.code.as_str(), "E-SPEC-TYPE");
    assert_eq!(spec.message(), "'rest' values cannot be arrays");
    assert_here(error.call_site(), line);
    assert_here(spec.call_site(), line);
    assert_eq!(spec.spec_location().unwrap().line, Some(emitted_line));
    assert_eq!(error.emitted_line(), emitted);
    let parsed = Spec::parse(error.text().unwrap().as_bytes(), "builder.iris").unwrap_err();
    let parsed = language_error(&parsed);
    assert_eq!(parsed.call_site(), None);
    assert_eq!(parsed.code, spec.code);
    assert_eq!(parsed.message(), spec.message());
    assert_eq!(parsed.spec_span(), spec.spec_span());
}

#[test]
fn an_error_on_the_next_statement_names_the_statement_it_belongs_to() {
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    b.read("bad", rest().dim(2));
    b.read("next", int());
    let error = b.build().expect_err("'rest' cannot be an array");
    assert_owner(&error, line, 2, Some("next: int"));
}

#[test]
fn an_error_at_the_end_of_a_nested_block_names_the_statement_in_it() {
    let mut b = SpecBuilder::unnamed();
    let mut line = 0;
    b.repeat(1, |s| {
        s.repeat(1, |s| {
            line = line!() + 1;
            s.read("bad", rest().dim(2));
        });
    });
    let error = b.build().expect_err("'rest' cannot be an array");
    assert_owner(&error, line, 4, Some("  }"));
}

#[test]
fn an_error_at_the_end_of_an_else_block_names_the_statement_in_it() {
    let mut b = SpecBuilder::unnamed();
    let mut line = 0;
    b.if_(lit(1).eq(1), |_| {}).else_(|s| {
        line = line!() + 1;
        s.read("bad", rest().dim(2));
    });
    let error = b.build().expect_err("'rest' cannot be an array");
    assert_owner(&error, line, 3, Some("}"));
}

#[test]
fn an_error_at_the_end_of_the_spec_names_the_last_statement() {
    let mut b = SpecBuilder::unnamed();
    b.read("n", int());
    let line = line!() + 1;
    b.read("bad", rest().dim(2));
    let error = b.build().expect_err("'rest' cannot be an array");
    assert_owner(&error, line, 3, None);
}

fn nest(s: &mut Scope<D0, Maybe>, depth: usize, read: &mut u32, block: &mut u32) {
    if depth == 0 {
        *read = line!() + 1;
        s.read("x", int());
    } else {
        *block = line!() + 1;
        s.if_(lit(1).eq(1), |s| nest(s, depth - 1, read, block));
    }
}

#[test]
fn an_error_after_a_brace_names_the_block_that_opened_it() {
    let mut b = SpecBuilder::unnamed();
    let (mut read, mut block) = (0, 0);
    b.if_(lit(1).eq(1), |s| nest(s, 256, &mut read, &mut block));
    let error = b.build().expect_err("the blocks are nested too deeply");

    assert_eq!(spec_error(&error).code.as_str(), "E-SPEC-NESTING");
    assert_eq!(error.emitted_line().map(str::trim), Some("x: int"));
    assert_here(error.call_site(), block);
}

#[test]
fn an_error_in_an_arm_names_the_arm() {
    let mut b = SpecBuilder::unnamed();
    let cap = b.capture("bad", alphabet("ab"));
    let mut line = 0;
    b.sections().body(|arms| {
        line = line!() + 1;
        arms.arm(pattern!("{cap}"));
    });
    let error = b.build().expect_err("a capture cannot be of an alphabet");
    assert_eq!(spec_error(&error).code.as_str(), "E-SPEC-PATTERN");
    assert_here(error.call_site(), line);
}

#[test]
fn a_resolution_error_in_an_arm_names_the_arm() {
    let mut b = SpecBuilder::unnamed();
    let outside = b.read("outside", int());
    let record = b.repeat(2, |s| {
        s.each_name("row");
        s.read("inside", int());
        s.record()
    });
    let cap = b.capture("cap", lit(0)..record.get(outside).ex());
    let mut line = 0;
    b.sections().body(|arms| {
        line = line!() + 1;
        arms.arm(pattern!("{cap}"));
    });
    let error = b.build().expect_err("the record is out of scope");

    assert_eq!(spec_error(&error).code.as_str(), "E-SPEC-RECORD-SCOPE");
    assert_eq!(error.emitted_line(), Some("sections {"));
    assert_here(error.call_site(), line);
}

#[test]
fn a_store_failure_names_the_statement_that_stores() {
    let mut b = SpecBuilder::unnamed();
    let stored: Field<Int> = b.forward("stored");
    let line = line!() + 1;
    let repeat = b.repeat_until("END");
    let repeat = repeat.store(stored);
    repeat.body(|s| {
        s.read("x", int());
    });
    let error = b.build().expect_err("a terminator is a string");
    assert!(matches!(error.kind(), BuildErrorKind::Binding));
    assert_here(error.call_site(), line);
}

#[test]
fn a_repeat_that_reads_nothing_names_the_repeat() {
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    let repeat = b.repeat_until_eof();
    repeat.body(|s| {
        s.let_("x", lit(1));
    });
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"1", "input.txt", &Params::new())
        .expect_err("the body reads nothing");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::NoProgress);
    assert_eq!(error.spec_location().unwrap().line, Some(2));
    assert_here(error.call_site(), line);
}

#[test]
fn a_column_failure_names_the_repeat() {
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    b.repeat(2, |s| {
        let x = s.read("x", int());
        s.if_(x.gt(0), |s| {
            s.read("y", int());
        });
        s.read("z", int());
    });
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"1 2 3 0 4", "input.txt", &Params::new())
        .expect_err("`y` is in only some repetitions");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::RepeatMissing);
    assert_eq!(error.spec_location().unwrap().line, Some(6));
    assert_here(error.call_site(), line);
}

#[test]
fn a_read_failure_in_an_arm_names_the_arm() {
    let mut b = SpecBuilder::unnamed();
    let n = b.capture("n", 1..=2);
    let mut line = 0;
    b.sections().body(|arms| {
        line = line!() + 1;
        arms.arm(pattern!("n = {n}"));
    });
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"n = 3\n", "input.txt", &Params::new())
        .expect_err("out of range");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::ExpectedValue);
    assert_eq!(error.spec_location().unwrap().line, Some(1));
    assert_here(error.call_site(), line);
}

#[test]
fn a_line_a_last_arm_left_unread_names_its_arm() {
    let mut b = SpecBuilder::unnamed();
    let mut line = 0;
    b.sections().body(|arms| {
        line = line!() + 1;
        arms.arm("needed");
        arms.arm("end").last().body(|s| {
            s.read("x", int());
        });
    });
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"end\n1\n", "input.txt", &Params::new())
        .expect_err("the first arm has no line");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::LineMatch);
    assert_eq!(error.spec_location().unwrap().line, Some(4));
    assert_here(error.call_site(), line);
}

#[test]
fn a_line_no_arm_matches_names_the_sections() {
    let mut b = SpecBuilder::unnamed();
    let line = line!() + 1;
    let sections = b.sections();
    sections.body(|arms| {
        arms.arm("known");
    });
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"other\n", "input.txt", &Params::new())
        .expect_err("no arm matches");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::SectionsUnexpected);
    assert_here(error.call_site(), line);
}

#[test]
fn a_failure_after_sections_end_has_no_call_site() {
    let mut b = SpecBuilder::unnamed();
    b.sections().body(|arms| {
        arms.arm("end").last();
    });
    let spec = b.build().unwrap();
    let failure = spec
        .read(b"end\nmore", "input.txt", &Params::new())
        .expect_err("trailing input");

    let error = language_error(&failure);
    assert_eq!(error.code, ErrorCode::TrailingInput);
    assert_eq!(error.call_site(), None);
}

/// Every public method of a statement or a guard takes `#[track_caller]`,
/// except those that write no statement and check nothing.
#[test]
fn every_public_method_of_a_scope_tracks_its_caller() {
    const WRITE_NOTHING: [&str; 5] = ["record", "unnamed", "source", "build", "finish"];
    let source = include_str!("../src/scope.rs");
    let mut checked = 0;
    let mut previous_lines: Vec<&str> = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            let end = rest.find(['(', '<']).expect("a function has parameters");
            let name = &rest[..end];
            if !WRITE_NOTHING.contains(&name) {
                assert!(
                    previous_lines.contains(&"#[track_caller]"),
                    "`{name}` does not take #[track_caller]"
                );
                checked += 1;
            }
        }
        if trimmed.starts_with("#[") || trimmed.starts_with("///") {
            previous_lines.push(trimmed);
        } else {
            previous_lines.clear();
        }
    }
    assert!(checked >= 50, "only {checked} methods were found");
}
