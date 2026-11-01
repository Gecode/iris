use iris_conformance::{
    RunOptions,
    case::{self, Case},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};
#[test]
fn implemented_conformance() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let paths = case::discover(
        &[repo.join("conformance/cases/0.1"), repo.join("formats")],
        None,
    )
    .unwrap();
    let discovered: HashSet<_> = paths.iter().cloned().collect();
    let mut pending = HashMap::new();
    let mut owners = BTreeMap::new();
    for line in include_str!("pending-cases.txt").lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        assert_eq!(fields.len(), 2, "invalid pending line: {line}");
        assert!(
            matches!(
                fields[1],
                "rust-005" | "rust-006" | "rust-007" | "rust-008" | "rust-009" | "rust-010"
            ),
            "invalid owner: {line}"
        );
        let path = repo.join(fields[0]);
        assert!(
            discovered.contains(&path),
            "pending case not discovered: {line}"
        );
        assert!(
            pending.insert(path, fields[1]).is_none(),
            "duplicate pending case: {line}"
        );
        *owners.entry(fields[1]).or_insert(0usize) += 1;
    }
    let mut operations = BTreeMap::new();
    let mut roots = vec![];
    for path in &paths {
        let case = Case::load(path, "0.1").unwrap();
        if !pending.contains_key(path) {
            let operation = case.operation.as_str();
            *operations.entry(operation).or_insert(0usize) += 1;
            roots.push(path.clone());
        }
    }
    assert!(!roots.is_empty());
    let selected = roots.len();
    println!(
        "Discovered {}; selected {selected} {operations:?}; pending {} {owners:?}",
        paths.len(),
        pending.len()
    );
    let report = iris_conformance::run(&RunOptions {
        driver: format!("\"{}\"", env!("CARGO_BIN_EXE_iris-driver")),
        driver_id: None,
        roots,
        filter: None,
        version: "0.1".into(),
        timeout: Duration::from_secs(30),
    })
    .unwrap();
    println!("{}", report.render());
    assert_eq!(report.cases.len(), selected);
    assert_eq!(report.exit_code(), 0);
}
