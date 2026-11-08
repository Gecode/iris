#!/usr/bin/env python3
"""Check, set and release-check the package versions of each component.

Usage, from anywhere in the repository:

    python3 -I scripts/versions.py check
    python3 -I scripts/versions.py set COMPONENT VERSION
    python3 -I scripts/versions.py release-check TAG

Components: rust, python, typescript, java, cpp, and reader, which names rust,
python and typescript together. See RELEASING.md.
"""

from __future__ import annotations

import re
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(
    subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        check=True,
        capture_output=True,
        text=True,
        cwd=Path(__file__).resolve().parent,
    ).stdout.strip()
)

SEMVER = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$")

# Rust, Python and TypeScript share the reader: the packages bind the Rust
# crates, so they carry one version and are released from one commit.
READER = ("rust", "python", "typescript")

# Releases made before the release check existed. Python and TypeScript 0.1.0
# were built from a later commit than rust-v0.1.0.
CHECKED_BEFORE = {"python-v0.1.0", "typescript-v0.1.0"}

# The Rust sources that the Python and TypeScript packages build in. A change
# here needs a Rust release at the same version first. rust/iris-js is the
# TypeScript package's own boundary; rust/Cargo.lock is excluded because
# Python builds with python/Cargo.lock.
BOUND_RUST = [
    "rust/Cargo.toml",
    "rust/*/Cargo.toml",
    "rust/*/build.rs",
    "rust/*/src/*",
    ":(exclude)rust/iris-js",
    ":(exclude)rust/iris-wasm",
    ":(exclude)rust/iris-driver",
    ":(exclude)rust/fuzz",
]


@dataclass
class Site:
    """One place a version is written: a file and a pattern whose group 2 is it."""

    path: str
    pattern: re.Pattern[str]
    what: str
    count: int = 1  # matches expected; 0 means one or more


def site(path: str, pattern: str, what: str, count: int = 1) -> Site:
    return Site(path, re.compile(pattern, re.MULTILINE), what, count)


def lock_sites(path: str, names: Callable[[str], bool], what: str) -> list[Site]:
    """The version lines of the path packages in a Cargo.lock, which have no source."""
    text = (ROOT / path).read_text()
    found = []
    for block in text.split("\n[[package]]\n")[1:]:
        name = re.search(r'^name = "([^"]+)"$', block, re.MULTILINE)
        if name and "\nsource = " not in block and names(name.group(1)):
            pattern = rf'(\[\[package\]\]\nname = "{re.escape(name.group(1))}"\nversion = ")([^"]+)(")'
            found.append(site(path, pattern, f"{what} {name.group(1)}"))
    return found


def rust_crate(name: str) -> bool:
    return name.startswith("gecode-iris") and name != "gecode-iris-python" or name.startswith("iris-")


def sites(component: str) -> list[Site]:
    if component == "rust":
        found = [site("rust/Cargo.toml", r'^(\[workspace\.package\]\n(?:[^\[\n].*\n)*?version = ")([^"]+)(")', "workspace version")]
        for manifest in sorted((ROOT / "rust").glob("*/Cargo.toml")):
            if manifest.parent.name == "fuzz":
                continue
            rel = manifest.relative_to(ROOT).as_posix()
            if re.search(r'^gecode-iris[-a-z]* = \{ version = "', manifest.read_text(), re.MULTILINE):
                found.append(site(rel, r'^(gecode-iris[-a-z]* = \{ version = ")([^"]+)(")', "internal requirement", 0))
        found += lock_sites("rust/Cargo.lock", rust_crate, "lock entry")
        found += lock_sites("python/Cargo.lock", rust_crate, "Python lock entry")
        return found
    if component == "python":
        return [
            site("python/Cargo.toml", r'^(\[package\]\n(?:[^\[\n].*\n)*?version = ")([^"]+)(")', "package version"),
            *lock_sites("python/Cargo.lock", lambda n: n == "gecode-iris-python", "lock entry"),
        ]
    if component == "typescript":
        return [
            site("typescript/package.json", r'^(  "version": ")([^"]+)(")', "package version"),
            site("typescript/package-lock.json", r'\A(\{\n  "name": "gecode-iris",\n  "version": ")([^"]+)(")', "lock version"),
            site("typescript/package-lock.json", r'(\n    "": \{\n      "name": "gecode-iris",\n      "version": ")([^"]+)(")', "lock root package"),
        ]
    if component == "java":
        return [site("java/gradle.properties", r"^(version=)(\S+)()$", "version")]
    if component == "cpp":
        return [site("cpp/CMakeLists.txt", r"^(project\(iris VERSION )(\S+)()", "project version")]
    raise SystemExit(f"unknown component {component!r}; expected one of {', '.join(COMPONENTS)} or reader")


COMPONENTS = ("rust", "python", "typescript", "java", "cpp")


def read(component: str) -> tuple[set[str], list[str]]:
    """The versions found for a component, and any problems finding them."""
    versions: set[str] = set()
    problems = []
    for s in sites(component):
        matches = list(s.pattern.finditer((ROOT / s.path).read_text()))
        if not matches or (s.count and len(matches) != s.count):
            problems.append(f"{s.path}: expected {s.count or 'some'} {s.what}, found {len(matches)}")
        for m in matches:
            versions.add(m.group(2))
    return versions, problems


def check() -> int:
    failed = False
    found = {}
    for component in COMPONENTS:
        versions, problems = read(component)
        for problem in problems:
            print(f"{component}: {problem}")
            failed = True
        if len(versions) != 1:
            print(f"{component}: versions disagree: {', '.join(sorted(versions))}")
            failed = True
            continue
        found[component] = versions.pop()
        print(f"{component:<11} {found[component]}")
    reader = {found[c] for c in READER if c in found}
    if len(reader) > 1:
        print(f"reader: rust, python and typescript must share one version, found {', '.join(sorted(reader))}")
        failed = True
    return 1 if failed else 0


def set_version(target: str, version: str) -> int:
    if not SEMVER.match(version):
        raise SystemExit(f"{version!r} is not a semantic version")
    components = READER if target == "reader" else (target,)
    for component in components:
        for s in sites(component):
            path = ROOT / s.path
            text, n = s.pattern.subn(lambda m: m.group(1) + version + m.group(3), path.read_text())
            if not n:
                raise SystemExit(f"{s.path}: no {s.what} to set")
            path.write_text(text)
    return check()


def git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)


def release_check(tag: str) -> int:
    m = re.fullmatch(r"([a-z]+)-v(.+)", tag)
    if not m or m.group(1) not in COMPONENTS:
        raise SystemExit(f"{tag!r} is not a component release tag")
    component, version = m.groups()
    versions, problems = read(component)
    if problems or versions != {version}:
        for problem in problems:
            print(problem)
        print(f"{tag}: the source has {component} {', '.join(sorted(versions))}")
        return 1
    if component in READER and component != "rust" and tag not in CHECKED_BEFORE:
        rust_tag = f"rust-v{version}"
        if git("rev-parse", "--verify", "--quiet", f"refs/tags/{rust_tag}").returncode:
            print(f"{tag}: release {rust_tag} first; the package binds the Rust crates")
            return 1
        changed = git("diff", "--name-only", rust_tag, "HEAD", "--", *BOUND_RUST).stdout.split()
        if changed:
            print(f"{tag}: the Rust sources the package builds in changed since {rust_tag}:")
            print("\n".join(f"  {p}" for p in changed))
            print("Release Rust at the same version from this commit first.")
            return 1
    print(f"{tag}: ok")
    return 0


def main(argv: list[str]) -> int:
    if argv[:1] == ["check"] and len(argv) == 1:
        return check()
    if argv[:1] == ["set"] and len(argv) == 3:
        return set_version(argv[1], argv[2])
    if argv[:1] == ["release-check"] and len(argv) == 2:
        return release_check(argv[1])
    print(__doc__.strip(), file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
