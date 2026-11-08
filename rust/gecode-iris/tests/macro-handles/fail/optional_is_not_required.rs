use gecode_iris::{Params, iris};

fn main() {
    let labelled = iris!("n:int if n > 0 { label:word }");
    let data = labelled.spec.read(b"1 w", "input.txt", &Params::new()).unwrap();
    let _: &str = data.get(labelled.label);
}
