# Repository scripts

These scripts support the repository foundations (M0), the layout in
`PLAN.md` §5, and the Gecode reference used by §6.1 and §7.3.

## Check repository conventions

Run `./scripts/check-repo.sh` from any directory. It requires Bash (3.2 or
newer), Git, and standard tools including awk, and works without network
access on macOS and Linux. The repository workflow runs it on pushes and
pull requests.

The script checks the required files (`README.md`, `LICENSE`, `AGENTS.md`,
`CONTEXT.md`, `PLAN.md`, and `docs/adr/README.md`), name spelling in tracked
text files, and relative links in tracked `.md` files. It reports all
violations with file names and line numbers where available, then exits
non-zero if any check fails. Untracked files, including `.zdev/` and
`.cache/`, are excluded naturally by Git; the script does not read them.
New files enter these checks once tracked (including intent-to-add files).

The name check uses `git grep -I -nw` to find the whole word in capitals,
skipping binary files. `name_exceptions` in the script is an empty list of
`path:pattern` exceptions: an exact repository-relative path and an extended
regular expression matching an allowed line.

Markdown links follow these conventions:

- Inline links and images are checked relative to the containing Markdown
  file. Destinations may use angle brackets, escaped punctuation, balanced
  parentheses, percent encoding, and an optional title.
- Fragments (`#...`) and query strings (`?...`) are removed before checking
  the path. Fragment-only links are skipped; heading anchors are not checked.
  The target may be an existing file or directory, whether tracked or not.
- Absolute URLs (including `mailto:` and other URI schemes) and paths
  beginning with `/` are skipped. No network requests are made.
- Code spans (including matching runs of multiple backticks) and fenced
  code blocks (backticks or tildes) are skipped. Code spans end at paragraph
  or block boundaries. Fences may appear in block quotes and space-indented
  lists; their indentation is measured within the container.
- Reference-style links are checked at their `[label]: destination`
  definitions, including definitions whose destination is on the next line.
  This covers full, collapsed, and shortcut references and also checks unused
  definitions. Undefined bracketed labels are plain text, not link errors.

This is a pragmatic CommonMark subset. HTML blocks, indented code blocks,
tab-indented containers, block quotes inside lists, and reference definitions
inside containers are not supported.

## Build the Gecode reference

From the repository root:

```bash
./scripts/gecode-reference.sh
eval "$(./scripts/gecode-reference.sh --print-env)"
"$GECODE_REFERENCE_INSTANCE_TOOL" -help
```

The build requires Git, CMake 3.21 or newer, a C++17 compiler, and a build
tool. It prefers Ninja when available and otherwise uses CMake's default
generator. It uses the available CPU count; set `CMAKE_BUILD_PARALLEL_LEVEL`
to limit parallel compilation.

The script shallow-fetches the pinned Gecode commit directly from GitHub.
It does not use an existing Gecode checkout. The pin lives near the top of
`gecode-reference.sh` and names the `release-6.5.0` tag's commit.

By default, sources, build output, and installed files are stored under
`<repo>/.cache/gecode/<sha>/{src,build,install}`. Set `IRIS_CACHE_DIR` to use
another cache root (relative paths are relative to the calling directory):

```bash
export IRIS_CACHE_DIR="$HOME/.cache/iris"
./scripts/gecode-reference.sh
eval "$(./scripts/gecode-reference.sh --print-env)"
```

Repeated builds reuse the checkout and rebuild only changed targets.
`--print-env` only checks the installed files and prints shell-quoted
exports; it never fetches, configures, builds, or creates directories. It
exits non-zero with a message if the reference has not been built.

| Export | Value |
|---|---|
| `GECODE_REFERENCE_INSTANCE_TOOL` | Installed `bin/gecode-instance` executable |
| `GECODE_REFERENCE_PREFIX` | Install prefix with `include/`, `lib/`, and `lib/cmake/Gecode/` |
| `GECODE_REFERENCE_SHA` | Full pinned commit SHA |
| `GECODE_REFERENCE_INCLUDE_DIR` | Installed header directory, containing `gecode/instance.hh` |
| `GECODE_REFERENCE_INSTANCE_LIBRARY` | Installed static `lib/libgecodeinstance.a` |

Builds use Release mode and static libraries with runtime search paths
disabled, so running the tool needs no shared-library path setup. The instance
reader needs integer variables, kernel, and support. Set and float variables,
search, minimodel, driver, FlatZinc, examples, tests, Qt/Gist, MPFR, and
CPProfiler are disabled because
the reader and tool do not need them. Only `gecode-instance` and its library
dependencies are built; upstream install rules supply the headers and CMake
package without modifying Gecode sources.

An adapter can use the installed CMake package to link all required static
dependencies: set `CMAKE_PREFIX_PATH` to `GECODE_REFERENCE_PREFIX`, use
`find_package(Gecode CONFIG REQUIRED COMPONENTS instance)`, and link
`Gecode::gecodeinstance`.

## Check and set versions

`python3 -I scripts/versions.py check` reads the version of each component
from every place it is written: the Rust workspace, the internal requirements
between crates and both Cargo lockfiles, `python/Cargo.toml`,
`typescript/package.json` and its lockfile, `java/gradle.properties`, and
`cpp/CMakeLists.txt`. It fails when the copies of one component disagree, or
when Rust, Python and TypeScript, which share one version, differ. The
repository workflow runs it. It needs Python 3.9 or newer and Git, and makes no
network requests.

`set COMPONENT VERSION` rewrites every copy, for `rust`, `python`,
`typescript`, `java`, `cpp`, or `reader` for the first three together, then
runs the check. The lockfile entries it rewrites are the workspace's own
packages, so `--locked` builds keep working without a `cargo update`.

`release-check TAG` checks a `COMPONENT-vVERSION` tag against the source. For
a Python or TypeScript tag it also requires the `rust-vVERSION` tag, and fails
when the Rust sources the package builds in changed since it. The release
workflows run it. [RELEASING.md](../RELEASING.md) describes the procedure.

## Build the C++ release archive

From any directory:

```bash
./scripts/cpp-dist.sh VERSION OUTDIR [REV]
./scripts/check-cpp-dist.sh OUTDIR/iris-cpp-VERSION.tar.gz [WORKDIR]
```

`cpp-dist.sh` writes `iris-cpp-VERSION.tar.gz` and
`iris-cpp-VERSION.tar.gz.sha256` to `OUTDIR`, in the format of `sha256sum`.
It reads the tree of the commit `REV`, `HEAD` by default, and never the
working tree, and fails unless `cpp/CMakeLists.txt` in that commit declares
`project(iris VERSION VERSION ...)`. The archive holds one directory,
`iris-cpp-VERSION/`, with `CHANGELOG.md`, `CMakeLists.txt`, `Doxyfile`,
`README.md`, `cmake/`, `include/` and `src/` from `cpp/`, and the repository's
`LICENSE`. Entries carry the commit's time and modes 644 and 755, and gzip
records no name or time, so one commit gives the same bytes with the same Git
and gzip. It needs Bash, Git 2.42 or newer, gzip, and `sha256sum` or `shasum`.

`check-cpp-dist.sh` checks the archive's name, the checksum file beside it if
there is one, and that the archive holds that one directory. It then
configures, builds and installs the archive as a top-level project with
warnings as errors, builds and runs `cpp/tests/consumer` against the installed
package, and builds and runs it again through FetchContent of the archive by
`file://` URL and SHA-256 hash. It uses `cmake` (3.28 or newer) and `ninja`
from `PATH` and the compiler that `CXX` names, works in a temporary directory
that it removes unless `WORKDIR` is given, and exits non-zero at the first
failure. The C++ release workflow runs both scripts.
