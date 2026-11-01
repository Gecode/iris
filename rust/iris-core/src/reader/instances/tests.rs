use super::*;
use crate::Params;
use std::{hint::black_box, time::Instant};

fn execute(spec: &Spec, input: &[u8], params: &Params, catalog: bool) -> [usize; 6] {
    let mut reader = Interpreter::new(spec, Input::new(input, "input")).with_params(params);
    reader.capture_charges = false;
    if catalog {
        reader.catalog = Some(Catalog::default());
    }
    reader.execute(spec.parsed.syntax()).unwrap();
    reader.finish_input().unwrap();
    reader.work.get()
}
fn instances(count: usize, width: usize) -> String {
    (0..count)
        .map(|i| format!("{} n{i}\n", vec!["1"; width].join(" ")))
        .collect()
}
fn linear(label: &str, small: [usize; 6], large: [usize; 6]) {
    let a: usize = small.iter().sum();
    let b: usize = large.iter().sum();
    println!("{label}: 1x={small:?} total={a}; 4x={large:?} total={b}");
    assert!(b <= 6 * a, "{label}: {b} > 6 * {a}");
}
#[test]
fn instances_scale_linearly() {
    for catalog in [false, true] {
        for by_name in [false, true] {
            let spec = Spec::parse(
                b"instances {repeat 2 as rows {x:int} check all(r in rows:r.x) name:word}",
                "spec",
            )
            .unwrap();
            let run = |n: usize| {
                let mut params = Params::new();
                params.set(
                    "instance",
                    if by_name {
                        format!("n{}", n - 1)
                    } else {
                        n.to_string()
                    },
                );
                execute(&spec, instances(n, 2).as_bytes(), &params, catalog)
            };
            linear(
                &format!("instances catalog={catalog} name={by_name}"),
                run(64),
                run(256),
            );
        }
    }
    // Deferred diagnostics on a single long physical line must not rescan it.
    let spec = Spec::parse(b"instances {x:int check 0}", "spec").unwrap();
    linear(
        "deferred rejections",
        execute(&spec, "1 ".repeat(64).as_bytes(), &Params::new(), true),
        execute(&spec, "1 ".repeat(256).as_bytes(), &Params::new(), true),
    );
    let run_width = |width| {
        let spec = Spec::parse(
            format!("instances {{a[{width}]:int name:word}}").as_bytes(),
            "spec",
        )
        .unwrap();
        execute(&spec, instances(8, width).as_bytes(), &Params::new(), true)
    };
    linear("instance size", run_width(64), run_width(256));
    let run_params = |n| {
        let source = (0..n).map(|i| format!("param p{i}=1 ")).collect::<String>();
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let mut params = Params::new();
        for i in 0..n {
            params.set(format!("p{i}"), "2");
        }
        execute(&spec, b"", &params, false)
    };
    linear("params", run_params(64), run_params(256));
}
#[test]
fn spec_executions_share_no_state() {
    let spec = Spec::parse(b"param p=1 instances {x:int check x>0 name:word}", "spec").unwrap();
    let mut params = Params::new();
    params.set("instance", "a");
    params.set("p", "7");
    assert!(spec.read(b"0 a", "input", &params).is_err());
    let valid = spec.read(b"2 a", "input", &params).unwrap();
    assert_eq!(valid.int("p").unwrap(), 7);
    assert_eq!(valid.int("x").unwrap(), 2);
    let catalog = spec.catalog(b"0 a 3 b", "input", &params).unwrap();
    assert_eq!(catalog.int("instances").unwrap(), 2);
    let valid = spec.read(b"4 b", "input", &Params::new()).unwrap();
    assert_eq!(valid.int("p").unwrap(), 1);
    assert_eq!(valid.int("x").unwrap(), 4);
    let mut pairs = Params::new();
    pairs.set("first", "old");
    pairs.set("second", "2");
    pairs.set("first", "new");
    assert_eq!(
        pairs.pairs(),
        &[
            ("first".into(), "new".into()),
            ("second".into(), "2".into())
        ]
    );
}
#[test]
fn instance_record_metadata_is_local() {
    let spec = Spec::parse(b"instances {n:int if n {repeat n as rows {each row x:int}} check !n || all(r in rows:r.x) name:word}", "spec").unwrap();
    let mut params = Params::new();
    params.set("instance", "b");
    assert_eq!(
        spec.read(b"2 1 1 a 1 2 b", "input", &params)
            .unwrap()
            .int("n")
            .unwrap(),
        1
    );
    assert_eq!(
        spec.catalog(b"2 1 1 a 0 b", "input", &Params::new())
            .unwrap()
            .int("instances")
            .unwrap(),
        2
    );
}
#[test]
#[ignore = "release timing evidence; run with --release --ignored --nocapture"]
fn instance_doubling_measurements() {
    fn measure(
        label: &str,
        size: usize,
        source: String,
        input: String,
        params: Params,
        catalog: bool,
    ) {
        let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
        let counts = execute(&spec, input.as_bytes(), &params, catalog);
        let mut samples = vec![];
        for _ in 0..7 {
            let start = Instant::now();
            for _ in 0..12 {
                black_box(
                    if catalog {
                        spec.catalog(input.as_bytes(), "input", &params)
                    } else {
                        spec.read(input.as_bytes(), "input", &params)
                    }
                    .unwrap(),
                );
            }
            samples.push(start.elapsed().as_secs_f64() * 1e6 / 12.0);
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{label}: size={size} work={} us={:.3}",
            counts.iter().sum::<usize>(),
            samples[3]
        );
    }
    for n in [256, 512, 1024, 2048, 4096] {
        for catalog in [false, true] {
            let mut params = Params::new();
            params.set("instance", n.to_string());
            measure(
                if catalog {
                    "instance-count-catalog"
                } else {
                    "instance-count-selection"
                },
                n,
                "instances {x:int check x name:word}".into(),
                instances(n, 1),
                params,
                catalog,
            );
        }
        measure(
            "instance-size",
            n,
            format!("instances {{a[{n}]:int name:word}}"),
            instances(4, n),
            Params::new(),
            true,
        );
        let source = (0..n).map(|i| format!("param p{i}=1 ")).collect::<String>();
        let mut params = Params::new();
        for i in 0..n {
            params.set(format!("p{i}"), "7");
        }
        measure("param-count", n, source, String::new(), params, false);
    }
}
#[test]
fn rejections_compare_names_without_copying() {
    // Every deferred rejection compares the selector with the local name, so
    // copying a long name there makes many checks quadratic.
    let mut frame = Frame::default();
    assert_eq!(name(&frame), Cow::Borrowed(""));
    frame.set("name".into(), Value::string("x".repeat(4096)));
    assert!(matches!(name(&frame), Cow::Borrowed(text) if text.len() == 4096));
}
