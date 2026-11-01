#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 0 ]]; then
  printf 'Usage: %s\n' "$0" >&2
  exit 2
fi
adapter_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_dir=$(cd "$adapter_dir/../../.." && pwd)
cache_dir=${IRIS_CACHE_DIR:-"$repo_dir/.cache"}
case "$cache_dir" in
  /*) ;;
  *) cache_dir="$PWD/$cache_dir" ;;
esac
"$repo_dir/scripts/gecode-reference.sh" >&2
eval "$("$repo_dir/scripts/gecode-reference.sh" --print-env)"
build_dir="$cache_dir/conformance/gecode/$GECODE_REFERENCE_SHA/build"
configure_args=(-S "$adapter_dir" -B "$build_dir")
if [[ ! -f "$build_dir/CMakeCache.txt" ]] && command -v ninja >/dev/null 2>&1; then
  configure_args+=(-G Ninja)
fi
cmake "${configure_args[@]}" \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="$GECODE_REFERENCE_PREFIX" >&2
build_jobs=${CMAKE_BUILD_PARALLEL_LEVEL:-$(getconf _NPROCESSORS_ONLN || printf '2')}
cmake --build "$build_dir" --parallel "$build_jobs" >&2
printf '%s/iris-gecode-driver\n' "$build_dir"
