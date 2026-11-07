use gecode_iris::build::{
    Arms, BuildError, D0, D1, D2, Field, Float, Int, Maybe, Record, SpecBuilder, Str, defined,
    float, gecode, int, lit, pattern, rest, size, ty, word,
};
use gecode_iris::{Params, Spec};

/// A TSPLIB reader. A file gives either an explicit distance matrix or node
/// coordinates, from which the distances follow.
pub struct Tsplib {
    pub spec: Spec,
    pub name: Field<Str, D0, Maybe>,
    pub n: Field<Int>,
    pub edge_weight_type: Field<Str, D0, Maybe>,
    pub weight: Field<Int, D2, Maybe>,
    pub x: Field<Float, D1, Maybe>,
    pub y: Field<Float, D1, Maybe>,
}

/// The entries of one section of numbered coordinates.
struct Coordinates {
    nodes: Record,
    x: Field<Float, D1, Maybe>,
    y: Field<Float, D1, Maybe>,
}

/// Adds the optional section `title` of `n` lines `node x y`, with the
/// entries named `{prefix}node`, `{prefix}x` and `{prefix}y`.
#[track_caller]
fn coordinate_section(arms: &mut Arms, title: &str, n: Field<Int>, prefix: &str) -> Coordinates {
    arms.arm(title).optional().body(|a| {
        a.repeat(n, |line| {
            line.read(format!("{prefix}node"), int());
            Coordinates {
                nodes: line.record(),
                x: line.read(format!("{prefix}x"), float()),
                y: line.read(format!("{prefix}y"), float()),
            }
        })
    })
}

pub fn tsplib() -> Result<Tsplib, BuildError> {
    let mut b = SpecBuilder::new("tsplib");
    b.comment("COMMENT");
    let name = b.optional_capture("name", rest());
    let problem_type = b.optional_capture("type", word());
    let n = b.capture("n", 2..=46340);
    let edge_weight_type = b.optional_capture("edge_weight_type", word());
    let edge_weight_format = b.optional_capture("edge_weight_format", word());
    let node_coord_type = b.optional_capture("node_coord_type", word());
    let display_data_type = b.optional_capture("display_data_type", word());
    let (nodes, x, y, weight) = b.sections().until("EOF").body(|s| {
        s.arm(pattern!("NAME : {name}")).optional();
        s.arm(pattern!("TYPE : {problem_type}")).optional();
        s.arm(pattern!("DIMENSION : {n}"));
        s.arm(pattern!("EDGE_WEIGHT_TYPE : {edge_weight_type}"))
            .optional();
        s.arm(pattern!("EDGE_WEIGHT_FORMAT : {edge_weight_format}"))
            .optional();
        s.arm(pattern!("NODE_COORD_TYPE : {node_coord_type}"))
            .optional();
        s.arm(pattern!("DISPLAY_DATA_TYPE : {display_data_type}"))
            .optional();
        let Coordinates { nodes, x, y } = coordinate_section(s, "NODE_COORD_SECTION", n, "");
        let weight = s.arm("EDGE_WEIGHT_SECTION").optional().body(|a| {
            a.if_(!defined(edge_weight_format), |t| {
                t.set(edge_weight_format, lit("FULL_MATRIX"));
            });
            let bound = gecode::INT_MAX / n;
            a.read(
                "weight",
                ty(lit(0)..=bound).dim(n).dim(n).layout(edge_weight_format),
            )
        });
        coordinate_section(s, "DISPLAY_DATA_SECTION", n, "display_");
        (nodes, x, y, weight)
    });
    b.if_(!defined(edge_weight_type), |t| {
        t.set(edge_weight_type, lit("EXPLICIT"));
    });

    b.if_(edge_weight_type.eq("EXPLICIT"), |t| {
        t.check(defined(weight)).message("no EDGE_WEIGHT_SECTION");
        t.check(size(weight).eq(n))
            .message("edge weights do not match DIMENSION");
    })
    .else_(|e| {
        let is = |weight_type: &str| edge_weight_type.eq(weight_type);
        e.check(is("EUC_2D") | is("CEIL_2D") | is("ATT") | is("GEO"))
            .message("EDGE_WEIGHT_TYPE must be EXPLICIT, EUC_2D, CEIL_2D, ATT, or GEO");
        e.check(defined(x)).message("no NODE_COORD_SECTION");
        e.check(size(nodes).eq(n))
            .message("coordinates do not match DIMENSION");
        e.check(x.ge(-1e9) & x.le(1e9) & y.ge(-1e9) & y.le(1e9))
            .message("coordinates must be between -1e9 and 1e9");
    });
    Ok(Tsplib {
        spec: b.build()?,
        name,
        n,
        edge_weight_type,
        weight,
        x,
        y,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tsplib = tsplib()?;
    for case in ["explicit-five", "euc-2d-five"] {
        let path = format!("formats/tsplib/cases/{case}/input.txt");
        let data = tsplib.spec.read_path(&path, &Params::new())?;
        let name: Option<&str> = data.get(tsplib.name);
        let n: i64 = data.get(tsplib.n);
        let weight_type: Option<&str> = data.get(tsplib.edge_weight_type);
        println!(
            "{}: {n} cities, {}",
            name.unwrap_or("unnamed"),
            weight_type.unwrap_or("?")
        );
        if let Some(weight) = data.get(tsplib.weight) {
            for row in weight.rows() {
                println!("  {row:?}");
            }
        }
        if let (Some(x), Some(y)) = (data.get(tsplib.x), data.get(tsplib.y)) {
            println!("  x {x:?}\n  y {y:?}");
        }
    }
    Ok(())
}
