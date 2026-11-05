use gecode_iris::build::SpecBuilder;

fn main() {
    let mut b = SpecBuilder::unnamed();
    b.repeat_until_eof().store("x");
}
