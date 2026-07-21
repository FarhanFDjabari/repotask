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

A single static binary — no runtime, no interpreter, nothing to keep on the PATH but the tool
itself. Download the archive for your platform from the
[releases page](https://github.com/FarhanFDjabari/repotask/releases) and put `repo-task` somewhere
on your `PATH`:

```bash
tar -xzf repo-task-v0.2.0-aarch64-apple-darwin.tar.gz
install -m 755 repo-task /usr/local/bin/
repo-task --version
```

On macOS the downloaded archive carries a quarantine flag, and the release binary is not yet
notarized — so the first run reports that Apple could not verify it. That is Gatekeeper declining to
check an unsigned binary, not a malware detection. Verify the archive against the checksum on the
releases page, then clear the flag:

```bash
xattr -d com.apple.quarantine /usr/local/bin/repo-task
```

Or build from source with a Rust toolchain:

```bash
cargo install --path .
```

Locally built binaries are never quarantined, so this path avoids the Gatekeeper prompt entirely.

Install once per machine. Each project keeps only a small config file and generated agent skills.

**Why a binary matters here.** An agent calls this CLI 10–40 times in a session, so startup cost is
felt as latency. `brief` returns in ~23 ms; the Python implementation it replaced took ~324 ms.

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
| Design | `design file`, `design node`, `design variables`, `design image`, `design map` |
| External systems | `connect <system> <verb>` |
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

Jira, GitHub, GitLab, and ClickUp are built in. **REST is the primary path**: the CLI makes the
call and distils the response, so the agent pays context for the *result* rather than the payload.
On a representative task payload that is 10,995 bytes down to 147. REST also reaches systems that
have no MCP server at all.

**MCP is the fallback** for when the CLI cannot make the call — no credential configured, no
network, a server that is down — because the agent's own connection may still succeed.

```yaml
connectors:
  jira:
    mode: auto            # auto (default) | rest | mcp | off
    base_url: https://acme.atlassian.net
    project: ACME
```

The rule for falling back: **no answer, not an unwelcome answer.** A 404 means the ticket does not
exist, and asking the agent to retry it spends tokens to reach the same place, so that surfaces as
an error. A missing credential or a 5xx falls back. `mode: rest` never falls back silently, and
every fallback is announced in the envelope's `warnings`.

Credentials come from the environment (`REPOTASK_JIRA_TOKEN`) or `~/.repo-task/secrets.yaml` —
never from the project repository.

### Any other system

Declare the call and the CLI will make it. No built-in connector needed, and no MCP server needed:

```yaml
connectors:
  acme:
    mode: auto
    base_url: https://api.acme.dev
    auth_header: Authorization
    auth_format: "Bearer {token}"
    verbs:
      task:
        path: /v1/task/{id}
        fields: [data.id, data.name, data.assignees.username]   # project the response
        mcp_tool: acme_get_task                                  # optional fallback
```

```bash
repo-task connect acme task --arg id=T-1
```

`fields` is what keeps the result small — dotted paths, and a path through an array maps over its
elements. Argument values are percent-encoded, so an argument cannot add path segments.

## Design

```bash
repo-task design file                 # distilled structure of the Figma file
repo-task design node 1:2 --depth 5   # one frame's structure
repo-task design variables            # design tokens
repo-task design image 1:2            # rendered URLs for the agent to look at
repo-task design map "Button/Primary" "FeedCard"
```

A raw Figma node tree is mostly transform matrices and vector geometry that say nothing about how
to build a screen. `design` keeps the frames, components, text, layout, and variables and drops the
rest. Images are the exception: the CLI cannot look at a frame, so it returns the rendered URL and
lets the agent view it.

`design map` is the part the agent cannot get from Figma — which of *this project's* components
already implements a given design component:

```text
Design component -> code
  PrimaryButton                ui/src/main/kotlin/com/acme/ui/Components.kt
  FeedCard                     ui/src/main/kotlin/com/acme/ui/Components.kt

No code found for: Checkout/PaymentSheet
```

Configure it with the Figma file key as `project`, and a token in `REPOTASK_FIGMA_TOKEN`. Without a
token it falls back to the Figma MCP server, which already holds the user's session.

`project` names the project's own design file. To read any other file you have access to, pass
`--file` with its key or a pasted link:

```bash
repo-task design file --file https://www.figma.com/design/AbC123XyZ890/Checkout-Flow
repo-task design node 1:2 --file AbC123XyZ890
```

The file carries into the MCP fallback as `fileKey`, so the agent reads the file you asked for
rather than whichever one is open.

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
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
```

Tests build real git repositories in a temp directory, so the remote knowledge-base path — clone,
pin, fetch — is covered without touching the network.

### Language support and binary size

The default build carries nine tree-sitter grammars (Kotlin, Java, Swift, Dart, TypeScript/TSX,
JavaScript, Python, Go, Rust) and weighs about 15 MB — the grammars are 83% of that. They sit
behind cargo features if you want a smaller binary:

```bash
cargo build --release --no-default-features                     # 2.6 MB, no grammars
cargo build --release --no-default-features --features backend  # 4.4 MB
cargo build --release --no-default-features --features web      # 5.7 MB
cargo build --release --no-default-features --features web,backend  # 7.4 MB
cargo build --release --no-default-features --features mobile   # 10.7 MB
```

`mobile` (Kotlin, Swift, Dart, Java) accounts for 8 MB on its own, so dropping it is most of the
saving on a web or backend project.

Size is a download-time concern only. Grammar tables are static pages that are never touched
unless a file of that language is parsed, so a 2.6 MB build and a 15.4 MB build start in the same
~4 ms — trimming grammars will not make the CLI answer an agent faster.

A build without a grammar still runs; it warns and skips files in that language rather than
pretending they contain nothing.

## License

Apache-2.0.
