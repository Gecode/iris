//! Typed access through handles: empty arrays, absence, and the errors a
//! handle declared by hand can still meet.
use gecode_iris_build::{
    D1, D2, Field, Float, Int, Maybe, Num, Numbers, SpecBuilder, Str, float, int, num, word,
};
use gecode_iris_core::{AccessError, Data, Params};

fn read(build: impl FnOnce(&mut SpecBuilder), input: &[u8]) -> Data {
    let mut b = SpecBuilder::unnamed();
    build(&mut b);
    b.build()
        .unwrap()
        .read(input, "input", &Params::new())
        .unwrap()
}

#[test]
fn an_empty_array_has_the_typed_empty_view() {
    let (mut ints, mut floats, mut strs, mut nums) = (None, None, None, None);
    let mut rows = None;
    let data = read(
        |b| {
            ints = Some(b.read("ints", int().dim(0)));
            floats = Some(b.read("floats", float().dim(0)));
            strs = Some(b.read("strs", word().dim(0)));
            nums = Some(b.read("nums", num().dim(0)));
            rows = Some(b.read("rows", float().dim(0).dim(3)));
        },
        b"",
    );
    assert!(data.get(ints.unwrap()).is_empty());
    assert!(data.get(floats.unwrap()).is_empty());
    assert!(data.get(strs.unwrap()).is_empty());
    assert!(matches!(
        data.get(nums.unwrap()),
        Numbers::Ints(&[]) | Numbers::Floats(&[])
    ));
    assert!(data.get(rows.unwrap()).is_empty());
}

#[test]
fn a_repeat_of_nothing_collects_an_empty_array() {
    let mut x = None;
    let data = read(
        |b| {
            x = Some(b.repeat(0, |r| r.read("x", word())));
        },
        b"",
    );
    let x: Field<Str, D1> = x.unwrap();
    assert!(data.get(x).is_empty());
}

#[test]
fn rows_walk_one_level_at_a_time() {
    let mut x = None;
    let data = read(
        |b| {
            x = Some(b.read("x", float().dim(2).dim(2)));
        },
        b"1 2 3 4",
    );
    let x: Field<Float, D1> = Field::declared("nothing");
    assert!(data.try_get(x).is_err());
    let x: Field<Float, D2> = Field::declared("x");
    let rows: Vec<&[f64]> = data.get(x).rows().collect();
    assert_eq!(rows, [&[1.0, 2.0][..], &[3.0, 4.0][..]]);
}

#[test]
fn an_absent_optional_entry_is_none() {
    let mut x = None;
    let data = read(
        |b| {
            let n = b.read("n", int());
            x = Some(b.if_(n, |t| t.read("x", int())).finish());
        },
        b"0",
    );
    assert_eq!(data.get(x.unwrap()), None);
}

#[test]
fn numbers_keep_the_type_the_data_gave() {
    let mut row = None;
    let data = read(
        |b| {
            row = Some(b.read("row", num().dim(2)));
        },
        b"1 2",
    );
    let row: Field<Num, D1> = row.unwrap();
    assert!(matches!(data.get(row), Numbers::Ints(&[1, 2])));
}

#[test]
fn a_declared_handle_reports_what_went_wrong() {
    let data = read(
        |b| {
            b.read("x", int());
        },
        b"1",
    );
    const MISSING: Field<Int> = Field::declared("missing");
    const WRONG_TYPE: Field<Str> = Field::declared("x");
    const WRONG_DEPTH: Field<Int, D1> = Field::declared("x");
    const ABSENT: Field<Int, gecode_iris_build::D0, Maybe> = Field::declared("missing");
    assert!(matches!(
        data.try_get(MISSING),
        Err(AccessError::UnknownEntry(_))
    ));
    assert!(data.try_get(WRONG_TYPE).is_err());
    assert!(data.try_get(WRONG_DEPTH).is_err());
    assert_eq!(data.try_get(ABSENT).unwrap(), None);
}

#[test]
#[should_panic(expected = "try_get")]
fn get_panics_with_a_pointer_to_try_get() {
    let data = read(|_| {}, b"");
    const MISSING: Field<Int> = Field::declared("missing");
    let _ = data.get(MISSING);
}
