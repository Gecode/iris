mod support;
use iris_syntax::{LanguageVersion, parse};
use std::{collections::BTreeSet, ffi::OsStr, fs, path::PathBuf, process::Command};

#[test]
fn repository_formatting() {
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
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .collect();
    let mut valid = 0;
    let mut invalid_utf8 = 0;
    let mut syntax_errors = 0;
    for path in &paths {
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStrExt;
            OsStr::from_bytes(path)
        };
        #[cfg(not(unix))]
        let path = OsStr::new(std::str::from_utf8(path).unwrap());
        let source = fs::read(repository.join(path)).unwrap();
        let Ok(text) = std::str::from_utf8(&source) else {
            invalid_utf8 += 1;
            continue;
        };
        let parsed = parse(&source, &path.to_string_lossy(), LanguageVersion::V0_1).unwrap();
        if !parsed.diagnostics().is_empty() {
            syntax_errors += 1;
            continue;
        }
        valid += 1;
        for width in [40, 80, 100] {
            support::checked(text, width);
        }
    }
    println!(
        "repository formatting: total={}, syntax-clean checked={valid}, invalid UTF-8 excluded={invalid_utf8}, syntax errors excluded={syntax_errors}, width checks={}, failures=0",
        paths.len(),
        valid * 3
    );
}
