//! A parameter handle sets its parameter, and the data has it. A parameter
//! keeps the scope's depth when its default is a scalar there, including a
//! handle of depth 0 outside every repeat, since a caller's value is a scalar
//! too.
use gecode_iris::build::{D1, Dyn, DynDepth, Field, Int, Num, SpecBuilder, Str, int, num, word};
use gecode_iris::{Params, iris};

/// A compile error unless both handles have one type.
fn same_handle<H>(_: H, _: H) {}

fn main() {
    let mut b = SpecBuilder::unnamed();
    let scale = b.param("scale");
    let n = b.read("n", int());
    let x = b.read("x", num());
    let w = b.read("w", word());
    let row = b.read("row", int().dim(2));
    let limit: Field<Int> = b.param_default("limit", n);
    let number: Field<Num> = b.param_default("number", x);
    let label: Field<Str> = b.param_default("label", w);
    let next: Field<Int> = b.param_default("next", n + 1);
    let _whole: Field<Int, DynDepth> = b.param_default("whole", row);
    let _first: Field<Int, DynDepth> = b.param_default("first", row.at(0));
    let _loose: Field<Dyn, DynDepth> = b.param_default("loose", n.dynamic());
    let in_repeat: Field<Int, D1> = b.repeat(1, |r| {
        let _outer: Field<Int, DynDepth> = r.param_default("outer", n);
        r.param_default("each", n + 1)
    });
    let view = iris!("n: int x: num w: word param limit = n param number = x param label = w");
    same_handle(limit, view.limit);
    same_handle(number, view.number);
    same_handle(label, view.label);
    let spec = b.build().unwrap();
    let mut params = Params::new();
    params.set(scale, 3);
    let data = spec.read(b"4 2.5 abc 1 2", "input", &params).unwrap();
    assert_eq!(data.get(scale).as_int().unwrap(), 3);
    let limit: i64 = data.get(limit);
    assert_eq!(limit, 4);
    assert_eq!(data.get(number), gecode_iris::build::Number::Float(2.5));
    assert_eq!(data.get(label), "abc");
    assert_eq!(data.get(next), 5);
    assert_eq!(data.get(in_repeat), &[5]);
}
