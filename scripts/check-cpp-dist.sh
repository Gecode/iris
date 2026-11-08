#!/usr/bin/env bash
# Builds and installs a C++ release archive as a project of its own, then
# builds and runs cpp/tests/consumer against the installed copy and through
# FetchContent of the archive. See scripts/README.md.
set -euo pipefail

if [[ $# -lt 1 || $# -gt 2 ]]; then
  printf 'Usage: %s ARCHIVE [WORKDIR]\n' "$0" >&2
  exit 2
fi
archive=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
consumer="$repo_dir/cpp/tests/consumer"

name=$(basename "$archive" .tar.gz)
if [[ "$name" != iris-cpp-* || "$archive" != *.tar.gz ]]; then
  printf 'check-cpp-dist: %s is not named iris-cpp-VERSION.tar.gz\n' "$archive" >&2
  exit 1
fi

if command -v sha256sum >/dev/null 2>&1; then
  hash=$(sha256sum "$archive" | cut -d ' ' -f 1)
else
  hash=$(shasum -a 256 "$archive" | cut -d ' ' -f 1)
fi
if [[ -f "$archive.sha256" ]]; then
  read -r recorded file <"$archive.sha256"
  if [[ "$recorded" != "$hash" || "$file" != "$name.tar.gz" ]]; then
    printf 'check-cpp-dist: %s does not match the archive\n' "$archive.sha256" >&2
    exit 1
  fi
fi

if [[ $# -eq 2 ]]; then
  work=$2
  mkdir -p "$work"
else
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT
fi
work=$(cd "$work" && pwd)
rm -rf "$work/src" "$work/build" "$work/prefix" "$work/find-package" "$work/fetch-content"
mkdir "$work/src"

# One top-level directory, named after the archive.
tar -xzf "$archive" -C "$work/src"
top=$(ls -A "$work/src")
if [[ "$top" != "$name" ]]; then
  printf 'check-cpp-dist: the archive holds %s, not one directory %s\n' \
    "$(echo "$top" | tr '\n' ' ')" "$name" >&2
  exit 1
fi

set -x
cmake -S "$work/src/$name" -B "$work/build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_COMPILE_WARNING_AS_ERROR=ON
cmake --build "$work/build"
cmake --install "$work/build" --prefix "$work/prefix"

cmake -S "$consumer" -B "$work/find-package" -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_COMPILE_WARNING_AS_ERROR=ON "-DCMAKE_PREFIX_PATH=$work/prefix"
cmake --build "$work/find-package"
"$work/find-package/consumer"

cmake -S "$consumer" -B "$work/fetch-content" -G Ninja -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_COMPILE_WARNING_AS_ERROR=ON "-DIRIS_URL=file://$archive" "-DIRIS_URL_SHA256=$hash"
cmake --build "$work/fetch-content"
"$work/fetch-content/consumer"
