#!/usr/bin/env bash
set -euo pipefail

gecode_repository=https://github.com/Gecode/gecode
# The release-6.5.0 tag.
gecode_sha=599f977b7855dba2a1a5f67db1892722372067d3

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cache_dir=${IRIS_CACHE_DIR:-"$repo_dir/.cache"}
case "$cache_dir" in
  /*) ;;
  *) cache_dir="$PWD/$cache_dir" ;;
esac
reference_dir="$cache_dir/gecode/$gecode_sha"
source_dir="$reference_dir/src"
build_dir="$reference_dir/build"
prefix="$reference_dir/install"
instance_tool="$prefix/bin/gecode-instance"
include_dir="$prefix/include"
instance_library="$prefix/lib/libgecodeinstance.a"

print_env() {
  if [[ ! -x "$instance_tool" || ! -f "$include_dir/gecode/instance.hh" ||
        ! -f "$instance_library" ]]; then
    printf 'Gecode reference is not built at %s. Run %s first.\n' \
      "$prefix" "$repo_dir/scripts/gecode-reference.sh" >&2
    return 1
  fi
  printf 'export GECODE_REFERENCE_INSTANCE_TOOL=%q\n' "$instance_tool"
  printf 'export GECODE_REFERENCE_PREFIX=%q\n' "$prefix"
  printf 'export GECODE_REFERENCE_SHA=%q\n' "$gecode_sha"
  printf 'export GECODE_REFERENCE_INCLUDE_DIR=%q\n' "$include_dir"
  printf 'export GECODE_REFERENCE_INSTANCE_LIBRARY=%q\n' "$instance_library"
}

if [[ $# -eq 1 && "${1:-}" == --print-env ]]; then
  print_env
  exit 0
elif [[ $# -ne 0 ]]; then
  printf 'Usage: %s [--print-env]\n' "$0" >&2
  exit 2
fi

if [[ ! -d "$source_dir/.git" ]]; then
  mkdir -p "$source_dir"
  git -C "$source_dir" init
fi
# Fetch by URL so that an interrupted earlier run cannot leave a missing remote.
if [[ $(git -C "$source_dir" rev-parse HEAD 2>/dev/null || true) != "$gecode_sha" ]]; then
  git -C "$source_dir" fetch --depth 1 "$gecode_repository" "$gecode_sha"
  git -C "$source_dir" checkout --detach FETCH_HEAD
fi

# Keep an existing generator; prefer Ninja for a new build directory.
cmake_args=(-S "$source_dir" -B "$build_dir")
if [[ ! -f "$build_dir/CMakeCache.txt" ]] && command -v ninja >/dev/null 2>&1; then
  cmake_args+=(-G Ninja)
fi
cmake "${cmake_args[@]}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DCMAKE_INSTALL_BINDIR=bin \
  -DCMAKE_INSTALL_INCLUDEDIR=include \
  -DCMAKE_INSTALL_LIBDIR=lib \
  -DCMAKE_INSTALL_MESSAGE=LAZY \
  -DCMAKE_SKIP_RPATH=ON \
  -DGECODE_BUILD_SHARED=OFF \
  -DGECODE_BUILD_STATIC=ON \
  -DGECODE_INSTALL=ON \
  -DGECODE_ENABLE_INSTANCE=ON \
  -DGECODE_ENABLE_INT_VARS=ON \
  -DGECODE_ENABLE_SEARCH=OFF \
  -DGECODE_ENABLE_SET_VARS=OFF \
  -DGECODE_ENABLE_FLOAT_VARS=OFF \
  -DGECODE_ENABLE_MINIMODEL=OFF \
  -DGECODE_ENABLE_DRIVER=OFF \
  -DGECODE_ENABLE_FLATZINC=OFF \
  -DGECODE_ENABLE_EXAMPLES=OFF \
  -DGECODE_ENABLE_QT=OFF \
  -DGECODE_ENABLE_GIST=OFF \
  -DGECODE_ENABLE_MPFR=OFF \
  -DGECODE_ENABLE_CPPROFILER=OFF \
  -DBUILD_TESTING=OFF

build_jobs=${CMAKE_BUILD_PARALLEL_LEVEL:-$(getconf _NPROCESSORS_ONLN || printf '2')}
cmake --build "$build_dir" --target gecode-instance --parallel "$build_jobs"
cmake --install "$build_dir" --config Release
print_env
