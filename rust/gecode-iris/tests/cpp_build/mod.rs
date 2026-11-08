//! What the tests that build C++ programs against `cpp/` share: a CMake project
//! outside the repository, built with the compiler and flags that the
//! environment chooses, as the CTest tests that run them pass them on.
//!
//! The programs build with CMake (`IRIS_CMAKE`, by default `cmake`) and the
//! compiler that CMake finds, so `CXX` and `CMAKE_GENERATOR` choose them.
//! `IRIS_CXX_FLAGS` and `IRIS_EXE_LINKER_FLAGS`, if set, replace the compile and
//! link flags of the build, including those of its configuration.
use std::{
    ffi::OsString,
    fmt::Write as _,
    path::{Path, PathBuf},
    process::Command,
    thread,
};

/// The environment variables that replace the build's flags, with the cache
/// variables they set.
const FLAGS: [(&str, &str); 2] = [
    ("IRIS_CXX_FLAGS", "CMAKE_CXX_FLAGS"),
    ("IRIS_EXE_LINKER_FLAGS", "CMAKE_EXE_LINKER_FLAGS"),
];

/// A CMake project in `root` whose programs link the library of `repo/cpp`
/// with its warnings as errors.
pub struct CmakeProject {
    pub root: PathBuf,
    pub repo: PathBuf,
}

impl CmakeProject {
    /// The start of the project's `CMakeLists.txt`, which declares the
    /// function `program(target sources...)`. A shard of many programs can
    /// hold more sections than an MSVC object file does without `/bigobj`.
    pub fn lists_head(&self, project: &str) -> String {
        let mut lists = String::new();
        writeln!(
            lists,
            "cmake_minimum_required(VERSION 3.28)\n\
             project({project} LANGUAGES CXX)\n\
             set(CMAKE_CXX_SCAN_FOR_MODULES OFF)\n\
             set(IRIS_BUILD_TESTS OFF CACHE BOOL \"\" FORCE)\n\
             set(IRIS_INSTALL OFF CACHE BOOL \"\" FORCE)\n\
             add_subdirectory(\"{}\" iris)\n\
             function(program target)\n  \
               add_executable(${{target}} ${{ARGN}})\n  \
               target_link_libraries(${{target}} PRIVATE iris::iris)\n  \
               set_target_properties(${{target}} PROPERTIES CXX_EXTENSIONS OFF)\n  \
               iris_set_warnings(${{target}})\n  \
               if(MSVC)\n    \
                 target_compile_options(${{target}} PRIVATE /bigobj)\n  \
               endif()\n\
             endfunction()",
            cmake_path(&self.repo.join("cpp"))
        )
        .unwrap();
        lists
    }

    /// Configure and build the project.
    pub fn build(&self) {
        let cmake = std::env::var_os("IRIS_CMAKE").unwrap_or_else(|| "cmake".into());
        let build = self.build_directory();
        let mut configure = Command::new(&cmake);
        configure
            .arg("-S")
            .arg(&self.root)
            .arg("-B")
            .arg(&build)
            .args([
                "-DCMAKE_BUILD_TYPE=Debug",
                "-DCMAKE_COMPILE_WARNING_AS_ERROR=ON",
            ]);
        // Given flags stand in for the configuration's own as well.
        for (variable, cache) in FLAGS {
            if let Some(flags) = std::env::var_os(variable) {
                let mut flag = OsString::from(format!("-D{cache}="));
                flag.push(flags);
                configure.arg(flag).arg(format!("-D{cache}_DEBUG="));
            }
        }
        expect_success(configure, "cmake configure");
        let mut compile = Command::new(&cmake);
        compile
            .arg("--build")
            .arg(&build)
            .args(["--config", "Debug", "--parallel"]);
        if let Ok(threads) = thread::available_parallelism() {
            compile.arg(threads.to_string());
        }
        expect_success(compile, "cmake --build");
    }

    /// The build directory of the compiler, generator, CMake and flags that
    /// the environment chooses, since CMake keeps the compiler of a
    /// directory's first configuration, and flags that are no longer given
    /// would stay in its cache. CMake reads `CXXFLAGS` and `LDFLAGS` only
    /// at a directory's first configuration too.
    fn build_directory(&self) -> PathBuf {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let variables = [
            "CXX",
            "CXXFLAGS",
            "LDFLAGS",
            "CMAKE_GENERATOR",
            "IRIS_CMAKE",
        ]
        .into_iter()
        .chain(FLAGS.map(|(variable, _)| variable));
        for variable in variables {
            // An unset variable differs from an empty one: no flags at all
            // are not CMake's defaults.
            let value = std::env::var_os(variable);
            let bytes = value
                .as_ref()
                .map_or(&[][..], |value| value.as_encoded_bytes());
            let end = if value.is_some() { 0 } else { 1 };
            for &byte in bytes.iter().chain([&end]) {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
        self.root.join(format!("build-{hash:016x}"))
    }

    /// The program that `target` builds.
    pub fn executable(&self, target: &str) -> PathBuf {
        let build = self.build_directory();
        let name = format!("{target}{}", std::env::consts::EXE_SUFFIX);
        // Multi-configuration generators put programs in a directory per
        // configuration.
        [build.join(&name), build.join("Debug").join(&name)]
            .into_iter()
            .find(|path| path.exists())
            .unwrap_or_else(|| panic!("no program {name} in {}", build.display()))
    }
}

/// `text` with the characters that a regular expression treats specially
/// escaped.
pub fn regex_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if "\\^$.|?*+()[]{}".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// A path as CMake reads it, with forward slashes.
pub fn cmake_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// The repository's root as an absolute path that CMake and the compilers
/// take: on Windows, without the `\\?\` prefix of a canonical path.
pub fn repository() -> PathBuf {
    let canonical =
        std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    match canonical
        .to_str()
        .and_then(|text| text.strip_prefix(r"\\?\"))
    {
        Some(local) if !local.starts_with(r"UNC\") => PathBuf::from(local),
        _ => canonical,
    }
}

/// Runs `command` and returns its standard output, or fails with both outputs.
pub fn expect_success(mut command: Command, name: &str) -> String {
    let output = command.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{name} failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}
