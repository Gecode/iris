//! Depth 2 and 3 views, and dimensions on top of repeats.
use gecode_iris::build::{Array, D1, D2, Int, SpecBuilder, int};
use gecode_iris::Params;

fn main() {
    let mut b = SpecBuilder::unnamed();
    let matrix = b.read("matrix", int().dim(2).dim(2));
    let cube = b.repeat(1, |r| r.read("cube", int().dim(2).dim(1)));
    let spec = b.build().unwrap();
    let data = spec
        .read(b"1 2 3 4 5 6", "input", &Params::new())
        .unwrap();
    let rows: Array<'_, Int, D1> = data.get(matrix);
    let row: &[i64] = rows.row(1);
    let planes: Array<'_, Int, D2> = data.get(cube);
    let plane: Array<'_, Int, D1> = planes.row(0);
    let line: &[i64] = plane.row(1);
    assert_eq!(row, &[3, 4]);
    assert_eq!(line, &[6]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.rows().count(), 2);
    assert!(rows.get(2).is_none());
}
