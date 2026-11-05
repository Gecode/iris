use gecode_iris_conformance::{
    RunOptions,
    case::{self, Case, Operation},
};
use std::{path::PathBuf, time::Duration};
#[test]
fn parse_only_conformance() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases/0.1");
    let paths = case::discover(&[root], None).unwrap();
    let all = paths.len();
    let roots: Vec<_> = paths
        .into_iter()
        .filter(|path| Case::load(path, "0.1").unwrap().operation == Operation::Parse)
        .collect();
    let count = roots.len();
    let report = gecode_iris_conformance::run(&RunOptions {
        driver: format!("\"{}\"", env!("CARGO_BIN_EXE_iris-driver")),
        driver_id: None,
        roots,
        filter: None,
        skip: Vec::new(),
        version: "0.1".into(),
        timeout: Duration::from_secs(30),
    })
    .unwrap();
    println!(
        "Selected {count} parse-only cases from {all}; excluded {} non-parse cases",
        all - count
    );
    println!("{}", report.render());
    assert_eq!(report.cases.len(), count);
    assert_eq!(report.exit_code(), 0);
}
