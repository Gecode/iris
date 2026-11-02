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
