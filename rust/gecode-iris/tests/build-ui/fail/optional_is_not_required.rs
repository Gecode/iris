use gecode_iris::build::{SpecBuilder, int};
use gecode_iris::Params;

fn main() {
    let mut b = SpecBuilder::unnamed();
    let n = b.read("n", int());
    let x = b.if_(n, |t| t.read("x", int())).finish();
    let spec = b.build().unwrap();
    let data = spec.read(b"1 2", "input", &Params::new()).unwrap();
    let _: i64 = data.get(x);
}
