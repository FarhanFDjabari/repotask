# RepoTask

RepoTask gives AI coding agents a cheap, current, reviewed answer to "how does *this* project do
things?" — without pasting your architecture rules into every prompt.

Your project's conventions, task playbooks, and code index live in a knowledge base. The agent
calls `repo-task`, the CLI returns only the slice that applies to the task at hand, and the agent
implements against it. The CLI never calls a language model: it locates, parses, budgets, and
returns JSON.

```text
you ──▶ agent ──▶ repo-task brief "add pagination" ──▶ ┌─ conventions  (how we build)
                        │                              ├─ project facts (what exists)
                        │                              └─ recipes      (how we do this task)
                        ◀── one budgeted context pack ──┘
```

## Why

Static rule files get pasted in whole, cost tokens on every turn, and go stale. RepoTask splits
project knowledge into three layers, keeps the generated layer honest with a code index, and hands
the agent a ranked, token-budgeted subset. An Android task never pays for the iOS conventions.

## Install

```bash
uv tool install repotask     # or: pipx install repotask
repo-task --version
```

Install once per machine. Each project keeps only a small config file and generated agent skills.

## Quick start

```bash
cd your-project
repo-task init                  # detect stacks, write .repo-task/config.yaml
repo-task kb init               # scaffold a starter knowledge base
repo-task index                 # index the code into project facts
repo-task skills sync           # generate agent skills + AGENTS.md
repo-task doctor                # verify the setup
```

Then ask your agent to do something. It will call the CLI on its own.

## The knowledge base

Three layers, one schema, stored either **in the project** or in a **shared git repository** so
several projects reuse the same standards and changes go through review.

| Layer | Holds | Written by |
| --- | --- | --- |
| `conventions/` | Architecture and stack decisions: which libraries, what layering, module boundaries | Humans |
| `facts/` | What actually exists: modules, viewmodels, routes, repositories, symbols | `repo-task index` |
| `recipes/` | Playbooks: how to add a screen, implement pagination, name things | Humans |

`slices/<stack>.yaml` decides which documents apply to which stack and intent, so the CLI can rank
and trim before the agent ever sees them.

```yaml
# .repo-task/config.yaml
schema_version: 2
project:
  name: acme-app
  stacks: [android, kotlin]
  base_branch: main
knowledge:
  remote: https://git.example.com/acme/mobile-kb   # shared, reviewed via PR
  ref: main
  # local: .repo-task/knowledge                    # or committed with the project
```

A remote knowledge base is cloned to `~/.repo-task/kb/<slug>` and pinned to `ref`. Regenerated
facts are proposed back as a branch — `repo-task kb propose` commits and prints the push command,
but never pushes for you.

## Commands

Every command takes `--json` and returns the same envelope:

```json
{"schema": "repotask.v2", "ok": true, "command": "brief", "data": {}, "warnings": []}
```

| Group | Commands |
| --- | --- |
| Setup | `init`, `kb init`, `doctor`, `migrate`, `skills sync` |
| Knowledge | `kb sync`, `kb status`, `kb propose` |
| Query | `brief`, `search`, `convention`, `recipe`, `fact`, `symbol` |
| Index | `index [--changed-only]` |
| Feature flow | `fetch`, `summarize`, `analyze`, `split` |
| Bugfix flow | `bug fetch`, `bug dedupe` |

### Feature flow

`fetch → summarize → analyze → split` turns a ticket into reviewable steps. Each step gathers
evidence and states a contract; the agent supplies the judgement.

```bash
repo-task fetch ACME-142         # pull the ticket or PRD locally
repo-task summarize ACME-142     # filter it down to what this project's stacks care about
repo-task analyze ACME-142       # impact set from the code index, plus an effort rubric
repo-task split ACME-142         # incremental steps, data layer before UI
```

### Bugfix flow

```bash
repo-task bug fetch BUG-88       # report + the code it implicates + the applicable conventions
repo-task bug dedupe             # cluster tickets that resolve to the same code
```

`bug dedupe` reports overlap as evidence — shared files, shared symbols, a similarity score. It
does not close or merge anything; a human decides.

## Connectors

Jira, GitHub, GitLab, and ClickUp, in one of two modes per system:

- **`mcp`** (default) — the CLI returns the tool call and the *agent* makes it through its own
  connection. No credentials in the CLI.
- **`rest`** — the CLI calls the API itself. Cheaper, and works without an agent connection.

```yaml
connectors:
  jira:
    mode: rest
    base_url: https://acme.atlassian.net
    project: ACME
```

REST credentials come from the environment (`REPOTASK_JIRA_TOKEN`) or `~/.repo-task/secrets.yaml`
— never from the project repository.

## Agent skills

`repo-task skills sync` writes thin skills that reference commands and contain **no** project
knowledge, so they never go stale:

```text
.claude/skills/repotask-search/SKILL.md    lookups: conventions, recipes, facts, symbols
.claude/skills/repotask-feature/SKILL.md   ticket to reviewable increments
.claude/skills/repotask-bugfix/SKILL.md    triage and duplicate detection
.claude/skills/repotask-review/SKILL.md    review a diff against project conventions
AGENTS.md                                  portable block for any other harness
```

Hand-written content in `AGENTS.md` is preserved; only the marked block is regenerated.

## Upgrading from 0.1

```bash
repo-task migrate
```

Version 2 replaces prompt-file generation with the knowledge base. The v1 `start`, `context`,
`investigate`, `review`, and `cr` commands are gone; `migrate` converts the config and tells you
which sections became knowledge-base concerns. Move your old `rules/*.md` into `conventions/`.

## Development

```bash
python3 -m venv .venv && . .venv/bin/activate
pip install -e '.[dev]'
pytest
ruff check .
```

## License

Apache-2.0.
