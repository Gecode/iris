//! A handle leaving a repeat already is the collected array; inside the repeat
//! the same handle means the current record.
use gecode_iris::build::{D1, Field, Int, SpecBuilder, int, lit};
use gecode_iris::Params;

fn main() {
    let mut b = SpecBuilder::unnamed();
    let x: Field<Int, D1> = b.repeat(2, |r| {
        let x = r.read("x", int());
        r.check(x.ge(lit(0)));
        x
    });
    let spec = b.build().unwrap();
    let data = spec.read(b"4 5", "input", &Params::new()).unwrap();
    let collected: &[i64] = data.get(x);
    assert_eq!(collected, &[4, 5]);
}
