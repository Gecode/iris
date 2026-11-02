use super::*;
use crate::Params;

#[test]
fn prepared_bounds_follow_current_frames_and_reads() {
    let spec = Spec::parse(
        b"limit:int repeat 2 {limit:int list xs:-limit..limit until 0}",
        "spec",
    )
    .unwrap();
    for _ in 0..2 {
        assert!(
            spec.read(b"1 2 3 0 3 1 0", "input", &Params::new())
                .is_err()
        );
        let data = spec
            .read(b"1 2 -2 2 0 3 -3 3 0", "input", &Params::new())
            .unwrap();
        assert_eq!(data.ints("xs", &[0]).unwrap(), [-2, 2]);
        assert_eq!(data.ints("xs", &[1]).unwrap(), [-3, 3]);
    }
    let spec = Spec::parse(b"repeat 3 {flag:int if flag {list xs:int until 0}}", "spec").unwrap();
    let data = spec
        .read(b"1 2 0 0 1 3 4 0", "input", &Params::new())
        .unwrap();
    assert_eq!(data.ints("xs", &[0]).unwrap(), [2]);
    assert!(data.ints("xs", &[1]).unwrap().is_empty());
    assert_eq!(data.ints("xs", &[2]).unwrap(), [3, 4]);
}

#[test]
fn prepared_lists_scale_linearly() {
    for malformed in [false, true] {
        for vary_rows in [false, true] {
            let measure = |scale| {
                let rows = if vary_rows { scale } else { 8 };
                let width = if vary_rows { 8 } else { scale };
                let source = format!("let limit=9 repeat {rows} {{list xs:-limit..limit until 0}}");
                let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
                let mut input = format!("{}0 ", "1 ".repeat(width)).repeat(rows);
                if malformed {
                    input.truncate(input.len() - 2);
                    input.push('x');
                }
                let mut reader =
                    Interpreter::new(&spec, Input::new(input.as_bytes(), "input")).unwrap();
                reader.capture_charges = false;
                assert_eq!(reader.execute(spec.parsed.syntax()).is_err(), malformed);
                if !malformed {
                    reader.finish_input().unwrap();
                }
                let work: usize = reader.work.get().iter().sum();
                (work, reader.input.control.test_work())
            };
            let small = measure(128);
            let large = measure(512);
            println!(
                "prepared lists rows={vary_rows} malformed={malformed}: work {small:?} -> {large:?}"
            );
            assert!(large.0 <= 6 * small.0);
            assert!(large.1 <= 6 * small.1);
        }
    }
}

#[test]
fn planned_list_repeat_preserves_charges_and_last_body_site() {
    // The conditional forces the ordinary block continuation with the same
    // leaf effects. Its literal condition has no language-budget charge.
    for (prefix, body, input, params) in [
        ("", "list xs:int until 0", "1 0 2 0", Params::new()),
        (
            "_[2][0]:int",
            "list xs:int until 0",
            "1 0 2 0",
            Params::new(),
        ),
        ("param n=2", "list xs:int until 0", "1 0 2 0", {
            let mut params = Params::new();
            params.set("n", "2");
            params
        }),
        ("", "list xs:1..2 until 0", "1 0 3 0", Params::new()),
    ] {
        let measure = |nested| {
            let body = if nested {
                format!("if 1 {{{body}}}")
            } else {
                body.into()
            };
            let source = format!("{prefix}\nrepeat 2 {{\n{body}\n}}");
            let spec = Spec::parse(source.as_bytes(), "spec").unwrap();
            let mut reader = Interpreter::new(&spec, Input::new(input.as_bytes(), "input"))
                .unwrap()
                .with_params(&params)
                .unwrap();
            let result = reader.execute(spec.parsed.syntax());
            let error = result.err().map(|error| {
                let Failure::Language(error) = error else {
                    panic!("language failure")
                };
                assert_eq!(error.spec_location().unwrap().line, Some(3));
                (
                    error.code,
                    (
                        error.input_location().unwrap().line,
                        error.input_location().unwrap().column,
                    ),
                    error.context().map(str::to_owned),
                )
            });
            (reader.charges, error)
        };
        assert_eq!(measure(false), measure(true));
    }
}

#[test]
fn list_bindings_follow_parent_frames_and_repeat_activations() {
    let spec = Spec::parse(b"let xs=2 repeat 2 {list xs:-xs..xs until 0}", "spec").unwrap();
    for _ in 0..2 {
        assert!(spec.read(b"3 0 1 0", "input", &Params::new()).is_err());
        let data = spec.read(b"-2 0 2 0", "input", &Params::new()).unwrap();
        assert_eq!(data.ints("xs", &[0]).unwrap(), [-2]);
        assert_eq!(data.ints("xs", &[1]).unwrap(), [2]);
    }
    let spec = Spec::parse(
        b"instances {limit:int repeat 2 {list xs:-limit..limit until 0} name:word}",
        "spec",
    )
    .unwrap();
    let input = b"1 1 0 -1 0 first 3 3 0 -3 0 second";
    let mut params = Params::new();
    params.set("instance", "2");
    for _ in 0..2 {
        let data = spec.read(input, "input", &params).unwrap();
        assert_eq!(data.ints("xs", &[0]).unwrap(), [3]);
        assert_eq!(data.ints("xs", &[1]).unwrap(), [-3]);
        assert!(spec.catalog(input, "input", &Params::new()).is_ok());
    }
    let spec = Spec::parse(
        b"repeat 0 {list xs:..limit until limit} let limit=1",
        "spec",
    )
    .unwrap();
    assert!(spec.read(b"", "input", &Params::new()).is_ok());
}
