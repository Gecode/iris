#!/usr/bin/env python3
"""Write the seed corpora for the parse and read fuzz targets.

Seeds come from every case in conformance/cases/0.1 and formats/, plus the
format reader specs. The parse corpus holds each reader spec. The read corpus
holds each spec followed by a NUL byte and the case input, the split that the
read target expects. It leaves out specs that contain NUL, and cases for
Specification chapter 10: those drive a work budget to its limit by design,
and each takes seconds under fuzzing instrumentation. Files are named by the
SHA-1 of their contents, as libFuzzer names its own, so rerunning the script
adds nothing twice and keeps entries that fuzzing added.

Run from anywhere with `python3 -I rust/fuzz/seed-corpus.py`.
"""

import hashlib
import tomllib
from collections.abc import Iterable, Iterator
from pathlib import Path

FUZZ_DIR = Path(__file__).resolve().parent
REPOSITORY = FUZZ_DIR.parents[1]
CASE_ROOTS = [REPOSITORY / "conformance/cases/0.1", REPOSITORY / "formats"]
SPEC_NAMES = ["spec.iris", "spec.bin"]
INPUT_NAMES = ["input.txt", "input.bin"]
WORK_BUDGET_CHAPTER = "spec/0.1/10-work-budgets.md"


def main() -> None:
    parse_seeds: set[bytes] = set()
    read_seeds: set[bytes] = set()
    for case in sorted(case_dirs()):
        spec = first_existing(case, SPEC_NAMES)
        if spec is None:
            continue
        spec_bytes = spec.read_bytes()
        parse_seeds.add(spec_bytes)
        if b"\0" in spec_bytes or is_work_budget_case(case):
            continue
        input_file = first_existing(case, INPUT_NAMES)
        input_bytes = b"" if input_file is None else input_file.read_bytes()
        read_seeds.add(spec_bytes + b"\0" + input_bytes)
    parse_seeds.update(
        path.read_bytes() for path in (REPOSITORY / "formats").glob("*/*.iris")
    )
    for target, seeds in [("parse", parse_seeds), ("read", read_seeds)]:
        added = write_corpus(FUZZ_DIR / "corpus" / target, seeds)
        print(f"{target}: {len(seeds)} seeds, {added} new")


def case_dirs() -> Iterator[Path]:
    for root in CASE_ROOTS:
        for case_file in root.rglob("case.toml"):
            yield case_file.parent


def first_existing(directory: Path, names: list[str]) -> Path | None:
    for name in names:
        path = directory / name
        if path.is_file():
            return path
    return None


def is_work_budget_case(case: Path) -> bool:
    with (case / "case.toml").open("rb") as case_file:
        section = tomllib.load(case_file).get("section", "")
    return section.startswith(WORK_BUDGET_CHAPTER)


def write_corpus(directory: Path, seeds: Iterable[bytes]) -> int:
    directory.mkdir(parents=True, exist_ok=True)
    added = 0
    for seed in seeds:
        path = directory / hashlib.sha1(seed).hexdigest()
        if not path.exists():
            path.write_bytes(seed)
            added += 1
    return added


if __name__ == "__main__":
    main()
