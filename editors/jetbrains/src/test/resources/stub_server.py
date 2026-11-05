#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Small stdio LSP peer for the plugin spike; never shipped in the plugin."""

import json
import sys
from typing import Any


def receive() -> dict[str, Any] | None:
    headers: dict[str, str] = {}
    while line := sys.stdin.buffer.readline():
        if line == b"\r\n":
            return json.loads(sys.stdin.buffer.read(int(headers["content-length"])))
        name, value = line.decode("ascii").split(":", 1)
        headers[name.lower()] = value.strip()
    return None


def send(message: dict[str, Any]) -> None:
    body = json.dumps({"jsonrpc": "2.0", **message}).encode("utf-8")
    sys.stdout.buffer.write(
        f"Content-Length: {len(body)}\r\n\r\n".encode("ascii") + body
    )
    sys.stdout.buffer.flush()


def publish_diagnostic(document: dict[str, Any]) -> None:
    send(
        {
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": document["uri"],
                "version": document.get("version"),
                "diagnostics": [
                    {
                        "range": {
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 0, "character": 6},
                        },
                        "severity": 2,
                        "code": "iris-stub",
                        "codeDescription": {"href": "https://www.gecode.dev/"},
                        "source": "iris-stub",
                        "message": "Iris stub diagnostic",
                    }
                ],
            },
        }
    )


def main() -> int:
    shutdown = False
    while (message := receive()) is not None:
        method = message.get("method")
        if method == "initialize":
            send(
                {
                    "id": message["id"],
                    "result": {
                        "capabilities": {
                            "textDocumentSync": {"openClose": True, "change": 1}
                        },
                        "serverInfo": {"name": "iris-stub", "version": "0.1"},
                    },
                }
            )
        elif method in ("textDocument/didOpen", "textDocument/didChange"):
            publish_diagnostic(message["params"]["textDocument"])
        elif method == "shutdown":
            shutdown = True
            send({"id": message["id"], "result": None})
        elif method == "exit":
            return 0 if shutdown else 1
        elif "id" in message:
            send({"id": message["id"], "result": None})
    return 0


if __name__ == "__main__":
    sys.exit(main())
