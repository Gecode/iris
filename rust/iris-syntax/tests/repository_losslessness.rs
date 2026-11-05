use gecode_iris_syntax::{LanguageVersion, parse};
use std::{collections::BTreeSet, ffi::OsStr, fs, path::PathBuf, process::Command};

#[test]
fn repository_losslessness() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("git")
        .current_dir(&repository)
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "*.iris",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let paths: BTreeSet<_> = output
        .stdout
        .split(|&byte| byte == 0)
        .filter(|path| !path.is_empty())
        .collect();
    let mut checked = 0;
    let mut invalid = 0;
    let mut failures = Vec::new();
    for path in &paths {
        // Git paths are NUL-delimited bytes, and need not be UTF-8 on Unix.
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStrExt;
            OsStr::from_bytes(path)
        };
        #[cfg(not(unix))]
        let path = OsStr::new(
            std::str::from_utf8(path).expect("Git path is representable on this platform"),
        );
        let bytes = fs::read(repository.join(path)).unwrap();
        if std::str::from_utf8(&bytes).is_err() {
            invalid += 1;
            continue;
        }
        let result = parse(&bytes, &path.to_string_lossy(), LanguageVersion::V0_1).unwrap();
        checked += 1;
        if result.syntax().text().to_string().as_bytes() != bytes {
            failures.push(path.to_owned());
        }
    }
    println!(
        "repository .iris sources: total={}, valid checked={checked}, invalid UTF-8 excluded={invalid}, reconstruction failures={}",
        paths.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{failures:?}");
}
