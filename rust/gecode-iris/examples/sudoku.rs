use gecode_iris::build::{BuildError, D2, Field, Int, SpecBuilder, alphabet, max, pattern};
use gecode_iris::{Params, Spec};

/// A Sudoku reader: the grid size, the box size, and the cells, with 0 for a
/// blank.
pub struct Sudoku {
    pub spec: Spec,
    pub n: Field<Int>,
    pub box_width: Field<Int>,
    pub box_height: Field<Int>,
    pub cells: Field<Int, D2>,
}

/// Cell values from 1 to 36, so `1` reads as 1 and `0` as 36.
const SYMBOLS: &str = "123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ0";

pub fn sudoku() -> Result<Sudoku, BuildError> {
    let mut b = SpecBuilder::new("sudoku");
    let (n, box_width, box_height, cells) = b.instances(|i| {
        let n = i.capture("n", 1..);
        let box_width = i.capture("box_width", 1..);
        let box_height = i.capture("box_height", 1..);
        i.line(pattern!("# size={n} box={box_width}x{box_height}"));
        let cells = i.read("cells", alphabet(SYMBOLS).blank(".").dim(n).dim(n));
        i.check((box_width * box_height).eq(n))
            .message("boxes must tile the grid");
        i.check(max(cells).le(n))
            .message("a value is larger than the size of the grid");
        (n, box_width, box_height, cells)
    });
    Ok(Sudoku {
        spec: b.build()?,
        n,
        box_width,
        box_height,
        cells,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sudoku = sudoku()?;
    let path = "formats/sudoku/cases/four-by-four/input.txt";
    let data = sudoku.spec.read_path(path, &Params::new())?;
    let n: i64 = data.get(sudoku.n);
    let (width, height): (i64, i64) = (data.get(sudoku.box_width), data.get(sudoku.box_height));
    println!("{n}x{n} grid with {width}x{height} boxes");
    for row in data.get(sudoku.cells).rows() {
        println!("{row:?}");
    }
    Ok(())
}
