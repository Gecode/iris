#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Generate fixed-seed random 3-SAT with a complete-clause byte target."""

import argparse
import hashlib
import random
import shutil
import tempfile
from pathlib import Path


def generate(output_path: Path, target_bytes: int, seed: int, variables: int) -> None:
    rng = random.Random(seed)
    size = clauses = 0
    with tempfile.TemporaryFile() as body:
        while size < target_bytes:
            chosen = rng.sample(range(1, variables + 1), 3)
            line = (
                " ".join(str(v if rng.getrandbits(1) else -v) for v in chosen) + " 0\n"
            ).encode("ascii")
            body.write(line)
            size += len(line)
            clauses += 1
        body.seek(0)
        with output_path.open("wb") as output:
            output.write(
                f"c Iris rust-013 seed {seed}\np cnf {variables} {clauses}\n".encode(
                    "ascii"
                )
            )
            shutil.copyfileobj(body, output, length=1024 * 1024)
    with output_path.open("rb") as output:
        digest = hashlib.file_digest(output, "sha256").hexdigest()
    print(f"bytes={output_path.stat().st_size} clauses={clauses} sha256={digest}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--bytes", type=int, default=100_000_000)
    parser.add_argument("--seed", type=int, default=73013)
    parser.add_argument("--variables", type=int, default=100_000)
    args = parser.parse_args()
    if args.bytes < 1 or args.variables < 3:
        parser.error("bytes must be positive and variables must be at least three")
    generate(args.output, args.bytes, args.seed, args.variables)


if __name__ == "__main__":
    main()
