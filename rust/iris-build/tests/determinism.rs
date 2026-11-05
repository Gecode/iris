//! The same calls give the same text, whatever else the process has done:
//! internal names come from stream order, never from a global counter.
use gecode_iris_build::{SpecBuilder, int, lit, pattern, sum_over};
use std::thread;

fn program() -> String {
    let mut b = SpecBuilder::new("determinism");
    let n = b.capture("n", lit(1)..);
    b.line(pattern!("{n}"));
    let (record, x) = b.repeat(n, |r| (r.record(), r.read("x", int())));
    b.check(
        sum_over(record, |a| {
            sum_over(record, |b| a.get(x).dynamic() * b.get(x))
        })
        .ge(0),
    );
    b.source().unwrap()
}

#[test]
fn the_same_program_prints_the_same_text() {
    assert_eq!(program(), program());
}

#[test]
fn concurrent_builders_print_the_same_text() {
    let expected = program();
    let threads: Vec<_> = (0..2)
        .map(|_| thread::spawn(|| (0..500).map(|_| program()).collect::<Vec<_>>()))
        .collect();
    for thread in threads {
        for text in thread.join().unwrap() {
            assert_eq!(text, expected);
        }
    }
}
