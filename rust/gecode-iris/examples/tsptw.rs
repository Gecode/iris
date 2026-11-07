use gecode_iris::build::{BuildError, D1, D2, Field, Int, SpecBuilder, gecode, lit, ty};
use gecode_iris::{Params, Spec};

/// A reader for the travelling salesman problem with time windows: a distance
/// matrix and a time window for each node, with the depot as node 0.
pub struct Tsptw {
    pub spec: Spec,
    pub n: Field<Int>,
    pub dist: Field<Int, D2>,
    pub ready: Field<Int, D1>,
    pub due: Field<Int, D1>,
}

pub fn tsptw() -> Result<Tsptw, BuildError> {
    let mut b = SpecBuilder::new("tsptw");
    b.comment("#");
    let n = b.read("n", 2..=1000);
    let dist = b.read("dist", ty(lit(0)..=gecode::INT_MAX / 2 / n).dim(n).dim(n));

    let time = b.define_type("time", lit(0)..=gecode::INT_MAX / 2);
    let (node, ready, due) = b.repeat(n, |node| {
        node.as_name("nodes");
        (
            node.record(),
            node.read("ready", time),
            node.read("due", time),
        )
    });

    b.check(node.get(ready).le(node.get(due)))
        .message("the time window of a node is invalid");
    b.check(node.at(0).get(ready).eq(0))
        .message("the depot (node 0) must open at time 0");
    Ok(Tsptw {
        spec: b.build()?,
        n,
        dist,
        ready,
        due,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tsptw = tsptw()?;
    let path = "formats/tsptw/cases/three-nodes/input.txt";
    let data = tsptw.spec.read_path(path, &Params::new())?;
    let n: i64 = data.get(tsptw.n);
    println!("{n} nodes");
    let windows = data.get(tsptw.ready).iter().zip(data.get(tsptw.due));
    for (node, (distances, (ready, due))) in data.get(tsptw.dist).rows().zip(windows).enumerate() {
        println!("node {node}: open {ready}..{due}, distances {distances:?}");
    }
    Ok(())
}
