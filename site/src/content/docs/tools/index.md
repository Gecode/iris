---
title: Tools
description: Check, read, format, and edit Iris reader specs.
editUrl: false
prev: false
next: false
---

The Rust implementation supplies the `iris` command-line tool and the
language server. Build them from source; package registries, binary releases,
and editor marketplace listings are planned.

- [CLI](/tools/cli/): validate a reader spec, read input as JSON, dzn or a summary, list instances, and run conformance cases.
- [Formatter](/tools/formatter/): one style for reader specs, with the line width from EditorConfig.
- [Language server](/tools/lsp/): diagnostics, completion, navigation, formatting, and live sample reading.
- [Editors](/tools/editors/): the shared TextMate grammar and the JetBrains plugin.
- [Agent skill](/tools/agent-skill/): guidance for an agent writing or debugging Iris 0.1 reader specs.

The [Specification](/specification/0.1/) defines what reader specs mean.
Tool diagnostics link to its [error codes](/specification/0.1/errors/).
