use gecode_iris::{Params, iris};

fn main() {
    let pair = iris!("a:int b:word");
    let data = pair.spec.read(b"1 w", "input.txt", &Params::new()).unwrap();
    let _: &str = data.get(pair.a);
}
