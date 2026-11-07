use gecode_iris::build::{BuildError, D2, Field, Int, SpecBuilder, gecode, lit, pattern, ty};
use gecode_iris::{Params, Spec};

/// A DIMACS CNF reader: the number of variables and one row of literals per
/// clause.
pub struct DimacsCnf {
    pub spec: Spec,
    pub vars: Field<Int>,
    pub clause: Field<Int, D2>,
}

pub fn dimacs_cnf() -> Result<DimacsCnf, BuildError> {
    let mut b = SpecBuilder::new("dimacs-cnf");
    b.comment("c");
    b.stop("%"); // SATLIB files end with "%" and "0"
    let vars = b.capture("vars", lit(0)..=gecode::INT_MAX.ex());
    let clauses = b.capture("clauses", 0..);
    b.line(pattern!("p cnf {vars} {clauses}"));
    let clause = b.repeat(clauses, |c| {
        // The terminating 0 is not stored.
        c.list("clause", ty(-vars..=vars.ex()).until(0))
    });
    Ok(DimacsCnf {
        spec: b.build()?,
        vars,
        clause,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cnf = dimacs_cnf()?;
    let path = "formats/dimacs-cnf/cases/three-variables/input.txt";
    let data = cnf.spec.read_path(path, &Params::new())?;
    let vars: i64 = data.get(cnf.vars);
    println!("{vars} variables");
    for clause in data.get(cnf.clause).rows() {
        println!("{clause:?}");
    }
    Ok(())
}
