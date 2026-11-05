use gecode_iris::build::{SpecBuilder, int, lit};

fn main() {
    let mut b = SpecBuilder::unnamed();
    let row = b.read("row", int().dim(2));
    b.set(row, lit(9));
}
