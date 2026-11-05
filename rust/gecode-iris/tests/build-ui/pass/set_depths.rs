//! `set` accepts what keeps the depth, and a value whose depth is open.
use gecode_iris::build::{DynField, Field, Int, SpecBuilder, D1, int, lit};
use gecode_iris::Params;

fn main() {
    let mut b = SpecBuilder::unnamed();
    let row = b.read("row", int().dim(2));
    let other = b.read("other", int().dim(2));
    b.set(row, other);
    let x: Field<Int, D1> = b.repeat(1, |r| {
        let x = r.read("x", int());
        r.set(x, lit(3));
        x
    });
    let open: DynField = b.let_(row, lit(1).dynamic());
    b.set(open, lit(2).dynamic());
    let spec = b.build().unwrap();
    let data = spec.read(b"1 2 3 4 5", "input", &Params::new()).unwrap();
    assert_eq!(data.get(x), &[3]);
    assert_eq!(data.get(open).unwrap().as_int().unwrap(), 2);
}
