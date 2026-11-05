//! A parameter handle sets its parameter, and the data has it.
use gecode_iris::build::SpecBuilder;
use gecode_iris::Params;

fn main() {
    let mut b = SpecBuilder::unnamed();
    let scale = b.param("scale");
    let spec = b.build().unwrap();
    let mut params = Params::new();
    params.set(scale, 3);
    let data = spec.read(b"", "input", &params).unwrap();
    assert_eq!(data.get(scale).as_int().unwrap(), 3);
}
