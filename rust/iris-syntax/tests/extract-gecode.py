#!/usr/bin/env -S uv run --script
# /// script
# dependencies = []
# ///
"""Extract the six pinned instance_spec raw strings without decoding them."""

import argparse
import re
import subprocess
from pathlib import Path

PIN = "5e7bbd5c26b0702c6fd652356b2289da0710ac3c"
SOURCES = {
    "sat": "sat.cpp",
    "tsp": "tsp.cpp",
    "tsptw": "tsptw.cpp",
    "job-shop": "job-shop.cpp",
    "nonogram": "nonogram.cpp",
    "sudoku": "sudoku-instances.hh",
}


def extract(repository: Path, source: str) -> bytes:
    contents = subprocess.check_output(
        ["git", "-C", str(repository), "show", f"{PIN}:examples/{source}"]
    )
    declarations = re.findall(rb"const char\* instance_spec\s*=", contents)
    payloads = re.findall(
        rb'const char\* instance_spec\s*=\s*R"\((.*?)\)";', contents, re.DOTALL
    )
    if len(declarations) != 1 or len(payloads) != 1:
        raise ValueError(f"expected one empty-delimiter raw string in {source}")
    return payloads[0]


def main() -> None:
    root = Path(__file__).resolve().parents[3]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--reference-repo", type=Path, default=root / ".cache" / "gecode" / PIN / "src"
    )
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    destination = Path(__file__).parent / "fixtures" / "gecode"
    for name, source in SOURCES.items():
        payload = extract(args.reference_repo, source)
        target = destination / f"{name}.iris"
        if args.check:
            if target.read_bytes() != payload:
                raise ValueError(f"fixture differs: {target}")
        else:
            target.write_bytes(payload)
        print(
            f"{name}: {len(payload)} bytes {'verified' if args.check else 'extracted'}"
        )


if __name__ == "__main__":
    main()
