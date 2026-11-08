//! The expansion compiles under a crate's strictest lint levels, with handle
//! fields left unused: it allows no lint itself, which `forbid` would refuse.
#![forbid(dead_code)]
#![deny(warnings, unused)]
use gecode_iris::{Params, iris};

fn main() {
    let unread = iris!("x:int y:float");
    let read = iris!("n:int");
    let data = read.spec.read(b"4", "input.txt", &Params::new()).unwrap();
    let n: i64 = data.get(read.n);
    assert_eq!(n, 4);
    assert_eq!(unread.spec.format_name(), None);
}
