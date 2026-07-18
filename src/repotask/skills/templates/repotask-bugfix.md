---
name: repotask-bugfix
description: Triage a bug report against the code it implicates and find duplicate tickets that share a root cause, using the repo-task CLI (bug fetch, bug dedupe). Use when handling a defect, crash, regression, or when asked whether tickets overlap. Triggers include "this is broken", "crash", "regression", "triage", "duplicate tickets", "which tickets can we merge".
---

# Bugfix workflow via repo-task

## Triage one report

```bash
repo-task --json bug fetch <ticket>
```

Returns the report, the impact set it resolves to, and this project's conventions for
that code. Find the root cause before proposing a fix, and name which impacted symbol
you believe is responsible.

If the connector runs in `mcp` mode, the first response contains a tool call — make it,
store the report with `repo-task fetch <ticket> --write -`, then run `bug fetch` again.

## Find duplicates

```bash
repo-task --json bug dedupe
repo-task --json bug dedupe --threshold 0.5   # stricter grouping
```

Clusters tickets whose reports resolve to the same files and symbols. In `mcp` mode it
returns a search call; make it and pipe the tickets back as JSON:

```bash
repo-task bug dedupe --tickets -   # [{"id": "...", "title": "...", "body": "..."}]
```

## Rules

- Shared code is not the same as a shared root cause. Confirm each cluster against the
  reports before recommending a merge, name the ticket that should survive, and say
  what the others would lose.
- Never recommend closing a ticket you have not read.
- State the root cause before the fix. A fix without a named cause is a guess.
- If the evidence does not support one root cause, say what signal you still need —
  a stack trace, a repro, a log — rather than picking the most likely file.
