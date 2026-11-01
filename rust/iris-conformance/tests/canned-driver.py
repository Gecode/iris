#!/usr/bin/env python3
"""Return canned bytes through the real protocol; never interpret an Iris spec."""

import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--prefix", default="")
    parser.add_argument("operation", choices=["parse", "read", "catalog", "dzn"])
    parser.add_argument("spec", type=Path)
    parser.add_argument("input", type=Path, nargs="?")
    parser.add_argument("--params")
    parser.add_argument("--version", required=True)
    parser.add_argument("--format", choices=["json", "dzn"])
    args = parser.parse_args()
    control = json.loads((args.spec.parent / "canned.json").read_text())
    assert args.operation == control["operation"]
    assert args.version == "0.1"
    assert args.spec.is_absolute()
    assert args.params == control.get("params")
    assert args.prefix == control.get("prefix", "")
    assert args.format == control.get("format")
    if args.operation == "parse":
        assert args.input is None
    else:
        assert args.input is not None and args.input.is_absolute()
    if "spec_hex" in control:
        assert args.spec.read_bytes().hex() == control["spec_hex"]
    if "input_hex" in control:
        assert args.input.read_bytes().hex() == control["input_hex"]
    if "hold_pipes" in control:
        subprocess.Popen(
            [
                sys.executable,
                "-c",
                "import sys; import time; time.sleep(float(sys.argv[1]))",
                str(control["hold_pipes"]),
            ]
        )
    if "pid_file" in control:
        Path(control["pid_file"]).write_text(str(os.getpid()))
    sys.stderr.buffer.write(control.get("stderr", "").encode())
    sys.stderr.buffer.flush()
    time.sleep(control.get("delay", 0))
    if control.get("signal"):
        os.kill(os.getpid(), signal.SIGTERM)
    sys.stdout.buffer.write((args.spec.parent / control["output_file"]).read_bytes())
    return control.get("exit", 0)


if __name__ == "__main__":
    sys.exit(main())
