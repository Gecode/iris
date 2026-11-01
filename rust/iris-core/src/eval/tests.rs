use super::*;
use crate::{Params, Spec, input::Input};
use std::{hint::black_box, time::Instant};

fn prepared<'a>(spec: &'a Spec, input: &'a [u8]) -> Interpreter<'a, 'a> {
    let mut reader = Interpreter::new(spec, Input::new(input, "input"));
    reader.capture_charges = false;
    reader.execute(spec.parsed.syntax()).unwrap();
    reader.work.set([0; 6]);
    reader
}
fn run_checks(reader: &mut Interpreter<'_, '_>, fails: bool) -> [usize; 6] {
    let checks = reader.spec.analysis.model.checks();
    let mut failed = false;
    for check in checks {
        reader.select_statement(&check.node);
        if let Err(error) = reader.execute_check(&check.node) {
            let Failure::Language(error) = error else {
                panic!("language error")
            };
            assert_eq!(error.code, ErrorCode::CheckFailed);
            assert!(error.record_path().is_some());
            failed = true;
            break;
        }
    }
    assert_eq!(failed, fails);
    reader.work.get()
}
fn work(source: &str, input: &str, fails: bool) -> [usize; 6] {
    let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
    let mut reader = prepared(&spec, input.as_bytes());
    run_checks(&mut reader, fails)
}
fn total(counts: [usize; 6]) -> usize {
    counts.into_iter().sum()
}
fn linear(name: &str, small: [usize; 6], large: [usize; 6]) {
    println!(
        "{name}: 1x={small:?} total={} 4x={large:?} total={}",
        total(small),
        total(large)
    );
    assert!(
        total(large) <= 6 * total(small),
        "{name}: {} > 6 * {}",
        total(large),
        total(small)
    );
}
fn records(n: usize, mode: usize) -> (String, String, bool) {
    let input = vec!["1"; n].join(" ");
    let condition = match mode {
        0 => "x<y && x+x==2",
        1 => "all(r in rows:r.x<r.y && r.x+r.x==2)",
        2 => "all(r in rows:r.x<0)",
        _ => "all(r in rows:r.x>0)",
    };
    (
        format!("repeat {n} as rows {{each row x:int let y=x+1}} if 0 {{check {condition}}}"),
        input,
        mode == 2,
    )
}
fn arrays(n: usize, width: usize, aggregate: bool) -> (String, String) {
    (
        format!(
            "a[{n}][{width}]:int if 0 {{check all(r in a:{})}}",
            if aggregate {
                format!("sum(r)=={width}")
            } else {
                format!("size(r)=={width} && r")
            }
        ),
        vec!["1"; n * width].join(" "),
    )
}
fn nested(depth: usize, values: bool, fail: bool) -> (String, String) {
    let mut source = (0..depth)
        .map(|i| format!("repeat 1 as rows{i} {{each row{i} "))
        .collect::<String>();
    if values {
        source.push_str("x:int");
    }
    source.push_str(&"}".repeat(depth));
    source.push_str(" if 0 {check ");
    for i in 0..depth {
        source.push_str(&format!(
            "all(r{i} in {}:",
            if i == 0 {
                "rows0".into()
            } else {
                format!("r{}.rows{i}", i - 1)
            }
        ));
    }
    source.push(if fail { '0' } else { '1' });
    source.push_str(&")".repeat(depth));
    source.push('}');
    (source, if values { "7".into() } else { String::new() })
}
fn balanced(leaves: usize) -> String {
    let mut parts = vec!["x".to_owned(); leaves];
    while parts.len() > 1 {
        parts = parts
            .chunks(2)
            .map(|p| {
                if p.len() == 1 {
                    p[0].clone()
                } else {
                    format!("({}+{})", p[0], p[1])
                }
            })
            .collect();
    }
    parts.pop().unwrap()
}
fn expressions(leaves: usize) -> String {
    format!(
        "repeat 4 {{let x=1}} if 0 {{check {}=={leaves}}}",
        balanced(leaves)
    )
}
fn checks(count: usize) -> String {
    let unrelated = (0..count)
        .map(|i| format!("let unused{i}=(1+2)*(3+4) "))
        .collect::<String>();
    format!(
        "a[4]:int if 0 {{{unrelated} {} }}",
        "check sum(a)==4 ".repeat(count)
    )
}

#[test]
fn checks_scale_linearly() {
    for mode in 0..4 {
        let (a, ai, af) = records(64, mode);
        let (b, bi, bf) = records(256, mode);
        linear(
            &format!("records mode {mode}"),
            work(&a, &ai, af),
            work(&b, &bi, bf),
        );
    }
    for aggregate in [false, true] {
        let (a, ai) = arrays(64, 16, aggregate);
        let (b, bi) = arrays(256, 16, aggregate);
        linear(
            "domain and copy volume",
            work(&a, &ai, false),
            work(&b, &bi, false),
        );
    }
    linear(
        "expression size",
        work(&expressions(32), "", false),
        work(&expressions(128), "", false),
    );
    let (a, ai) = nested(16, false, true);
    let (b, bi) = nested(64, false, true);
    linear(
        "depth and witness",
        work(&a, &ai, true),
        work(&b, &bi, true),
    );
    let (a, ai) = nested(16, false, false);
    let (b, bi) = nested(64, false, false);
    linear(
        "structural depth",
        work(&a, &ai, false),
        work(&b, &bi, false),
    );
    let markers = |depth| {
        let mut source = (0..depth)
            .map(|i| format!("repeat 1 as rows{i} {{each row{i} "))
            .collect::<String>();
        source.push_str("repeat 0 as ends {}");
        source.push_str(&"}".repeat(depth));
        source.push_str(&format!(" if 0 {{check size(row{}.ends)==0}}", depth - 1));
        source
    };
    linear(
        "cold marker summaries",
        work(&markers(16), "", false),
        work(&markers(64), "", false),
    );
    let source = markers(64);
    let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
    let mut reader = prepared(&spec, b"");
    let cold = run_checks(&mut reader, false);
    reader.work.set([0; 6]);
    let warm = run_checks(&mut reader, false);
    assert!(warm[WorkKind::Metadata as usize] < cold[WorkKind::Metadata as usize]);
    linear(
        "check count",
        work(&checks(32), "1 1 1 1", false),
        work(&checks(128), "1 1 1 1", false),
    );
    let array = |depth| {
        format!(
            "a{}:int if 0 {{check all(r in a:sum(r)==7)}}",
            "[1]".repeat(depth)
        )
    };
    linear(
        "deep value selections",
        work(&array(512), "7", false),
        work(&array(2048), "7", false),
    );
    // Reaching the first row does not copy or scan the remaining domain.
    let (a, ai, af) = records(64, 2);
    let (b, bi, bf) = records(4096, 2);
    assert_eq!(work(&a, &ai, af), work(&b, &bi, bf));
    // A last false row visits the prefix, while a first false row stops immediately.
    for n in [64, 256] {
        let (source, mut input, _) = records(n, 3);
        input.pop();
        input.push('0');
        let last = work(&source, &input, true);
        let (source, input, _) = records(n, 2);
        let first = work(&source, &input, true);
        assert!(last[WorkKind::Dispatch as usize] > n * first[WorkKind::Dispatch as usize] / 2);
    }
}

#[test]
fn expression_hooks_select_views_in_order() {
    for (source, input, expected) in [
        (
            "repeat 2 as rows {x:int} let a=rows[0].x",
            "1 2",
            vec![1, 2],
        ),
        (
            "repeat 2 as rows {x:int} let a=sum(r in rows:r.x)",
            "1 2",
            vec![1, 2, 1, 2],
        ),
        (
            "a[2][2]:int let s=sum(r in a:sum(r))",
            "1 2 3 4",
            vec![3, 2, 3, 2],
        ),
        ("repeat 2 {x:int} check x==x", "1 2", vec![2, 2]),
        (
            "repeat 1 as rows {each row repeat 2 as steps {each step repeat 0 as ends {}}} check size(step.ends)==0",
            "",
            vec![3, 2, 2],
        ),
    ] {
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(input.as_bytes(), "input"));
        reader.execute(spec.parsed.syntax()).unwrap();
        assert_eq!(
            reader
                .charges
                .iter()
                .filter(|c| c.expression)
                .map(|c| c.units)
                .collect::<Vec<_>>(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn borrowed_selection_preserves_roots_and_promoted_columns() {
    let spec = Spec::parse(
        b"repeat 2 as rows {x:num} let y=rows[0].x check all(r in rows:r.x==9007199254740992)",
        "spec",
    )
    .unwrap();
    let data = spec
        .read(
            b"9007199254740993 9007199254740992.0",
            "input",
            &Params::new(),
        )
        .unwrap();
    assert_eq!(
        data.value("x", &[0]).unwrap().as_float().unwrap(),
        9007199254740992.0
    );
    assert_eq!(
        data.value("y", &[]).unwrap().as_float().unwrap(),
        9007199254740992.0
    );
    let spec = Spec::parse(
        b"a[2][0]:word let b=a[1] check size(b)==0 let z=-0.0",
        "spec",
    )
    .unwrap();
    let data = spec.read(b"", "input", &Params::new()).unwrap();
    assert_eq!(data.value("a", &[]).unwrap().depth(), 2);
    assert_eq!(data.value("b", &[]).unwrap().depth(), 1);
    assert!(
        data.value("z", &[])
            .unwrap()
            .as_float()
            .unwrap()
            .is_sign_negative()
    );
}

#[test]
#[ignore = "release doubling measurements; parse and domain setup excluded from evaluator timings"]
fn check_doubling_measurements() {
    fn measure(label: &str, cases: impl IntoIterator<Item = (usize, String, String, bool)>) {
        let mut previous = None;
        for (size, source, input, fails) in cases {
            let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
            // Read/collection is deliberately measured separately from the checks.
            let mut reader = prepared(&spec, input.as_bytes());
            run_checks(&mut reader, fails);
            let iterations = if size >= 2048 { 12 } else { 24 };
            let mut read_times = Vec::new();
            let mut check_times = Vec::new();
            let mut counts = [0; 6];
            for _ in 0..9 {
                let start = Instant::now();
                for _ in 0..iterations {
                    black_box(prepared(&spec, input.as_bytes()));
                }
                read_times.push(start.elapsed().as_nanos() as f64 / iterations as f64);
                let start = Instant::now();
                for _ in 0..iterations {
                    reader.work.set([0; 6]);
                    counts = run_checks(&mut reader, fails);
                    black_box(counts);
                }
                check_times.push(start.elapsed().as_nanos() as f64 / iterations as f64);
            }
            read_times.sort_by(f64::total_cmp);
            check_times.sort_by(f64::total_cmp);
            let read = read_times[4] / 1000.0;
            let check = check_times[4] / 1000.0;
            println!(
                "{label}: size={size} work={counts:?} total={} read_us={read:.3} evaluator_us={check:.3} ratio={:.3}",
                total(counts),
                previous.map_or(0.0, |p| check / p)
            );
            previous = Some(check);
        }
    }
    for form in ["lifted", "records", "array"] {
        for outcome in ["all-true", "last-false", "first-false"] {
            measure(&format!("records-{form}-{outcome}"), [256,512,1024,2048,4096].map(|n| {
                let values=(0..n).map(|i|if (outcome=="last-false"&&i+1==n)||(outcome=="first-false"&&i==0){"0"}else{"1"}).collect::<Vec<_>>();
                if form=="array" {
                    let input=values.iter().map(|v|format!("{v} 2")).collect::<Vec<_>>().join("\n");
                    (n,format!("a[{n}][2]:int if 0 {{check all(r in a:r[0]>0 && r[0]<r[1])}}"),input,outcome!="all-true")
                } else {
                    let condition=if form=="lifted" {"x>0 && x<y"}else{"all(r in rows:r.x>0 && r.x<r.y)"};
                    (n,format!("repeat {n} as rows {{each row x:int let y=x+1}} if 0 {{check {condition}}}"),values.join("\n"),outcome!="all-true")
                }
            }));
        }
    }
    for aggregate in [false, true] {
        measure(
            &format!("array-n-aggregate-{aggregate}"),
            [256, 512, 1024, 2048, 4096].map(|n| {
                let (s, i) = arrays(n, 8, aggregate);
                (n, s, i, false)
            }),
        );
        measure(
            &format!("array-width-aggregate-{aggregate}"),
            [8, 16, 32, 64, 128].map(|w| {
                let (s, i) = arrays(128, w, aggregate);
                (w, s, i, false)
            }),
        );
    }
    for values in [false, true] {
        for fail in [false, true] {
            measure(
                &format!("depth-values-{values}-fail-{fail}"),
                [16, 32, 64, 128].map(|d| {
                    let (s, i) = nested(d, values, fail);
                    (d, s, i, fail)
                }),
            );
        }
    }
    measure(
        "marker-depth",
        [16, 32, 64, 128].map(|depth| {
            let mut source = (0..depth)
                .map(|i| format!("repeat 1 as rows{i} {{each row{i} "))
                .collect::<String>();
            source.push_str("repeat 0 as ends {}");
            source.push_str(&"}".repeat(depth));
            source.push_str(&format!(" if 0 {{check size(row{}.ends)==0}}", depth - 1));
            (depth, source, String::new(), false)
        }),
    );
    measure(
        "array-depth",
        [512, 1024, 2048, 4096].map(|d| {
            (
                d,
                format!(
                    "a{}:int if 0 {{check all(r in a:sum(r)==7)}}",
                    "[1]".repeat(d)
                ),
                "7".into(),
                false,
            )
        }),
    );
    measure(
        "expression-leaves",
        [32, 64, 128, 256, 512].map(|n| (n, expressions(n), String::new(), false)),
    );
    measure(
        "checks",
        [64, 128, 256, 512, 1024].map(|n| (n, checks(n), "1 1 1 1".into(), false)),
    );
}

#[test]
fn invalid_arguments_stop_before_their_charge() {
    for (source, code, expected) in [
        (
            "repeat 1 as rows {x:int} let v=rows[0.0].x",
            ErrorCode::IndexType,
            vec![],
        ),
        (
            "let v=distinct(1,1,\"s\")",
            ErrorCode::FunctionType,
            vec![1, 1],
        ),
        (
            "let v=sum(9223372036854775807,1)",
            ErrorCode::IntegerOverflow,
            vec![1, 1],
        ),
    ] {
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"1", "input"));
        let Failure::Language(error) = reader.execute(spec.parsed.syntax()).unwrap_err() else {
            panic!("language error")
        };
        assert_eq!(error.code, code);
        assert_eq!(error.record_path(), None);
        assert_eq!(
            reader
                .charges
                .iter()
                .filter(|c| c.expression)
                .map(|c| c.units)
                .collect::<Vec<_>>(),
            expected
        );
    }
    for source in [
        "repeat 1 {a[0]:int b[0]:int} check 1||a||a",
        "repeat 1 {a[0]:int b[0]:int} check 1||a||b",
        "repeat 1 as rows {each row a[0]:int b[0]:int} check 1||row.a||row.b",
        "repeat 1 as rows {each row x:int let a=row.x}",
    ] {
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let mut reader = Interpreter::new(&spec, Input::new(b"1", "input"));
        reader.execute(spec.parsed.syntax()).unwrap();
        let expected = if source.contains("x:int") {
            vec![]
        } else if source.contains("row.a") {
            vec![2]
        } else if source.ends_with("a||b") {
            vec![1, 1]
        } else {
            vec![1]
        };
        assert_eq!(
            reader
                .charges
                .iter()
                .filter(|c| c.expression)
                .map(|c| c.units)
                .collect::<Vec<_>>(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn ordinary_field_and_bound_array_witness_labels() {
    for (condition, message, path) in [
        (
            "all(v in row.a:v)",
            "check failed for element 2 of 'row.a' in record 2 of 'rows'",
            vec![1, 1],
        ),
        (
            "all(v in (row).a:v)",
            "check failed for element 2 of 'row.a' in record 2 of 'rows'",
            vec![1, 1],
        ),
        (
            "all(v in rows[1].a:v)",
            "check failed for element 2 of the array",
            vec![1],
        ),
        (
            "all(r in a:all(v in r:v))",
            "check failed for element 2 of 'r' in record 2 of 'rows'",
            vec![1, 1],
        ),
    ] {
        let source = format!("repeat 2 as rows {{each row a[2]:int}} check {condition}");
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let Failure::Language(error) = spec.read(b"1 1 1 0", "input", &Params::new()).unwrap_err()
        else {
            panic!("language error")
        };
        assert_eq!(error.message(), message);
        assert_eq!(error.record_path(), Some(path.as_slice()));
    }
}
