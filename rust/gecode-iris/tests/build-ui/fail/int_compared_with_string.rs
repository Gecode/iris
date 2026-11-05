use gecode_iris::build::{SpecBuilder, int};

fn main() {
    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    b.check(x.eq("one"));
}
