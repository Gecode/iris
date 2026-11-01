use iris_conformance::{
    RunOptions,
    case::{self, Case},
};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
#[test]
fn full_conformance() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let paths = case::discover(
        &[repo.join("conformance/cases/0.1"), repo.join("formats")],
        None,
    )
    .unwrap();
    let mut operations = BTreeMap::new();
    let mut areas = BTreeMap::new();
    for path in &paths {
        let case = Case::load(path, "0.1").unwrap();
        *operations.entry(case.operation.as_str()).or_insert(0usize) += 1;
        let area = path.parent().unwrap().strip_prefix(&repo).unwrap();
        *areas.entry(area).or_insert(0usize) += 1;
    }
    assert!(!paths.is_empty());
    let count = paths.len();
    println!("Discovered {count} cases {operations:?}");
    println!("Cases by area: {areas:?}");
    let report = iris_conformance::run(&RunOptions {
        driver: format!("\"{}\"", env!("CARGO_BIN_EXE_iris-driver")),
        driver_id: None,
        roots: paths,
        filter: None,
        version: "0.1".into(),
        timeout: Duration::from_secs(30),
    })
    .unwrap();
    println!("{}", report.render());
    assert_eq!(report.cases.len(), count);
    assert_eq!(report.exit_code(), 0);
}
