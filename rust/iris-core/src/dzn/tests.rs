use super::*;
use crate::{Entry, value::ArrayLevel};

fn data(value: Value) -> Data {
    Data {
        entries: vec![Entry {
            name: "x".into(),
            value,
        }],
    }
}
fn work(data: &Data) -> usize {
    let mut out = io::sink();
    let mut writer = Writer::new(&mut out);
    writer.write_data(data).unwrap();
    writer.work
}
fn deep(depth: usize, empty: bool) -> Data {
    let mut value = if empty {
        Value::empty("int")
    } else {
        Value::int(7)
    };
    value.set_dimensions(&vec![if empty { 0 } else { 1 }; depth]);
    data(value)
}

#[test]
fn deeply_nested_dzn_uses_no_call_stack() {
    // This depth exceeds a recursive writer's stack even in a release build.
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(|| {
            let depth = 65_536;
            for empty in [false, true] {
                let output = deep(depth, empty).to_dzn().unwrap();
                let mut expected = b"% x: ".to_vec();
                expected.extend("array[int] of tuple(".repeat(depth - 1).bytes());
                expected.extend_from_slice(b"array[int] of int");
                expected.extend(std::iter::repeat_n(b')', depth - 1));
                expected.extend_from_slice(b"\nx = [");
                if !empty {
                    expected.extend("([".repeat(depth - 1).bytes());
                    expected.push(b'7');
                    expected.extend("],)".repeat(depth - 1).bytes());
                }
                expected.extend_from_slice(b"];\n");
                assert_eq!(output, expected);
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn dzn_work_scales_linearly() {
    for family in [
        "entries",
        "leaves",
        "floats",
        "bytes",
        "rows",
        "empty rows",
        "depth",
        "empty depth",
    ] {
        let sample = |n| match family {
            "entries" => Data {
                entries: (0..n)
                    .map(|i| Entry {
                        name: format!("x{i}"),
                        value: Value::int(7),
                    })
                    .collect(),
            },
            "leaves" | "floats" => {
                let mut value = Value {
                    leaves: if family == "floats" {
                        LeafStorage::Float(vec![1.2345678901234567; n])
                    } else {
                        LeafStorage::Int(vec![7; n])
                    },
                    levels: vec![],
                };
                value.set_dimensions(&[n]);
                data(value)
            }
            "bytes" => data(Value::string("\0\t\n\"\\é😀".repeat(n))),
            "rows" | "empty rows" => {
                // A ragged entry with n rows: shape detection and traversal
                // must never revisit earlier rows or rebuild earlier output.
                let offsets: Vec<_> = (0..=n)
                    .map(|i| if family == "rows" { i / 2 } else { 0 })
                    .collect();
                data(Value {
                    leaves: LeafStorage::Int(vec![7; offsets[n]]),
                    levels: vec![
                        ArrayLevel {
                            offsets: vec![0, n],
                        },
                        ArrayLevel { offsets },
                    ],
                })
            }
            "depth" => deep(n, false),
            _ => deep(n, true),
        };
        let small = work(&sample(256));
        let large = work(&sample(1024));
        println!("dzn {family}: 256={small}, 1024={large}");
        assert!(large <= 6 * small, "{family}: {small} -> {large}");
    }
}

#[test]
fn dzn_propagates_output_failures() {
    struct Fails {
        remaining: usize,
    }
    impl Write for Fails {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            let count = self.remaining.min(bytes.len());
            self.remaining -= count;
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let document = deep(10, false);
    let length = document.to_dzn().unwrap().len();
    for remaining in 0..length {
        assert_eq!(
            document
                .write_dzn(&mut Fails { remaining })
                .unwrap_err()
                .kind(),
            io::ErrorKind::BrokenPipe
        );
    }
    assert!(Data::default().to_dzn().unwrap().is_empty());
}

#[test]
fn float_precision_gap_and_boundaries() {
    // All 25 positive values and their negatives in appendix C.4.
    for magnitude in (99_999_999_999_999_600u64..100_000_000_000_000_000).step_by(16) {
        for negative in [false, true] {
            let value = magnitude as f64 * if negative { -1.0 } else { 1.0 };
            assert_eq!(
                float::format(value).0,
                format!("{}{magnitude}.0", if negative { "-" } else { "" })
            );
        }
    }
    for (value, expected) in [
        (-0.0, "-0.0"),
        (0.0, "0.0"),
        (20.0, "20.0"),
        (1e16, "10000000000000000.0"),
        (1e17, "1e+17"),
        (1e-4, "0.0001"),
        (1e-5, "1e-05"),
        (f64::from_bits(1), "5e-324"),
        (f64::from_bits(2), "1e-323"),
        (f64::MAX, "1.7976931348623157e+308"),
        (f64::MIN_POSITIVE, "2.2250738585072014e-308"),
    ] {
        assert_eq!(float::format(value).0, expected);
    }
}
