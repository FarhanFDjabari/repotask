---
name: repotask-feature
description: Turn a sprint ticket or PRD into scoped, reviewable implementation steps using the repo-task CLI (fetch, summarize, analyze, split). Use when asked to implement a ticket, plan a feature, estimate effort, or break work down. Triggers include a ticket id, "implement", "plan this feature", "how big is this", "break this down", or a pasted PRD.
---

# Feature workflow via repo-task

Four steps, each one a CLI call that gathers evidence for a decision you then make.
The CLI never calls a model; it fetches, filters, and computes impact.

## 1. Fetch

```bash
repo-task --json fetch <ticket>
```

Use this even when you have an MCP tool for the tracker — the CLI routes the call and
stores the result for the later steps.

If the response contains `request` instead of the ticket, the CLI could not reach the API
itself (connector in `mcp` mode, or `auto` mode with no credential configured). Call the
tool it names, then store the result — without this the later steps have nothing to read.

```bash
repo-task fetch <ticket> --write -   # pipe the ticket text on stdin
```

## 2. Summarize

```bash
repo-task --json summarize <ticket>
```

Returns the raw source, this project's context pack, and a contract. Most PRDs
describe several platforms — keep only what applies to this project's stacks, drop
the rest, and preserve every surviving acceptance criterion. Write it back:

```bash
repo-task summarize <ticket> --write -
```

## 3. Analyze

```bash
repo-task --json analyze <ticket>
```

Returns the impact set — the files, symbols, and modules the ticket's vocabulary
resolves to — plus an effort rubric. Estimate from that evidence and name the two or
three facts that drove your band. If the impact set is empty, the index may be stale
or the ticket may use vocabulary the code does not: say which.

## 4. Split

```bash
repo-task --json split <ticket>
```

Returns proposed increments grouped by module and layer, data before UI. Each step
must build, be independently revertible, and be reviewable on its own. Merge steps
that cannot stand alone; split any step whose diff would be hard to review.

## Rules

- Run the steps in order — each reads what the previous one stored.
- Do not start editing code during `analyze` or `split`. Plan first, implement after.
- Effort estimates are yours, but they must cite the impact set, not the prose.
- Load the `repotask-search` skill when you need conventions while implementing.
