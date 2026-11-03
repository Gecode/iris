---
title: Agent skill
description: Give a coding agent the workflow and references for Iris 0.1 reader specs.
editUrl: false
prev: false
next: false
---

The Iris agent skill is a `SKILL.md` with references for writing, explaining and
debugging reader specs. It covers Iris 0.1. The skill has the agent validate
its work with the [CLI](/tools/cli/).

## Install from the repository

The source is [`skill/iris/`](https://github.com/Gecode/iris/tree/main/skill/iris).
Copy that whole directory, including `SKILL.md` and `references/`, into your
agent's skills directory. For example, a project-local Claude Code installation
places it at `.claude/skills/iris/`; a project-local Codex installation places it
at `.agents/skills/iris/`. Keep the reference paths relative to `SKILL.md` intact.
Build the CLI and make `iris` available on the agent's `PATH`.

A packaged Claude Code plugin is planned; copying the directory is the
installation path for now.

## What it covers

The workflow starts with the format description and sample input, looks for a
close match in the [format library](/formats/), then drafts a reader spec.
The agent checks the spec, reads the sample, compares every entry with the input,
adds ranges and checks for the model's assumptions, tests invalid and edge-case
inputs, and formats the result.

The references cover reading patterns, exact language rules, diagnostics,
existing formats, and embedding. They address common mistakes: counts that mean
records rather than values, line-aware statements that advance unexpectedly,
sentinels, array columns from repeats, checks over records, and instance selection.
They also describe what Iris 0.1 cannot express and when the host program must
finish validation.

The [Specification](/specification/0.1/) is normative. When skill guidance and
the Specification disagree, follow the Specification. See the
[error reference](/specification/0.1/errors/) for stable language error codes.
