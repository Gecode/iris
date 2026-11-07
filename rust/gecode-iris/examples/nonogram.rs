use gecode_iris::build::{
    BuildError, D2, Expr, Field, Int, Record, Scope, SpecBuilder, count, eol, min, pattern, size,
    sum, ty,
};
use gecode_iris::{Params, Spec};

/// A nonogram reader: the board size and the clues of each row and column.
pub struct Nonogram {
    pub spec: Spec,
    pub width: Field<Int>,
    pub height: Field<Int>,
    pub row: Field<Int, D2>,
    pub col: Field<Int, D2>,
}

/// The clue lines of one side of the board.
struct Clues {
    lines: Record,
    clues: Field<Int, D2>,
}

/// Reads `count` lines of comma-separated clues into the entry `name`.
#[track_caller]
fn clue_lines(s: &mut Scope, count: Field<Int>, name: &str) -> Clues {
    s.repeat(count, |line| Clues {
        lines: line.record(),
        clues: line.list(name, ty(0..).until(eol())),
    })
}

/// A 0 clue says the line is empty, so it must stand alone.
fn zero_stands_alone(clues: Field<Int, D2>) -> Expr<Int> {
    count(clues).le(1) | min(clues).gt(0)
}

/// The clues and one gap between each pair of them fit into `length`.
fn clues_fit(clues: Field<Int, D2>, length: Field<Int>) -> Expr<Int> {
    (sum(clues) + count(clues) - 1).le(length)
}

pub fn nonogram() -> Result<Nonogram, BuildError> {
    let mut b = SpecBuilder::new("nonogram");
    b.separators(",");
    let width = b.capture("width", 1..);
    let height = b.capture("height", 1..);
    let (rows, cols) = b.sections().skip_other().body(|s| {
        s.arm(pattern!("width {width}"));
        s.arm(pattern!("height {height}"));
        let rows = s.arm("rows").body(|a| clue_lines(a, height, "row"));
        let cols = s.arm("columns").body(|a| clue_lines(a, width, "col"));
        (rows, cols)
    });
    let (row, col) = (rows.clues, cols.clues);

    b.check(height.le(1_000_000 / width))
        .message("the board must have at most a million fields");
    b.check(size(rows.lines).eq(height) & size(cols.lines).eq(width))
        .message("the numbers of rows and columns differ from the board size");
    b.check(zero_stands_alone(row))
        .message("a 0 must be the only clue of its row");
    b.check(zero_stands_alone(col))
        .message("a 0 must be the only clue of its column");
    b.check(clues_fit(row, width))
        .message("the clues of a row do not fit into the width of the board");
    b.check(clues_fit(col, height))
        .message("the clues of a column do not fit into the height of the board");
    Ok(Nonogram {
        spec: b.build()?,
        width,
        height,
        row,
        col,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nonogram = nonogram()?;
    let path = "formats/nonogram/cases/five-by-five/input.txt";
    let data = nonogram.spec.read_path(path, &Params::new())?;
    let (width, height): (i64, i64) = (data.get(nonogram.width), data.get(nonogram.height));
    println!("{width}x{height} board");
    println!(
        "rows: {:?}",
        data.get(nonogram.row).rows().collect::<Vec<_>>()
    );
    println!(
        "columns: {:?}",
        data.get(nonogram.col).rows().collect::<Vec<_>>()
    );
    Ok(())
}
