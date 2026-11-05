use gecode_iris::build::{SpecBuilder, int, lit};

fn main() {
    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    b.set(x, lit("text"));
}
