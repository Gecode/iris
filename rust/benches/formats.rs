//! Small-input execution regressions; CLI performance acceptance is separate.
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use iris_core::{Params, Spec};
use std::{
    fs,
    hint::black_box,
    path::{Path, PathBuf},
};

fn directories(path: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(path)
        .expect("format directory")
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.is_dir())
        .collect();
    paths.sort();
    paths
}

fn formats(criterion: &mut Criterion) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../formats");
    let mut group = criterion.benchmark_group("formats");
    for format in directories(&root) {
        let name = format.file_name().unwrap().to_str().unwrap();
        let spec_path = format.join(format!("{name}.iris"));
        let spec = Spec::parse(
            &fs::read(&spec_path).expect("reader spec"),
            spec_path.to_str().unwrap(),
        )
        .expect("valid reader spec");
        for case in directories(&format.join("cases")) {
            let input = case.join("input.txt");
            if !input.is_file() {
                continue;
            }
            let bytes = fs::read(&input).expect("input");
            let mut params = Params::new();
            let params_path = case.join("params.json");
            if params_path.is_file() {
                let values: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_slice(&fs::read(params_path).unwrap())
                        .expect("case parameters");
                for (name, value) in values {
                    params.set(name, value.as_str().expect("string parameter"));
                }
            }
            let result = spec.read(&bytes, "input.txt", &params);
            assert_eq!(
                result.is_ok(),
                case.join("expected.json").is_file(),
                "{}",
                case.display()
            );
            if !case.join("expected.json").is_file() {
                assert!(case.join("error.json").is_file());
            }
            group.throughput(Throughput::Bytes(bytes.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(name, case.file_name().unwrap().to_str().unwrap()),
                &bytes,
                |bencher, bytes| {
                    bencher.iter(|| {
                        black_box(spec.read(black_box(bytes), "input.txt", black_box(&params)))
                    })
                },
            );
        }
    }
    group.finish();
}
criterion_group!(benches, formats);
criterion_main!(benches);
