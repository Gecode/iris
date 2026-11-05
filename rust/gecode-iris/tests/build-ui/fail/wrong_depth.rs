use gecode_iris::build::{SpecBuilder, int};
use gecode_iris::Params;

fn main() {
    let mut b = SpecBuilder::unnamed();
    let row = b.read("row", int().dim(2));
    let spec = b.build().unwrap();
    let data = spec.read(b"1 2", "input", &Params::new()).unwrap();
    let _: i64 = data.get(row);
}
