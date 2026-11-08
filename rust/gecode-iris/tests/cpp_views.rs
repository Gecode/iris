//! Generates a C++ view of every format in `formats/`, plus specs that need
//! dynamic handles, deep entries, unusual identifiers or long names, and holds
//! them to what they promise:
//!
//! - one program includes every view, so each compiles with the warnings of
//!   `cpp/` as errors, and pins with `static_assert` the type of
//!   `data.get(handle)` for each handle, as the C++ guide documents it;
//! - the program reads each case and compares what every handle gives with
//!   untyped access, so a handle that promises the wrong value type, depth or
//!   presence fails, since `data.get` checks them, and it checks that every
//!   entry has a handle;
//! - with `IRIS_CLANG_FORMAT`, which names clang-format 22.1.8, every view is
//!   what clang-format writes with `cpp/.clang-format`, and with
//!   `IRIS_CLANG_TIDY`, clang-tidy 22.1.8 accepts every view with
//!   `cpp/.clang-tidy` (on macOS, export `SDKROOT` for it).
//!
//! The program builds as `cpp_build` describes, which is how the CTest test
//! `generated-views` passes on a build's compiler and flags.
mod cpp_build;

use cpp_build::{CmakeProject, cmake_path, expect_success, regex_escape};
use gecode_iris_cli::check_spec;
use gecode_iris_conformance::canonical::Outcome;
use gecode_iris_conformance::case::{Case, Operation, discover};
use gecode_iris_gen::{
    Language,
    view::{self, Access, Leaf, View},
    view_code,
};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// A view header, named like its namespace, and the handles it declares.
struct Header {
    name: String,
    view: View,
    /// The identifier of each handle, in the order of `view.handles`.
    identifiers: Vec<String>,
}

/// One read that the program compares.
struct Read {
    header: String,
    spec: Vec<u8>,
    input: Vec<u8>,
    params: BTreeMap<String, String>,
}

/// Specs that are not formats, with an input. Each exercises a part of the
/// handle list: a name without one shape, an optional entry, and names that C++
/// cannot spell or that a header must not take, such as `int` and `iris`.
const SPECS: &[(&str, &str, &str)] = &[
    (
        "redefined",
        "x:int let x=\"s\" y:int y:float z:word",
        "1 2 2.5 t",
    ),
    (
        "open_shape",
        "n:int param p a[1]:int param q=a repeat n {let r=a}",
        "0 7",
    ),
    (
        "optional",
        "x:int if x {y:float s[1]:word} u:num v[1]:num",
        "0 2.5 1",
    ),
    (
        "spellings",
        "int:int if:int iris:int std:int assert:int errno:int namespace:int b:int \
         count:int e1:int _:int aB:int a_b:int Field:int",
        "1 2 3 4 5 6 7 8 9 10 11 12 13 14",
    ),
];

/// Reaches beyond the widest identifier and past a full line of the literal.
const LONGEST_NAME: usize = 130;

/// A depth above the deepest typed handle of Rust and Python, which C++ has.
fn deep_spec() -> String {
    "repeat 1 {".repeat(9) + "v:int" + &"}".repeat(9)
}

/// Names of every length around the widths where a declaration changes layout,
/// for kinds of handle type from the narrowest to the widest.
fn layout_spec() -> String {
    let mut spec = String::from("c:int\n");
    for length in 1..=LONGEST_NAME {
        let name = "n".repeat(length);
        writeln!(spec, "{name}_i: int").unwrap();
        writeln!(spec, "{name}_f[1][1][1]: float").unwrap();
        writeln!(spec, "if c {{\n{name}_s: word\n}}").unwrap();
        writeln!(spec, "if c {{\n{name}_d[1][1][1][1][1][1][1][1]: num\n}}").unwrap();
        writeln!(spec, "{name}_x: int\nlet {name}_x = \"s\"").unwrap();
    }
    spec
}

#[test]
#[ignore = "needs CMake and a C++23 compiler"]
fn cpp_views_compile_and_read_the_same_data_as_untyped_access() {
    let repo = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap();
    let project = Project::new(&repo);
    let mut headers = Vec::new();
    let mut reads = Vec::new();
    for (name, spec) in format_specs(&repo) {
        headers.push(project.write_view(&name, &spec));
        reads.extend(format_reads(&repo, &name, &spec));
    }
    assert_eq!(headers.len(), 6, "the format library has six formats");
    assert!(reads.len() >= 6, "too few cases: {}", reads.len());
    for (name, spec, input) in SPECS {
        headers.push(project.write_view(name, spec.as_bytes()));
        let params = if *name == "open_shape" {
            BTreeMap::from([("p".to_owned(), "1".to_owned())])
        } else {
            BTreeMap::new()
        };
        reads.push(Read {
            header: (*name).to_owned(),
            spec: spec.as_bytes().to_vec(),
            input: input.as_bytes().to_vec(),
            params,
        });
    }
    for (name, spec, input) in [
        ("deep", deep_spec(), "1".to_owned()),
        (
            "widths",
            layout_spec(),
            format!("1 {}", "1 1.5 w 1.5 1 ".repeat(LONGEST_NAME)),
        ),
    ] {
        headers.push(project.write_view(name, spec.as_bytes()));
        reads.push(Read {
            header: name.to_owned(),
            spec: spec.into_bytes(),
            input: input.into_bytes(),
            params: BTreeMap::new(),
        });
    }

    if let Some(clang_format) = std::env::var_os("IRIS_CLANG_FORMAT") {
        project.format(&clang_format, &headers);
        println!("clang-format: {} views clean", headers.len());
    }
    project.write_program(&headers, &reads);
    if let Some(clang_tidy) = std::env::var_os("IRIS_CLANG_TIDY") {
        project.tidy(&clang_tidy);
        println!("clang-tidy: {} views clean", headers.len());
    }
    project.cmake.build();
    let summary = project.compare();
    println!("C++ views: {} views, {summary}", headers.len());
    assert!(
        summary.starts_with(&format!("{} reads", reads.len())),
        "{summary}"
    );
}

/// Each `<format>/<format>.iris`, by format name.
fn format_specs(repo: &Path) -> Vec<(String, Vec<u8>)> {
    let mut specs = Vec::new();
    for entry in fs::read_dir(repo.join("formats")).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let spec = fs::read(path.join(format!("{name}.iris"))).unwrap();
        specs.push((name.replace('-', "_"), spec));
    }
    specs.sort();
    specs
}

/// The reads of a format's cases that succeed.
fn format_reads(repo: &Path, header: &str, spec: &[u8]) -> Vec<Read> {
    let directory = repo.join("formats").join(header.replace('_', "-"));
    let mut reads = Vec::new();
    for path in discover(&[directory], None).unwrap() {
        let case = Case::load(&path, "0.1").unwrap();
        if case.operation != Operation::Read || !matches!(case.expected, Outcome::Data(_)) {
            continue;
        }
        let params = case
            .params
            .as_deref()
            .map_or_else(BTreeMap::new, |text| serde_json::from_str(text).unwrap());
        reads.push(Read {
            header: header.to_owned(),
            spec: spec.to_vec(),
            input: fs::read(case.input.unwrap()).unwrap(),
            params,
        });
    }
    reads
}

/// What `data.get(handle)` returns for a handle of `access`, as the C++ guide
/// documents it, written without the printer's help.
fn view_type(access: Access) -> String {
    let Access::Typed {
        value,
        depth,
        optional,
    } = access
    else {
        return "std::optional<iris::ValueRef>".into();
    };
    let (value_type, scalar, row) = match value {
        Leaf::Int => (
            "std::int64_t",
            "std::int64_t",
            "std::span<const std::int64_t>",
        ),
        Leaf::Float => ("double", "double", "std::span<const double>"),
        Leaf::Str => (
            "std::string",
            "std::string_view",
            "iris::Array<std::string, 1>",
        ),
        Leaf::Num => ("iris::Number", "iris::Number", "iris::Numbers"),
    };
    let view = match depth {
        0 => scalar.to_owned(),
        1 => row.to_owned(),
        _ => format!("iris::Array<{value_type}, {depth}>"),
    };
    if optional {
        format!("std::optional<{view}>")
    } else {
        view
    }
}

/// The identifiers that a generated header declares, in order: the word before
/// the brace of each declaration.
fn declared_identifiers(source: &str) -> Vec<String> {
    let flat = source.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.split(';')
        .filter_map(|statement| {
            statement
                .split_once("inline constexpr ")
                .map(|(_, declaration)| declaration)
        })
        .map(|declaration| {
            let (head, _) = declaration
                .split_once('{')
                .expect("a declaration has a brace");
            head.rsplit(' ').next().unwrap().to_owned()
        })
        .collect()
}

/// The functions that render a typed view and an untyped value as the same
/// text, and compare them.
const RENDER: &str = r#"namespace {

int failures = 0;
std::size_t compared = 0;

std::string render(std::int64_t value) { return std::to_string(value); }
std::string render(double value) {
  std::array<char, 32> buffer{};
  const auto end =
      std::to_chars(buffer.data(), buffer.data() + buffer.size(), value).ptr;
  return {buffer.data(), end};
}
std::string render(std::string_view value) {
  return '"' + std::string(value) + '"';
}
std::string render(const iris::Number &value) {
  return std::visit([](auto number) { return render(number); }, value);
}
std::string render(const iris::Numbers &value);
std::string render(const iris::ValueRef &value);
template <class T> std::string render(std::span<const T> row);
template <iris::HandleValue V, std::size_t D>
std::string render(const iris::Array<V, D> &array);
template <class T> std::string render(const std::optional<T> &value);

template <class T> std::string render(std::span<const T> row) {
  std::string text = "[";
  for (std::size_t i = 0; i < row.size(); i++)
    text += (i > 0 ? "," : "") + render(row[i]);
  return text + "]";
}
std::string render(const iris::Numbers &value) {
  return std::visit([](auto row) { return render(row); }, value);
}
template <iris::HandleValue V, std::size_t D>
std::string render(const iris::Array<V, D> &array) {
  std::string text = "[";
  for (std::size_t i = 0; i < array.size(); i++)
    text += (i > 0 ? "," : "") + render(array[i]);
  return text + "]";
}
template <class T> std::string render(const std::optional<T> &value) {
  return value ? render(*value) : "absent";
}
std::string render(const iris::ValueRef &value) {
  if (value.depth() > 0) {
    std::string text = "[";
    for (std::size_t i = 0; i < value.size(); i++)
      text += (i > 0 ? "," : "") + render(value[i]);
    return text + "]";
  }
  switch (value.type()) {
  case iris::ValueType::Int:
    return render(value.integer());
  case iris::ValueType::Float:
    return render(value.number());
  case iris::ValueType::String:
    return render(value.text());
  }
  return "?";
}

/// Compare what `handle` reads from `data` with untyped access
template <class H>
void compare(const iris::Data &data, const H &handle, std::string_view view) {
  const std::string typed = render(data.get(handle));
  const std::string untyped = render(data.find(handle.name()));
  if (typed != untyped) {
    failures++;
    std::cerr << view << ": " << handle.name() << " reads " << typed
              << " through its handle and " << untyped << " untyped\n";
  }
  compared++;
}

/// Check that every entry of `data` is among `names`
void every_entry_has_a_handle(const iris::Data &data,
                              std::initializer_list<std::string_view> names,
                              std::string_view view) {
  for (std::size_t i = 0; i < data.size(); i++) {
    if (std::find(names.begin(), names.end(), data.name(i)) == names.end()) {
      failures++;
      std::cerr << view << ": entry " << data.name(i) << " has no handle\n";
    }
  }
}

"#;

/// The program's `main`, after the table of reads.
const MAIN: &str = r#"
} // namespace

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
  const std::filesystem::path root = argv[1];
  for (std::size_t i = 0; i < std::size(reads); i++) {
    const std::string number = std::to_string(i);
    try {
      const iris::Spec spec = iris::Spec::load(root / (number + ".iris"));
      iris::Params params;
      for (const auto &[name, value] : reads[i].params)
        params.set(name, value);
      reads[i].compare(spec.read(root / (number + ".txt"), params));
    } catch (const std::exception &error) {
      failures++;
      std::cerr << "read " << number << ": " << error.what() << '\n';
    }
  }
  std::cout << std::size(reads) << " reads, " << compared
            << " entries compared\n";
  return failures == 0 ? 0 : 1;
}
// NOLINTEND
"#;

/// The headers that the program includes before the views.
const INCLUDES: &[&str] = &[
    "iris/iris.hpp",
    "algorithm",
    "array",
    "charconv",
    "concepts",
    "cstddef",
    "cstdint",
    "exception",
    "filesystem",
    "initializer_list",
    "iostream",
    "iterator",
    "optional",
    "span",
    "string",
    "string_view",
    "utility",
    "variant",
    "vector",
];

/// A C++ string literal of `text`, with every byte outside printable ASCII
/// escaped.
fn literal(text: &str) -> String {
    let mut quoted = String::from("\"");
    for byte in text.bytes() {
        match byte {
            b'"' | b'\\' => {
                quoted.push('\\');
                quoted.push(char::from(byte));
            }
            b' '..=b'~' => quoted.push(char::from(byte)),
            _ => quoted.push_str(&format!("\\{byte:03o}")),
        }
    }
    quoted.push('"');
    quoted
}

/// The function that compares every handle of `header` and pins its type.
fn compare_function(header: &Header) -> String {
    let name = &header.name;
    let mut text = format!("void compare_{name}(const iris::Data &data) {{\n");
    let handles = header.identifiers.iter().zip(&header.view.handles);
    for (identifier, handle) in handles.clone() {
        let expected = view_type(handle.access);
        writeln!(
            text,
            "  static_assert(std::same_as<decltype(data.get({name}::{identifier})),\n                \
             {expected}>);\n  compare(data, {name}::{identifier}, \"{name}\");"
        )
        .unwrap();
    }
    let names: Vec<String> = handles
        .map(|(identifier, _)| format!("{name}::{identifier}.name()"))
        .collect();
    writeln!(
        text,
        "  every_entry_has_a_handle(data, {{{}}}, \"{name}\");\n}}\n",
        names.join(", ")
    )
    .unwrap();
    text
}

/// A directory of generated views and the program that reads with them,
/// outside the repository.
struct Project {
    cmake: CmakeProject,
}

impl Project {
    fn new(repo: &Path) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cpp-views");
        // Files of an earlier run must not survive; the build directory may,
        // so that a second run compiles only what changed.
        for directory in ["views", "reads"] {
            let path = root.join(directory);
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path).unwrap();
        }
        Self {
            cmake: CmakeProject {
                root,
                repo: repo.to_owned(),
            },
        }
    }

    fn view_path(&self, name: &str) -> PathBuf {
        self.cmake.root.join("views").join(format!("{name}.hpp"))
    }

    /// The view of `spec`, read as `name.iris`, so its namespace is `name`.
    fn write_view(&self, name: &str, spec: &[u8]) -> Header {
        let checked = check_spec(spec, &format!("{name}.iris")).unwrap();
        let code = view_code(&checked.parsed, &checked.analysis, Language::Cpp).unwrap();
        assert!(
            code.contains(&format!("namespace {name} {{")),
            "{name}: {code}"
        );
        fs::write(self.view_path(name), &code).unwrap();
        let view = view::view(&checked.analysis).unwrap();
        let identifiers = declared_identifiers(&code);
        assert_eq!(identifiers.len(), view.handles.len(), "{name}: {code}");
        Header {
            name: name.to_owned(),
            view,
            identifiers,
        }
    }

    /// Every view is what clang-format writes with `cpp/.clang-format`.
    fn format(&self, clang_format: &OsString, headers: &[Header]) {
        let style = format!(
            "--style=file:{}",
            self.cmake.repo.join("cpp/.clang-format").display()
        );
        let mut command = Command::new(clang_format);
        command.args([style.as_str(), "--dry-run", "--Werror"]);
        command.args(headers.iter().map(|header| self.view_path(&header.name)));
        expect_success(command, "clang-format --dry-run --Werror");
    }

    /// clang-tidy with `cpp/.clang-tidy` finds nothing in the views, through
    /// the program that includes them.
    fn tidy(&self, clang_tidy: &OsString) {
        let config = format!(
            "--config-file={}",
            self.cmake.repo.join("cpp/.clang-tidy").display()
        );
        let include = format!(
            "--extra-arg=-I{}",
            self.cmake.repo.join("cpp/include").display()
        );
        let views = self.cmake.root.join("views");
        let filter = format!("--header-filter={}/", regex_escape(&cmake_path(&views)));
        let mut command = Command::new(clang_tidy);
        command.args([config.as_str(), filter.as_str(), "--quiet"]);
        command.args(["--extra-arg=-std=c++23", include.as_str()]);
        command.arg(self.cmake.root.join("compare.cpp"));
        expect_success(command, "clang-tidy");
    }

    /// Write the reads, the program that compares them, and its CMake project.
    fn write_program(&self, headers: &[Header], reads: &[Read]) {
        let reads_directory = self.cmake.root.join("reads");
        // clang-tidy checks the views that the program includes, not the
        // program.
        let mut text = String::from("// NOLINTBEGIN\n");
        for header in INCLUDES {
            writeln!(text, "#include <{header}>").unwrap();
        }
        for header in headers {
            writeln!(text, "#include \"views/{}.hpp\"", header.name).unwrap();
        }
        text.push('\n');
        text.push_str(RENDER);
        for header in headers {
            text.push_str(&compare_function(header));
            text.push('\n');
        }
        text.push_str(
            "struct Read {\n  void (*compare)(const iris::Data &);\n  \
             std::vector<std::pair<std::string, std::string>> params;\n};\n\n\
             const Read reads[] = {\n",
        );
        for (number, read) in reads.iter().enumerate() {
            fs::write(reads_directory.join(format!("{number}.iris")), &read.spec).unwrap();
            fs::write(reads_directory.join(format!("{number}.txt")), &read.input).unwrap();
            let params: Vec<String> = read
                .params
                .iter()
                .map(|(name, value)| format!("{{{}, {}}}", literal(name), literal(value)))
                .collect();
            writeln!(
                text,
                "    {{compare_{}, {{{}}}}},",
                read.header,
                params.join(", ")
            )
            .unwrap();
        }
        text.push_str("};\n");
        text.push_str(MAIN);
        fs::write(self.cmake.root.join("compare.cpp"), text).unwrap();
        let mut lists = self.cmake.lists_head("iris_cpp_views");
        lists.push_str("program(compare compare.cpp)\n");
        lists.push_str("target_include_directories(compare PRIVATE \"${PROJECT_SOURCE_DIR}\")\n");
        fs::write(self.cmake.root.join("CMakeLists.txt"), lists).unwrap();
    }

    /// Runs the program and returns its summary.
    fn compare(&self) -> String {
        let mut run = Command::new(self.cmake.executable("compare"));
        run.arg(self.cmake.root.join("reads"));
        expect_success(run, "the comparison")
    }
}
