# Repository scripts

These scripts support the repository foundations (M0), the layout in
`PLAN.md` §5, and the Gecode reference used by §6.1 and §7.3.

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
`gecode-reference.sh`; it will move to `release-6.5.0` when that tag exists.

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
