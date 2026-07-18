---
name: repotask-search
description: Look up this project's architecture conventions, task playbooks, and indexed code facts (modules, viewmodels, routes, symbols) through the repo-task CLI. Use BEFORE writing or changing code in this repository, and whenever you need to know how this project does something — naming, layering, which library it uses, where a symbol lives. Triggers include "how does this project", "what is the convention for", "where is X defined", "which module owns".
---

# Project knowledge via repo-task

This project's conventions, playbooks, and code index live in a knowledge base that
`repo-task` reads for you. Query it instead of grepping the repository or guessing —
the knowledge base is reviewed and current, your assumptions are not.

## Start here

```bash
repo-task --json brief "<what you are about to do>" --changed
```

`brief` returns one budgeted pack: the conventions and recipes that apply to this
project's stacks, plus the indexed facts, ranked for this task. `--changed` adds the
files you are already touching so ranking sharpens. This is almost always the right
first call.

## Targeted lookups

```bash
repo-task --json search "<keywords>"          # find document ids across all layers
repo-task --json convention <id-or-topic>     # architecture and stack rules
repo-task --json recipe <id-or-task>          # step-by-step playbook for a task
repo-task --json fact <family>                # curated project facts, e.g. viewmodels
repo-task --json fact                         # list the available fact families
repo-task --json symbol <name-or-regex>       # where a declaration lives
```

## Output contract

Every command returns `{"ok": true, "command": ..., "data": ..., "warnings": []}`.
On failure `ok` is `false` and `error.message` says what to do next. Read `data`;
do not parse the human-facing tables.

## Rules

- Prefer `brief` over reading knowledge documents one by one.
- Cite the document `id` you relied on when you explain a decision.
- A convention beats your default judgement. If you think a convention is wrong,
  say so explicitly instead of quietly ignoring it.
- If `brief` returns nothing useful, the knowledge base may be stale or unsynced:
  run `repo-task kb status` and report what you find rather than proceeding blind.
- Facts come from `repo-task index`. If a symbol you expect is missing, the index is
  stale — say so instead of concluding the code does not exist.
