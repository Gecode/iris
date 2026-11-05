//! A definition that may not execute gives an optional view of its depth.
use gecode_iris::build::{SpecBuilder, int, word};
use gecode_iris::Params;

fn main() {
    let mut b = SpecBuilder::unnamed();
    let n = b.read("n", int());
    let (scalar, row) = b.if_(n, |t| (t.read("scalar", word()), t.read("row", int().dim(2)))).finish();
    let otherwise = b.if_(n, |_| {}).else_(|e| e.read("otherwise", int())).1;
    let spec = b.build().unwrap();
    let data = spec.read(b"1 w 2 3", "input", &Params::new()).unwrap();
    let present: Option<&str> = data.get(scalar);
    let ints: Option<&[i64]> = data.get(row);
    let absent: Option<i64> = data.get(otherwise);
    assert_eq!(present, Some("w"));
    assert_eq!(ints, Some(&[2, 3][..]));
    assert_eq!(absent, None);
}
