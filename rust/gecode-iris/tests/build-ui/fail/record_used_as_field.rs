use gecode_iris::build::{SpecBuilder, int};

fn main() {
    let mut b = SpecBuilder::unnamed();
    let job = b.repeat(1, |r| {
        r.read("x", int());
        r.record()
    });
    b.let_("y", job);
}
