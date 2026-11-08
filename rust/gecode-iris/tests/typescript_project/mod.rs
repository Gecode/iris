//! What the tests that check generated TypeScript share: a directory outside
//! the repository whose `src` files import `gecode-iris` from `typescript/`
//! through a link and are checked with oxfmt, oxlint and tsc as `typescript/`
//! configures them, then run with Node. It needs Node 22.18 or later, `npm ci`
//! and `npm run build` in `typescript/`.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// The most files that one run of oxfmt or oxlint checks.
const BATCH: usize = 500;

/// A directory of TypeScript files that import `gecode-iris` from `package`.
pub struct TypeScriptProject {
    pub root: PathBuf,
    pub package: PathBuf,
}

impl TypeScriptProject {
    /// The project `name` in Cargo's test directory, with the directories
    /// `src` and `others` emptied of files of an earlier run.
    pub fn new(name: &str, others: &[&str]) -> Self {
        let repo = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
        let package = repo.join("typescript");
        assert!(
            package.join("dist/build/node.js").exists() && package.join("node_modules").exists(),
            "run `npm ci` and `npm run build` in {} first",
            package.display()
        );
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
        for directory in std::iter::once(&"src").chain(others) {
            let path = root.join(directory);
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path).unwrap();
        }
        fs::write(root.join("package.json"), "{ \"type\": \"module\" }\n").unwrap();
        let tsconfig = format!(
            "{{\n  \"extends\": {},\n  \"compilerOptions\": {{ \"typeRoots\": [{}] }},\n  \
             \"include\": [\"src\"]\n}}\n",
            json_string(&package.join("tsconfig.json")),
            json_string(&package.join("node_modules/@types")),
        );
        fs::write(root.join("tsconfig.json"), tsconfig).unwrap();
        let modules = root.join("node_modules");
        fs::create_dir_all(&modules).unwrap();
        let link = modules.join("gecode-iris");
        if fs::symlink_metadata(&link).is_ok() {
            fs::remove_file(&link)
                .or_else(|_| fs::remove_dir(&link))
                .unwrap();
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&package, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&package, &link).unwrap();
        Self { root, package }
    }

    /// The file `src/NAME.ts`.
    pub fn source(&self, name: &str) -> PathBuf {
        self.root.join("src").join(format!("{name}.ts"))
    }

    /// Check that oxfmt would leave `files` as they are and that oxlint finds
    /// nothing in them, as `typescript/` configures both. They skip what git
    /// ignores, as the target directory, in a directory they walk, so they
    /// get the files by name, in batches that a command line holds; oxlint
    /// runs in the package to find its type-aware linter there.
    pub fn check_style(&self, files: &[PathBuf]) {
        let tsconfig = self.root.join("tsconfig.json");
        for (batch, files) in files.chunks(BATCH).enumerate() {
            let mut oxfmt = self.tool("oxfmt");
            oxfmt
                .arg("--check")
                .arg("-c")
                .arg(self.package.join(".oxfmtrc.json"))
                .args(files);
            self.expect_success(&format!("oxfmt-{batch}"), oxfmt);
            let mut oxlint = self.tool("oxlint");
            oxlint
                .args(["--type-aware", "--deny-warnings", "-c"])
                .arg(self.package.join(".oxlintrc.json"))
                .arg("--tsconfig")
                .arg(&tsconfig)
                .args(files);
            self.expect_success(&format!("oxlint-{batch}"), oxlint);
        }
    }

    /// Type-check every file in `src` in one tsc run.
    pub fn check_types(&self) {
        let mut tsc = self.tool("tsc");
        tsc.arg("-p")
            .arg(self.root.join("tsconfig.json"))
            .arg("--noEmit");
        self.expect_success("tsc", tsc);
    }

    /// Run `script` with Node and `args`, in the package, and return what it
    /// prints; its output is in `NAME.log`.
    pub fn node(&self, name: &str, script: &Path, args: &[&Path]) -> String {
        let mut node = Command::new("node");
        node.current_dir(&self.package).arg(script).args(args);
        self.expect_success(name, node)
    }

    /// The executable `name` that `npm ci` installs in the package, to run
    /// there.
    fn tool(&self, name: &str) -> Command {
        let bin = self.package.join("node_modules/.bin");
        let path = if cfg!(windows) {
            bin.join(format!("{name}.cmd"))
        } else {
            bin.join(name)
        };
        let mut command = Command::new(path);
        command.current_dir(&self.package);
        command
    }

    /// Run `command`, with its output in `NAME.log`, which a failure prints,
    /// and return its standard output.
    fn expect_success(&self, name: &str, mut command: Command) -> String {
        let log = self.root.join(format!("{name}.log"));
        let output = command
            .output()
            .unwrap_or_else(|error| panic!("cannot run {name}: {error}"));
        let mut text = output.stdout.clone();
        text.extend_from_slice(&output.stderr);
        fs::write(&log, &text).unwrap();
        assert!(
            output.status.success(),
            "{name} failed (log in {}):\n{}",
            log.display(),
            String::from_utf8_lossy(&text)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }
}

/// `path` as a JSON string.
fn json_string(path: &Path) -> String {
    let mut text = String::from("\"");
    for character in path.to_string_lossy().chars() {
        match character {
            '"' | '\\' => {
                text.push('\\');
                text.push(character);
            }
            character => text.push(character),
        }
    }
    text.push('"');
    text
}
