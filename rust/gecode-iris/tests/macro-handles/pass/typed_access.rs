//! Each kind of handle field, read with the type `Data::get` documents.
use gecode_iris::build::{Array, D1, Int, Number};
use gecode_iris::{Params, ValueRef, iris};

fn main() {
    let grid = iris!(r#"
        n:int
        scale:num
        row[n]:float
        repeat n { cells[2]:int label:word }
        if n > 2 { note:word }
        x:int x:word  # redefined with another value type
        spec:int      # beside the field `spec`
    "#);
    let input = b"2 1.5 0.5 0.25 1 2 a 3 4 b 7 w 9";
    let data = grid.spec.read(input, "input.txt", &Params::new()).unwrap();
    let n: i64 = data.get(grid.n);
    let scale: Number = data.get(grid.scale);
    let row: &[f64] = data.get(grid.row);
    let cells: Array<'_, Int, D1> = data.get(grid.cells);
    let labels: &[String] = data.get(grid.label);
    let note: Option<&str> = data.get(grid.note);
    let x: Option<ValueRef<'_>> = data.get(grid.x);
    let entry_spec: i64 = data.get(grid.spec_2);
    assert_eq!(n, 2);
    assert_eq!(scale, Number::Float(1.5));
    assert_eq!(row, &[0.5, 0.25]);
    assert_eq!(cells.row(1), &[3, 4]);
    assert_eq!(labels, ["a", "b"]);
    assert_eq!(note, None);
    assert_eq!(x.unwrap().as_str().unwrap(), "w");
    assert_eq!(entry_spec, 9);
}
