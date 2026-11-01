use super::*;
use crate::Params;

fn reader<'a>(spec: &'a Spec, input: &'a [u8]) -> Interpreter<'a, 'a> {
    let mut reader = Interpreter::new(spec, Input::new(input, "input"));
    reader.capture_charges = false;
    reader
}

fn budget_error(failure: Failure) -> Error {
    let Failure::Language(error) = failure else {
        panic!("expected a language error")
    };
    assert_eq!(error.code.as_str(), "E-BUDGET");
    assert_eq!(error.record_path(), None);
    error
}

#[test]
fn budget_boundaries_and_fresh_operations() {
    let input = b"0 ".repeat(1024);
    // The input has 2048 bytes: 1056 * 1024 = B + 16*N.
    for (source, input, extra) in [
        ("_[1024][1023][0]:int", &b""[..], "repeat 1 {}"),
        (
            "a[1024]:int repeat 1056 {check sum(a)==0}",
            input.as_slice(),
            "check sum(1)==1",
        ),
    ] {
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        for _ in 0..2 {
            reader(&spec, input).read().unwrap();
        }
        reader(&spec, input).catalog().unwrap();
        let spec = Spec::parse(format!("{source}\n{extra}").as_bytes(), "spec").unwrap();
        let error = budget_error(reader(&spec, input).read().unwrap_err());
        assert_eq!(error.spec_location().unwrap().line, Some(2));
    }
}

#[test]
fn budget_charges_are_atomic_and_do_not_overflow() {
    let spec = Spec::parse(b"", "spec").unwrap();
    let mut reader = reader(&spec, b"\xc3\xa9\r\n");
    assert_eq!(reader.expression_remaining.get(), BASE_BUDGET + 64);
    reader.charge_idle(BASE_BUDGET).unwrap();
    reader.charge_expression(BASE_BUDGET + 64).unwrap();
    reader.charge_idle(0).unwrap();
    reader.charge_expression(0).unwrap();
    assert_eq!(reader.charge_idle(1).unwrap_err().code, ErrorCode::Budget);
    assert_eq!(
        reader.charge_expression(1).unwrap_err().code,
        ErrorCode::Budget
    );

    let mut reader = Interpreter::new(&spec, Input::new(b"", "input"));
    assert_eq!(
        reader.charge_idle(u128::MAX).unwrap_err().code,
        ErrorCode::Budget
    );
    assert_eq!(
        reader.charge_expression(u128::MAX).unwrap_err().code,
        ErrorCode::Budget
    );
}

#[test]
fn budget_failure_after_deferred_rejection_is_hard() {
    let spec = Spec::parse(
        b"_[1048576][0]:int\ninstances {_:int check 0 \"deferred\" repeat 1 {}}",
        "spec",
    )
    .unwrap();
    let mut params = Params::new();
    params.set("instance", "2");
    for catalog in [false, true] {
        let reader = reader(&spec, b"0\n1").with_params(&params);
        let error = budget_error(if catalog {
            reader.catalog().unwrap_err()
        } else {
            reader.read().unwrap_err()
        });
        assert_eq!(error.details.earlier_rejection.as_deref(), Some("deferred"));
        assert_eq!(error.input_location().unwrap().line, Some(1));
        assert_eq!(error.input_location().unwrap().column, Some(2));
        assert_eq!(error.spec_location().unwrap().line, Some(2));
        assert_eq!(error.context(), Some("instance [1] > repeat [1 of 1]"));
    }
}

#[test]
fn budgets_scale_linearly() {
    for (label, source, input) in [
        ("idle iterations", "repeat N {}", String::new()),
        (
            "empty dimension prefixes",
            "a[prefix][0]:int",
            String::new(),
        ),
        (
            "aggregate charges",
            "a[16]:int repeat N {check sum(a)==16}",
            "1 ".repeat(16),
        ),
        (
            "selected rows",
            "repeat N {a[0]:int} let z=size(a[0]) check all(r in a:size(r)==0)",
            String::new(),
        ),
    ] {
        let work = |n: usize| {
            let source = source
                .replace("[prefix]", &"[1]".repeat(n))
                .replace('N', &n.to_string());
            let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
            let mut reader = reader(&spec, input.as_bytes());
            reader.execute(spec.parsed.syntax()).unwrap();
            reader.work.get().into_iter().sum::<usize>()
        };
        let small = work(128);
        let large = work(512);
        println!("{label}: 1x={small}; 4x={large}");
        assert!(large <= 6 * small, "{label}: {large} > 6 * {small}");
    }
    let work = |n: usize| {
        let spec = Spec::parse(
            format!("a[{n}][2]:int check all(r in a:sum(r)==2)").as_bytes(),
            "spec",
        )
        .unwrap();
        let input = b"1 ".repeat(2 * n);
        let mut reader = reader(&spec, &input);
        reader.execute(spec.parsed.syntax()).unwrap();
        reader.work.get().into_iter().sum::<usize>()
    };
    let small = work(128);
    let large = work(512);
    println!("input rows: 1x={small}; 4x={large}");
    assert!(large <= 6 * small, "input rows: {large} > 6 * {small}");
}

#[test]
fn budget_exhaustion_stops_work_promptly() {
    for (source, input, counts) in [
        ("repeat N {}", Vec::new(), [1048577, i32::MAX]),
        (
            "a[1024]:int repeat N {check sum(a)==0}",
            b"0 ".repeat(1024),
            [1057, i32::MAX],
        ),
    ] {
        let mut work = Vec::new();
        for count in counts {
            let spec =
                Spec::parse(source.replace('N', &count.to_string()).as_bytes(), "spec").unwrap();
            let mut reader = reader(&spec, &input);
            budget_error(reader.execute(spec.parsed.syntax()).unwrap_err());
            let counts = reader.work.get();
            let total = counts.into_iter().sum::<usize>();
            assert!(total < 12 * BASE_BUDGET as usize);
            work.push(counts);
        }
        // A would-be run thousands of times longer stops at the same event.
        assert_eq!(work[0], work[1]);
    }
}
