//! Round-trips every corpus spec that `iris check` accepts through the C++
//! builder: generate the builder program, check it with `clang-format` and,
//! if one is named, `clang-tidy`, compile it with the warnings of `cpp/` as
//! errors, run it, and compare the spec it prints with the original. The
//! layout forms and the limit specs of the Rust round trip go the same way.
//!
//! `IRIS_CLANG_FORMAT` names `clang-format` 22.1.8, and `IRIS_CLANG_TIDY`, if
//! set, `clang-tidy` 22.1.8 (on macOS, export `SDKROOT` for it). The programs
//! build with CMake (`IRIS_CMAKE`, by default `cmake`) and the compiler that
//! CMake finds, so `CXX` and `CMAKE_GENERATOR` choose them. `IRIS_CXX_FLAGS`
//! and `IRIS_EXE_LINKER_FLAGS`, if set, replace the compile and link flags of
//! the build, including those of its configuration, which is how the CTest
//! test `builder-round-trip` passes on a sanitizer's flags. Each program is
//! included into a shard of the build in a namespace of its own, and the first
//! one also builds alone, with its `main`.
mod round_trip;

use gecode_iris_cli::check_spec;
use gecode_iris_gen::{Language, builder_code};
use round_trip::{Tally, corpus, extra_specs, limit_specs};
use std::{
    ffi::OsString,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Instant,
};

/// The number of programs in one shard of the build.
const SHARD: usize = 24;

/// The environment variables that replace the build's flags, with the cache
/// variables they set.
const FLAGS: [(&str, &str); 2] = [
    ("IRIS_CXX_FLAGS", "CMAKE_CXX_FLAGS"),
    ("IRIS_EXE_LINKER_FLAGS", "CMAKE_EXE_LINKER_FLAGS"),
];

/// The headers that every generated program may include, which a shard
/// includes first so that their include guards keep them out of the
/// namespaces.
const HEADERS: &[&str] = &[
    "iris/build.hpp",
    "cstdint",
    "exception",
    "fstream",
    "iostream",
    "string",
    "string_view",
];

#[test]
#[ignore = "needs IRIS_CLANG_FORMAT, CMake and a C++23 compiler"]
fn accepted_specs_round_trip_through_the_cpp_builder() {
    let started = Instant::now();
    let clang_format =
        std::env::var_os("IRIS_CLANG_FORMAT").expect("IRIS_CLANG_FORMAT names clang-format 22.1.8");
    let clang_tidy = std::env::var_os("IRIS_CLANG_TIDY");
    let repo = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let specs = corpus(&repo);
    let project = Project::new(&repo);

    // Each program with the spec it must print.
    let mut generated: Vec<(String, Vec<u8>, String)> = Vec::new();
    let mut skipped = 0;
    for path in &specs {
        let bytes = fs::read(path).unwrap();
        let Ok(checked) = check_spec(&bytes, &path.to_string_lossy()) else {
            skipped += 1;
            continue;
        };
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Cpp)
            .expect("a checked spec lowers");
        let name = format!("spec_{}", generated.len());
        fs::write(project.source(&name), code).unwrap();
        generated.push((name, bytes, path.display().to_string()));
    }
    let compared = generated.len();
    for (name, text) in extra_specs() {
        project.write(&name, &text);
        generated.push((
            name.clone(),
            text.into_bytes(),
            format!("extra spec {name}"),
        ));
    }
    let extra = generated.len() - compared;
    for (name, text) in limit_specs() {
        project.write(name, &text);
        generated.push((
            name.to_owned(),
            text.into_bytes(),
            format!("limit spec {name}"),
        ));
    }
    let names: Vec<&str> = generated.iter().map(|(name, ..)| name.as_str()).collect();
    println!(
        "generated {} programs in {:.1?}",
        names.len(),
        started.elapsed()
    );

    let shards = project.write_shards(&names);
    let step = Instant::now();
    project.format(&clang_format, &names);
    println!("clang-format: clean in {:.1?}", step.elapsed());
    if let Some(clang_tidy) = &clang_tidy {
        let step = Instant::now();
        project.tidy(clang_tidy, shards, names[0]);
        println!("clang-tidy: clean in {:.1?}", step.elapsed());
    }
    let step = Instant::now();
    project.build();
    println!("built {shards} shards in {:.1?}", step.elapsed());
    let step = Instant::now();
    project.run(shards);
    println!("ran them in {:.1?}", step.elapsed());

    let out = project.root.join("out");
    let mut differences = Vec::new();
    for (name, original, title) in &generated {
        if let Err(message) = round_trip::round_trip(&out, original, name) {
            differences.push(format!("{title} ({name}.cpp): {message}"));
        }
    }
    let tally = Tally {
        compared,
        skipped,
        extra,
        corpus: specs.len(),
    };
    println!("wall time {:.1?}", started.elapsed());
    tally.report("C++ builder", &differences);
}

/// A CMake project of generated programs, outside the repository.
struct Project {
    root: PathBuf,
    repo: PathBuf,
}

impl Project {
    fn new(repo: &Path) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cpp-round-trip");
        // Generated files of an earlier run must not survive; the build
        // directory may, so that a second run compiles only what changed.
        for directory in ["src", "out"] {
            let path = root.join(directory);
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path).unwrap();
        }
        Self {
            root,
            repo: repo.to_owned(),
        }
    }

    fn source(&self, name: &str) -> PathBuf {
        self.root.join("src").join(format!("{name}.cpp"))
    }

    /// The program of a spec beyond the corpus, which must pass `iris check`.
    fn write(&self, name: &str, text: &str) {
        let checked = check_spec(text.as_bytes(), name)
            .unwrap_or_else(|error| panic!("{name}: {error}\n{text}"));
        let code = builder_code(&checked.parsed, &checked.analysis, Language::Cpp).unwrap();
        fs::write(self.source(name), code).unwrap();
    }

    /// Run `command` on each of `files` in chunks, one for each thread.
    fn each_chunk(&self, files: &[PathBuf], label: &str, command: impl Fn() -> Command + Sync) {
        let threads = thread::available_parallelism().map_or(4, usize::from);
        let size = files.len().div_ceil(threads).max(1);
        let failures: Vec<String> = thread::scope(|scope| {
            let handles: Vec<_> = files
                .chunks(size)
                .map(|chunk| {
                    let command = &command;
                    scope.spawn(move || {
                        let mut command = command();
                        command.args(chunk);
                        let output = command.output().unwrap();
                        (!output.status.success()).then(|| {
                            format!(
                                "{}{}",
                                String::from_utf8_lossy(&output.stdout),
                                String::from_utf8_lossy(&output.stderr)
                            )
                        })
                    })
                })
                .collect();
            handles
                .into_iter()
                .filter_map(|handle| handle.join().unwrap())
                .collect()
        });
        assert!(
            failures.is_empty(),
            "{label} failed:\n{}",
            failures.join("\n")
        );
    }

    /// Every program is what clang-format writes with `cpp/.clang-format`.
    fn format(&self, clang_format: &OsString, names: &[&str]) {
        let style = format!(
            "--style=file:{}",
            self.repo.join("cpp/.clang-format").display()
        );
        let files: Vec<PathBuf> = names.iter().map(|name| self.source(name)).collect();
        self.each_chunk(&files, "clang-format --dry-run --Werror", || {
            let mut command = Command::new(clang_format);
            command.args([style.as_str(), "--dry-run", "--Werror"]);
            command
        });
    }

    /// Every program passes clang-tidy with `cpp/.clang-tidy`: through the
    /// shards, which include them, and the first program alone, for its
    /// `main`.
    fn tidy(&self, clang_tidy: &OsString, shards: usize, first: &str) {
        let config = format!(
            "--config-file={}",
            self.repo.join("cpp/.clang-tidy").display()
        );
        let include = format!("--extra-arg=-I{}", self.repo.join("cpp/include").display());
        // Diagnostics in the programs that a shard includes, and none in the
        // library's headers.
        let programs = self.root.join("src");
        let filter = format!("--header-filter={}/", regex_escape(&cmake_path(&programs)));
        let mut files: Vec<PathBuf> = (0..shards)
            .map(|index| self.root.join(format!("shard_{index}.cpp")))
            .collect();
        files.push(self.source(first));
        self.each_chunk(&files, "clang-tidy", || {
            let mut command = Command::new(clang_tidy);
            command.args([config.as_str(), filter.as_str(), "--quiet"]);
            command.args(["--extra-arg=-std=c++23", include.as_str()]);
            command
        });
    }

    /// Write the shards and the CMake project that builds them, and return
    /// the number of shards.
    fn write_shards(&self, names: &[&str]) -> usize {
        let mut lists = String::new();
        writeln!(
            lists,
            "cmake_minimum_required(VERSION 3.28)\n\
             project(iris_cpp_round_trip LANGUAGES CXX)\n\
             set(CMAKE_CXX_SCAN_FOR_MODULES OFF)\n\
             set(IRIS_BUILD_TESTS OFF CACHE BOOL \"\" FORCE)\n\
             set(IRIS_INSTALL OFF CACHE BOOL \"\" FORCE)\n\
             add_subdirectory(\"{}\" iris)\n\
             function(program target)\n  \
               add_executable(${{target}} ${{ARGN}})\n  \
               target_link_libraries(${{target}} PRIVATE iris::iris)\n  \
               set_target_properties(${{target}} PROPERTIES CXX_EXTENSIONS OFF)\n  \
               iris_set_warnings(${{target}})\n\
             endfunction()\n\
             program(standalone src/{}.cpp)",
            cmake_path(&self.repo.join("cpp")),
            names[0]
        )
        .unwrap();
        let shards: Vec<&[&str]> = names.chunks(SHARD).collect();
        for (index, shard) in shards.iter().enumerate() {
            fs::write(
                self.root.join(format!("shard_{index}.cpp")),
                shard_source(shard),
            )
            .unwrap();
            writeln!(lists, "program(shard_{index} shard_{index}.cpp)").unwrap();
        }
        fs::write(self.root.join("CMakeLists.txt"), lists).unwrap();
        shards.len()
    }

    /// Configure and build the project, then run the first program alone.
    fn build(&self) {
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

        // The program alone prints the same spec as through its shard.
        let output = Command::new(self.executable("standalone"))
            .output()
            .unwrap();
        assert!(output.status.success(), "the standalone program failed");
        fs::write(self.root.join("out/standalone.iris"), output.stdout).unwrap();
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

    fn executable(&self, target: &str) -> PathBuf {
        let build = self.build_directory();
        let name = format!("{target}{}", std::env::consts::EXE_SUFFIX);
        // Multi-configuration generators put programs in a directory per
        // configuration.
        [build.join(&name), build.join("Debug").join(&name)]
            .into_iter()
            .find(|path| path.exists())
            .unwrap_or_else(|| panic!("no program {name} in {}", build.display()))
    }

    fn run(&self, shards: usize) {
        let out = self.root.join("out");
        thread::scope(|scope| {
            let handles: Vec<_> = (0..shards)
                .map(|index| {
                    let program = self.executable(&format!("shard_{index}"));
                    let out = &out;
                    scope.spawn(move || {
                        let mut command = Command::new(program);
                        command.arg(out);
                        expect_success(command, &format!("shard_{index}"));
                    })
                })
                .collect();
            for handle in handles {
                handle.join().unwrap();
            }
        });
        let first = fs::read(out.join("spec_0.iris")).ok();
        let standalone = fs::read(out.join("standalone.iris")).unwrap();
        assert_eq!(
            first.as_deref(),
            Some(standalone.as_slice()),
            "spec_0 prints differently alone"
        );
    }
}

/// `text` with the characters that a regular expression treats specially
/// escaped.
fn regex_escape(text: &str) -> String {
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
fn cmake_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// A program that includes each named program into a namespace of its own
/// and records what each builder prints, or why it printed nothing.
fn shard_source(names: &[&str]) -> String {
    // clang-tidy checks the programs that the shard includes, not the shard.
    let mut text = String::from("// NOLINTBEGIN\n");
    for header in HEADERS {
        writeln!(text, "#include <{header}>").unwrap();
    }
    text.push('\n');
    for name in names {
        writeln!(text, "namespace {name} {{\n#include \"src/{name}.cpp\"\n}}").unwrap();
    }
    text.push_str(
        "\nnamespace {\n\n\
         template <class F>\n\
         void record(const std::string &out, const std::string &name, F builder) {\n  \
           std::string text;\n  \
           std::string extension = \".iris\";\n  \
           try {\n    \
             text = builder().source();\n  \
           } catch (const std::exception &failure) {\n    \
             text = failure.what();\n    \
             extension = \".error\";\n  \
           }\n  \
           std::ofstream file(out + \"/\" + name + extension, std::ios::binary);\n  \
           file << text;\n\
         }\n\n\
         } // namespace\n\n\
         int main(int argc, char **argv) {\n  \
           if (argc != 2) {\n    \
             return 2;\n  \
           }\n  \
           const std::string out = argv[1];\n",
    );
    for name in names {
        writeln!(text, "  record(out, \"{name}\", {name}::builder);").unwrap();
    }
    text.push_str("  return 0;\n}\n// NOLINTEND\n");
    text
}

fn expect_success(mut command: Command, name: &str) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{name} failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
