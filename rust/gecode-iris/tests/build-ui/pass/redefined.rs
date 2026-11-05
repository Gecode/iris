//! A redefinition gives a dynamic handle; `set` keeps the static type.
use gecode_iris::build::{DynField, SpecBuilder, float, int, lit};
use gecode_iris::{Params, ValueRef};

fn main() {
    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", int());
    let redefined: DynField = b.read(x, float());
    let y = b.read("y", int());
    b.set(y, lit(9));
    let spec = b.build().unwrap();
    let data = spec.read(b"1 2.5 3", "input", &Params::new()).unwrap();
    let value: ValueRef<'_> = data.get(redefined).unwrap();
    assert_eq!(value.as_float().unwrap(), 2.5);
    assert_eq!(data.get(y), 9);
}
