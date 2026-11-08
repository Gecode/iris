#!/usr/bin/env bash
# Builds the C++ release archive iris-cpp-VERSION.tar.gz and its checksum
# file from a commit's tree. See scripts/README.md.
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  printf 'Usage: %s VERSION OUTDIR [REV]\n' "$0" >&2
  exit 2
fi
version=$1
outdir=$2
rev=${3:-HEAD}

repo_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
git=(git -C "$repo_dir" -c tar.umask=022)

commit=$("${git[@]}" rev-parse --verify --quiet "$rev^{commit}") || {
  printf 'cpp-dist: %s names no commit\n' "$rev" >&2
  exit 1
}
declared=$("${git[@]}" show "$commit:cpp/CMakeLists.txt" |
  sed -n 's/^project(iris VERSION \([^ ]*\) .*/\1/p')
if [[ "$declared" != "$version" ]]; then
  printf 'cpp-dist: cpp/CMakeLists.txt at %s declares version %s, not %s\n' \
    "$rev" "${declared:-none}" "$version" >&2
  exit 1
fi

# The library and what building, installing and using it needs. The tests,
# the conformance driver, the fuzz targets, the examples and the presets
# stay in the repository: they need conformance/ and formats/, and the
# conformance tests need Cargo.
paths=(CHANGELOG.md CMakeLists.txt Doxyfile README.md cmake include src)

name=iris-cpp-$version
archive=$name.tar.gz
mkdir -p "$outdir"
outdir=$(cd "$outdir" && pwd)

# Keep LICENSE's bytes, trailing newline included.
license=$("${git[@]}" show "$commit:LICENSE" && printf x)
license=${license%x}

# Every entry gets the commit's time and fixed modes, and gzip records no
# name or time, so the same commit gives the same bytes. LICENSE comes before
# --prefix with its full path, as older Git versions do not prefix it.
mtime=$("${git[@]}" log -1 --format=%ct "$commit")
"${git[@]}" archive --format=tar --mtime="@$mtime" \
  --add-virtual-file="$name/LICENSE:$license" --prefix="$name/" \
  "$commit:cpp" -- "${paths[@]}" |
  gzip -n -9 >"$outdir/$archive.tmp"
mv "$outdir/$archive.tmp" "$outdir/$archive"

if command -v sha256sum >/dev/null 2>&1; then
  (cd "$outdir" && sha256sum "$archive" >"$archive.sha256")
else
  (cd "$outdir" && shasum -a 256 "$archive" >"$archive.sha256")
fi
printf '%s\n' "$outdir/$archive" "$outdir/$archive.sha256"
