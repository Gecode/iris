"""The ``iris`` console script, which runs the Rust ``iris`` command line."""

from __future__ import annotations

import signal
import sys

from gecode_iris._native import run_cli


def main() -> None:
    """Run ``iris`` on this process's arguments and exit with its status."""
    # The Rust code writes to the process's file descriptors directly.
    sys.stdout.flush()
    sys.stderr.flush()
    # Python's handler would only take effect once the Rust code returns, so
    # restore the default; an inherited SIG_IGN or a custom handler stays.
    if signal.getsignal(signal.SIGINT) is signal.default_int_handler:
        signal.signal(signal.SIGINT, signal.SIG_DFL)
    sys.exit(run_cli(sys.argv))
