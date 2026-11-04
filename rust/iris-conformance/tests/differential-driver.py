#!/usr/bin/env python3
"""A fake driver for differential tests; it never interprets an Iris spec.

It parses every spec, and reads one entry, `lines`, the number of lines of
the spec. With --skew it adds 1 when a spec line contains a colon, so it
disagrees with the plain driver exactly then. With --exit N it exits N
without output for every request but parse. With --sleep SECONDS it waits
that long before answering any request but parse.
"""

import argparse
import json
import sys
import time
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skew", action="store_true")
    parser.add_argument("--exit", type=int)
    parser.add_argument("--sleep", type=float)
    parser.add_argument("operation", choices=["parse", "read", "catalog", "dzn"])
    parser.add_argument("spec", type=Path)
    parser.add_argument("input", type=Path, nargs="?")
    parser.add_argument("--params")
    parser.add_argument("--version", required=True)
    parser.add_argument("--format", choices=["json", "dzn"])
    args = parser.parse_args()
    if args.operation == "parse":
        print(json.dumps({"iris": "0.1", "parsed": True}))
        return 0
    if args.exit is not None:
        return args.exit
    if args.sleep is not None:
        time.sleep(args.sleep)
    lines = args.spec.read_bytes().splitlines()
    value = len(lines)
    if args.skew and any(b":" in line for line in lines):
        value += 1
    if args.operation == "dzn" or args.format == "dzn":
        sys.stdout.write(f"lines = {value};\n")
    else:
        entry = {"name": "lines", "type": "int", "depth": 0, "value": value}
        print(json.dumps({"iris": "0.1", "entries": [entry]}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
