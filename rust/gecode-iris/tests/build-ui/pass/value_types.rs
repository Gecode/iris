//! Each value type at depth 0 and depth 1.
use gecode_iris::build::{Number, Numbers, SpecBuilder, float, int, lit, num, word};
use gecode_iris::{Params, ValueRef};

fn main() {
    let mut b = SpecBuilder::unnamed();
    let i = b.read("i", int());
    let f = b.read("f", float());
    let s = b.read("s", word());
    let n = b.read("n", num());
    let is = b.read("is", int().dim(2));
    let fs = b.read("fs", float().dim(2));
    let ss = b.read("ss", word().dim(2));
    let ns = b.read("ns", num().dim(2));
    let d = b.let_("d", lit(1).dynamic());
    let spec = b.build().unwrap();
    let data = spec
        .read(b"1 2.5 w 3 1 2 1.5 2.5 a b 4 5", "input", &Params::new())
        .unwrap();
    let scalar_int: i64 = data.get(i);
    let scalar_float: f64 = data.get(f);
    let scalar_str: &str = data.get(s);
    let scalar_num: Number = data.get(n);
    let ints: &[i64] = data.get(is);
    let floats: &[f64] = data.get(fs);
    let strs: &[String] = data.get(ss);
    let nums: Numbers<'_> = data.get(ns);
    let dynamic: ValueRef<'_> = data.get(d);
    assert_eq!(scalar_int, 1);
    assert_eq!(scalar_float, 2.5);
    assert_eq!(scalar_str, "w");
    assert_eq!(scalar_num, Number::Int(3));
    assert_eq!(ints, &[1, 2]);
    assert_eq!(floats, &[1.5, 2.5]);
    assert_eq!(strs, &["a".to_owned(), "b".to_owned()]);
    assert_eq!(nums.len(), 2);
    assert_eq!(dynamic.as_int().unwrap(), 1);
}
