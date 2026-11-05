use gecode_iris::build::{SpecBuilder, word};

fn main() {
    let mut b = SpecBuilder::unnamed();
    let x = b.read("x", word());
    b.let_("y", x + 1);
}
