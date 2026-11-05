use gecode_iris::build::{SpecBuilder, int};

fn main() {
    let mut b = SpecBuilder::unnamed();
    let row = b.read("row", int().dim(2));
    let n = b.read("n", int());
    b.set(row, n);
}
