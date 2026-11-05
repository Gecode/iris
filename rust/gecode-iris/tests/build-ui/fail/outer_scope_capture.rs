use gecode_iris::build::{SpecBuilder, Scope, int};

fn add(s: &mut Scope) {
    s.repeat(1, |_| {
        s.read("x", int());
    });
}

fn main() {
    add(&mut SpecBuilder::unnamed());
}
