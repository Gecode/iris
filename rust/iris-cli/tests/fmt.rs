use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};
use tempfile::TempDir;

const SOURCE: &str = "let total=sum(first_value,second_value,third_value)";
const FLAT: &str = "let total = sum(first_value, second_value, third_value)\n";
const WRAPPED: &str = "let total = sum(\n  first_value,\n  second_value,\n  third_value\n)\n";

fn sandbox() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    // This boundary isolates every lookup from the machine's configuration.
    // Tests may create a child with no .editorconfig of its own.
    fs::write(root.path().join(".editorconfig"), "root = true\n").unwrap();
    root
}

fn fmt(root: &Path, args: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_iris"))
        .arg("fmt")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn assert_status(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn formats_files_and_nested_directory_operands() {
    let root = sandbox();
    let path = root.path();
    fs::create_dir_all(path.join("nested/deeper")).unwrap();
    for name in [
        "a.iris",
        "nested/b.iris",
        "nested/deeper/.iris",
        "explicit.txt",
    ] {
        fs::write(path.join(name), "repeat 2 {x:int}").unwrap();
    }
    for name in ["nested/other.txt", "nested/c.iris.bak", "nested/upper.Iris"] {
        fs::write(path.join(name), "keep me").unwrap();
    }
    fs::write(path.join(".gitignore"), "nested/b.iris\n").unwrap();
    let output = fmt(path, &[".", "explicit.txt"], b"");
    assert_status(&output, 0);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    for name in [
        "a.iris",
        "nested/b.iris",
        "nested/deeper/.iris",
        "explicit.txt",
    ] {
        assert_eq!(
            fs::read_to_string(path.join(name)).unwrap(),
            "repeat 2 {\n  x: int\n}\n"
        );
    }
    assert_eq!(fs::read(path.join("nested/other.txt")).unwrap(), b"keep me");
    assert_eq!(
        fs::read(path.join("nested/c.iris.bak")).unwrap(),
        b"keep me"
    );
    assert_eq!(
        fs::read(path.join("nested/upper.Iris")).unwrap(),
        b"keep me"
    );
    assert_status(&fmt(path, &["--check", "."], b""), 0);
}

#[test]
fn check_and_diff_are_non_writing_and_combine() {
    let root = sandbox();
    let path = root.path();
    fs::write(path.join("spec.iris"), "x:int").unwrap();
    for (args, code) in [
        (vec!["--check", "spec.iris"], 1),
        (vec!["--diff", "spec.iris"], 0),
        (vec!["--check", "--diff", "spec.iris"], 1),
    ] {
        let output = fmt(path, &args, b"");
        assert_status(&output, code);
        if args.contains(&"--diff") {
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                "--- spec.iris\toriginal\n+++ spec.iris\tformatted\n@@ -1 +1 @@\n-x:int\n\\ No newline at end of file\n+x: int\n"
            );
        } else {
            assert!(output.stdout.is_empty());
        }
        assert_eq!(fs::read(path.join("spec.iris")).unwrap(), b"x:int");
    }
    fs::write(path.join("spec.iris"), "x: int\n").unwrap();
    let output = fmt(path, &["--check", "--diff", "spec.iris"], b"");
    assert_status(&output, 0);
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

#[test]
fn editorconfig_nested_globs_overrides_and_root() {
    let root = sandbox();
    let path = root.path();
    fs::write(path.join(".editorconfig"), "root = true\n[*.iris]\nmax_line_length = 40\nindent_style = tab\nindent_size = 8\nend_of_line = crlf\ninsert_final_newline = false\n").unwrap();
    fs::create_dir_all(path.join("nested/deeper")).unwrap();
    fs::create_dir(path.join("boundary")).unwrap();
    fs::write(
        path.join("nested/.editorconfig"),
        "[special.iris]\nmax_line_length = off\n[unset.iris]\nmax_line_length = unset\n",
    )
    .unwrap();
    fs::write(path.join("boundary/.editorconfig"), "root = true\n").unwrap();
    for name in [
        "top.iris",
        "nested/deeper/inner.iris",
        "nested/special.iris",
        "nested/unset.iris",
        "boundary/spec.iris",
    ] {
        fs::write(path.join(name), SOURCE).unwrap();
    }
    assert_status(&fmt(path, &["."], b""), 0);
    for name in ["top.iris", "nested/deeper/inner.iris"] {
        assert_eq!(fs::read_to_string(path.join(name)).unwrap(), WRAPPED);
    }
    for name in [
        "nested/special.iris",
        "nested/unset.iris",
        "boundary/spec.iris",
    ] {
        assert_eq!(fs::read_to_string(path.join(name)).unwrap(), FLAT);
    }
}

#[test]
fn no_editorconfig_uses_default_width() {
    let root = sandbox();
    let path = root.path().join("unconfigured");
    fs::create_dir(&path).unwrap();
    // No .editorconfig in the operand directory. The parent only stops lookup;
    // it supplies no properties, so configuration above the tempdir cannot leak.
    fs::write(path.join("spec.iris"), SOURCE).unwrap();
    assert_status(&fmt(&path, &["spec.iris"], b""), 0);
    assert_eq!(fs::read_to_string(path.join("spec.iris")).unwrap(), FLAT);
}

#[test]
fn stdin_prints_formatted_source_and_uses_only_the_named_paths_config() {
    let root = sandbox();
    let path = root.path();
    fs::write(
        path.join(".editorconfig"),
        "root = true\n[*.iris]\nmax_line_length = 40\n",
    )
    .unwrap();
    let output = fmt(path, &["-"], SOURCE.as_bytes());
    assert_status(&output, 0);
    assert_eq!(output.stdout, FLAT.as_bytes());
    let output = fmt(
        path,
        &["--stdin-filepath", "new.iris", "-"],
        SOURCE.as_bytes(),
    );
    assert_status(&output, 0);
    assert_eq!(output.stdout, WRAPPED.as_bytes());
    assert!(!path.join("new.iris").exists());
    let output = fmt(path, &["--check", "-"], b"x:int");
    assert_status(&output, 1);
    assert!(output.stdout.is_empty());
    assert_status(&fmt(path, &["--check", "-"], FLAT.as_bytes()), 0);
    let output = fmt(
        path,
        &["--diff", "--check", "--stdin-filepath", "new.iris", "-"],
        b"x:int",
    );
    assert_status(&output, 1);
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("--- new.iris\toriginal\n+++ new.iris\tformatted\n")
    );
    assert_status(&fmt(path, &["--diff", "-"], b"x:int"), 0);
}

#[test]
fn source_and_io_failures_leave_bad_files_untouched_and_continue() {
    let root = sandbox();
    let path = root.path();
    let invalid = b"# valid first line\n# \xff";
    fs::write(path.join("bad.iris"), b"let x=@").unwrap();
    fs::write(path.join("utf8.iris"), invalid).unwrap();
    fs::write(path.join("good.iris"), b"x:int").unwrap();
    let output = fmt(
        path,
        &["missing.iris", "bad.iris", "utf8.iris", "good.iris"],
        b"",
    );
    assert_status(&output, 1);
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    for message in [
        "missing.iris:",
        "bad.iris:1:",
        "unexpected character '@'",
        "utf8.iris:2:3:",
        "UTF-8",
    ] {
        assert!(stderr.contains(message), "{stderr}");
    }
    assert_eq!(fs::read(path.join("bad.iris")).unwrap(), b"let x=@");
    assert_eq!(fs::read(path.join("utf8.iris")).unwrap(), invalid);
    assert_eq!(fs::read(path.join("good.iris")).unwrap(), b"x: int\n");
    for bytes in [b"let x=@".as_slice(), invalid.as_slice()] {
        let output = fmt(path, &["-"], bytes);
        assert_status(&output, 1);
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn rejects_invalid_usage_and_invalid_width_without_writing() {
    let root = sandbox();
    let path = root.path();
    for args in [
        vec![],
        vec!["-", "spec.iris"],
        vec!["-", "-"],
        vec!["--stdin-filepath", "spec.iris", "spec.iris"],
    ] {
        assert_status(&fmt(path, &args, b""), 2);
    }
    fs::write(path.join("spec.iris"), SOURCE).unwrap();
    for width in ["0", "-1", "narrow"] {
        fs::write(
            path.join(".editorconfig"),
            format!("root = true\n[*.iris]\nmax_line_length = {width}\n"),
        )
        .unwrap();
        let output = fmt(path, &["spec.iris"], b"");
        assert_status(&output, 1);
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("max_line_length")
        );
        assert_eq!(fs::read(path.join("spec.iris")).unwrap(), SOURCE.as_bytes());
    }
}

#[cfg(unix)]
#[test]
fn skips_file_and_directory_symlinks_including_operands() {
    use std::os::unix::fs::symlink;
    let root = sandbox();
    let outside = sandbox();
    let path = root.path();
    fs::write(outside.path().join("spec.iris"), b"x:int").unwrap();
    symlink(outside.path(), path.join("linked")).unwrap();
    symlink(outside.path().join("spec.iris"), path.join("linked.iris")).unwrap();
    symlink(path, path.join("loop")).unwrap();
    assert_status(&fmt(path, &[".", "linked", "linked.iris"], b""), 0);
    assert_eq!(
        fs::read(outside.path().join("spec.iris")).unwrap(),
        b"x:int"
    );
}

#[test]
fn diff_applies_to_names_with_spaces() {
    let root = sandbox();
    let path = root.path();
    fs::write(path.join("name with spaces.iris"), "x:int").unwrap();
    let output = fmt(path, &["--diff", "name with spaces.iris"], b"");
    assert_status(&output, 0);
    assert!(output.stdout.starts_with(
        b"--- name with spaces.iris\toriginal\n+++ name with spaces.iris\tformatted\n"
    ));
    let Ok(mut patch) = Command::new("patch")
        .args(["--batch", "-p0"])
        .current_dir(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
    else {
        return;
    };
    patch
        .stdin
        .take()
        .unwrap()
        .write_all(&output.stdout)
        .unwrap();
    assert!(patch.wait().unwrap().success());
    assert_eq!(
        fs::read(path.join("name with spaces.iris")).unwrap(),
        b"x: int\n"
    );
}

#[cfg(unix)]
#[test]
fn writes_replace_files_and_keep_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let root = sandbox();
    let path = root.path();
    let spec = path.join("spec.iris");
    fs::write(&spec, "x:int").unwrap();
    fs::set_permissions(&spec, fs::Permissions::from_mode(0o640)).unwrap();
    assert_status(&fmt(path, &["spec.iris"], b""), 0);
    assert_eq!(fs::read(&spec).unwrap(), b"x: int\n");
    assert_eq!(
        fs::metadata(&spec).unwrap().permissions().mode() & 0o777,
        0o640
    );

    // A directory that refuses the temporary file makes the write fail
    // before the original is touched.
    let locked = path.join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(locked.join("spec.iris"), "x:int").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::File::create(locked.join("probe")).is_ok() {
        // Privileged users ignore directory permissions.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let output = fmt(path, &["locked/spec.iris"], b"");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_status(&output, 1);
    assert_eq!(fs::read(locked.join("spec.iris")).unwrap(), b"x:int");
    assert_eq!(fs::read_dir(&locked).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn unreadable_editorconfig_fails_without_writing() {
    use std::os::unix::fs::PermissionsExt;
    let root = sandbox();
    let path = root.path();
    fs::create_dir(path.join("nested")).unwrap();
    let config = path.join("nested/.editorconfig");
    fs::write(&config, "[*]\nmax_line_length = 40\n").unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read(&config).is_ok() {
        // Privileged users ignore file permissions.
        return;
    }
    fs::write(path.join("nested/spec.iris"), "x:int").unwrap();
    let output = fmt(path, &["nested/spec.iris"], b"");
    assert_status(&output, 1);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("cannot read .editorconfig"), "{stderr}");
    assert_eq!(fs::read(path.join("nested/spec.iris")).unwrap(), b"x:int");
}

#[cfg(unix)]
#[test]
fn temporary_name_collision_keeps_the_existing_file() {
    let root = sandbox();
    let path = root.path();
    fs::write(path.join("spec.iris"), "x:int").unwrap();
    // exec keeps the shell's process ID, which names the temporary file.
    let output = Command::new("/bin/sh")
        .args([
            "-c",
            r#"printf sentinel > ".spec.iris.iris-fmt-$$.tmp"; exec "$0" fmt spec.iris"#,
            env!("CARGO_BIN_EXE_iris"),
        ])
        .current_dir(path)
        .output()
        .unwrap();
    assert_status(&output, 1);
    assert_eq!(fs::read(path.join("spec.iris")).unwrap(), b"x:int");
    let collisions: Vec<_> = fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|entry| {
            entry
                .extension()
                .is_some_and(|extension| extension == "tmp")
        })
        .collect();
    assert_eq!(collisions.len(), 1);
    assert_eq!(fs::read(&collisions[0]).unwrap(), b"sentinel");
}
