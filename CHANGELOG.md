# Changelog

## 0.2.4 - 2026-07-21

### MCP-only connectors are first class

A verb reachable only over MCP had to declare a `path` it would never call, so the
config carried a fake REST route to satisfy the schema — and `doctor` failed without
one. `path` is now optional: a verb declares `path` for a REST call, `mcp_tool` for one
the agent runs, and the config is rejected only when it declares neither. `mode: rest`
still requires a `path`, because there is nothing else for it to call.

`fetch` no longer stops at the four built-in drivers. A connector that declares a
`fetch` verb answers `repo-task fetch --system <name>`, and is picked automatically when
it is the only one configured; the ticket reaches the verb as `identifier`.

An MCP-mode connector no longer reports `REST call unavailable`. Nothing was attempted,
so the envelope states `"mode": "mcp"` with the reason `Connector configured for MCP
execution` and raises no fallback warning — the warning is for the case where the CLI
tried and could not.

### `design` reads any file, not only the configured one

`connectors.figma.project` names the project's own design file, so reading anything
else meant editing the config. `design file`, `node`, `variables`, and `image` now take
`--file`, which accepts a bare file key or a pasted link — the key sits in the same
position for every editor, so `figma.com/design/<key>/<name>` and its `/file/`,
`/board/`, `/slides/`, and `/proto/` siblings all work. A link that carries no key is
rejected rather than turned into a malformed URL.

The file also reaches the MCP fallback, which previously named no file at all and so
read whichever one the user had open. The hint now carries `fileKey`, the argument name
both the hosted server and the desktop bridge use. Arguments without a value are left
out instead of sent blank: `design file` had been emitting `"nodeId": ""`, which every
server declaring that argument declares non-empty.

### The MCP hint matches the server it is addressed to

Servers exposing the same Figma tools disagree on their arguments: the hosted server
takes a single `nodeId` everywhere, while a desktop bridge reads the current selection
and names nodes only on `get_screenshot`, as a list. An argument the server does not
declare is not refused — it is ignored, so the call succeeds against the wrong target.
Sending `nodeId` to the bridge left `nodeIds` unset, and a screenshot with no `nodeIds`
exports the current selection: whatever the user last clicked, rather than the node that
was asked for.

`connectors.figma.mcp_dialect` picks the shape: `figma` (the default) or `bridge`. The
CLI never connects to an MCP server, so it cannot discover the dialect and has to be
told; an unrecognized one is an error rather than a silent default.

`--depth` and `--scale` reach the bridge too, which declares both — they had been
dropped on the MCP path, so a hint rendered at the server's default rather than what
was asked for. The hosted server's nearest equivalent to `--scale` is `maxDimension`, a
pixel cap rather than a multiplier, so that dialect still sends neither. A `--scale`
that is not a number is now rejected before the hint is built.

## 0.2.3 - 2026-07-21

### SwiftUI views and components are indexed

Swift's grammar folds `struct`, `class`, `actor`, and `enum` into one
`class_declaration` node, so every Swift type was recorded as a `class`. SwiftUI
views and components are structs, which left them indistinguishable from reference
types and kept the `components` family (declared over other stacks) from reaching
them. The extractor now reads the leading keyword and reports `struct`, `actor`, and
`enum` as their own kinds. The `components`, `screens`, `viewmodels`, `repositories`,
and `usecases` families declare the `swiftui` stack and accept the new kinds where
they already accepted `class`.

An `extension` also parses as a `class_declaration`, and its `name` field is the
*extended* type — so it was being indexed as a phantom symbol under a borrowed name
(an `extension View` surfaced as a `View` screen). Extensions are no longer emitted;
declarations nested inside them still are.

## 0.2.2 - 2026-07-21

### Colorized human output

`doctor`, `search`, `symbol`, `fact`, `index`, and `convention`/`recipe` now use color
for headings, ids, and pass/fail state. Styling goes through `anstream`, which strips it
when stdout is not a terminal, so `--json`, piped output, and `NO_COLOR` stay plain. The
agent-facing JSON envelope is unchanged and byte-identical, because color lives only in
the human renderers, below the `output::emit` seam.

### Shell completions

`repo-task completions <shell>` prints a completion script for bash, zsh, fish, elvish,
or powershell. It is handled before command dispatch, since a completion script is shell
source and cannot be wrapped in the envelope.

### Build-size documentation

The README now carries measured per-feature binary sizes and notes that trimming
grammars is a download-size lever only: grammar tables are static pages touched solely
when a matching file is parsed, so every build starts in ~4 ms regardless of size.

## 0.2.1 - 2026-07-20

### Fact families apply to the project's stacks

`stacks` on a fact family was declared in the schema but never read, so it documented
an intent the CLI did not enforce. It now filters: a family whose `stacks` share
nothing with `project.stacks` is not built. An empty `stacks` still means every stack.

### Components are no longer web-only

The `components` family excluded mobile through `languages: [typescript, tsx,
javascript]`, so Compose and Flutter components were unreachable. That list is gone and
the mobile stacks are declared. Because PascalCase means "every class" outside
TypeScript, a `path_pattern` now carries the precision the language list had been
providing by accident.

## 0.2.0 - 2026-07-19

### Rewritten in Rust

RepoTask now ships as a single static binary. The previous Python implementation
required Python 3.10+ plus ~35 MB of dependencies; macOS ships 3.9, so system Python
no longer satisfied it. The binary removes the prerequisite entirely.

Startup was the stronger reason. An agent calls this CLI 10–40 times per session, so
interpreter startup is felt as latency (measured, median, same machine and fixtures):

| command | Rust | Python | |
| --- | --- | --- | --- |
| `brief` | 23.0 ms | 324.2 ms | 14.1x |
| `symbol` | 21.8 ms | 318.9 ms | 14.6x |
| `search` | 21.5 ms | 315.4 ms | 14.7x |
| `index` | 71.1 ms | 401.2 ms | 5.6x |

The JSON envelope is unchanged and verified identical to the Python implementation
across 18 command/budget/intent combinations, so generated skills keep working.

- Nine tree-sitter grammars are compiled in (~15 MB); cargo features trim the set
- Install is a downloaded binary or `cargo install`; the Python packaging is gone
- 113 tests: 56 unit and 57 integration

Rebuilt around a knowledge base the agent queries, replacing prompt-file generation.
This is a breaking change; run `repo-task migrate` to upgrade a 0.1 project.

### Added

- Three-layer knowledge base (conventions, project facts, recipes) sourced from a remote
  git repository or an in-project directory, with per-stack slices and token budgeting.
- `brief` — one ranked, budgeted context pack for a task; the command agents call first.
- `index`, `symbol`, `fact` — tree-sitter code indexing into curated fact families
  declared by the knowledge base (Kotlin, Swift, Dart, TypeScript, Python, Go, Rust, Java).
- `kb init|sync|status|propose` — scaffold, pin, inspect, and propose facts back by PR.
- Feature flow: `fetch`, `summarize`, `analyze`, `split`.
- Bugfix flow: `bug fetch`, `bug dedupe` (clusters tickets that resolve to the same code).
- Connectors for Jira, GitHub, GitLab, and ClickUp in `mcp` mode (the agent makes the call)
  or `rest` mode (the CLI does), with credentials outside the project.
- `skills sync` — generates Claude Code skills and an AGENTS.md block that reference
  commands only, so they never carry stale project knowledge.
- A stable `--json` envelope on every command.

### Changed

- Configuration moved to `.repo-task/config.yaml` at schema version 2.
- The CLI now installs globally (a downloaded binary or `cargo install`) rather than per project.

### Removed

- The `start`, `context`, `investigate`, `review`, `cr`, `status`, and `list` commands, along
  with the prompt, agent-assignment, rules, and template services they used.
- The portable zipapp build and its release artifact, along with the Python packaging.

## 0.1.1 - Unreleased

- Replaced the PyInstaller native binary with a portable single-file zipapp built by
  `scripts/build_portable.py`; the artifact is a few dozen KB and runs on any Python 3.9+ with no
  third-party dependencies.
- Removed all third-party runtime dependencies: the CLI now uses `argparse`, a built-in terminal
  helper, and a small YAML subset reader instead of Typer, Rich, and PyYAML.
- Accepted block-sequence YAML items aligned with their key (standard `key:` then `- item` at the
  same indent) in the configuration reader.
- Lowered the supported Python floor to 3.9 (the version bundled with current macOS) by replacing
  `datetime.UTC` and the 3.11-only `importlib.resources.abc` import; added a version guard so even
  older interpreters get a clear message instead of an import error.
- Switched pull-request CI to a cross-platform (Linux/macOS/Windows) portable-script smoke test and
  release publishing to `repo-task-<version>.tar.gz`.
- Fixed Python 3.11 bundled-resource loading.

## 0.1.0 - 2026-06-13

- Initial standalone RepoTask Milestone 1 implementation.
- Provider-neutral initialization, task workspaces, prompts, review, status, and CR workflows.
- GitLab and GitHub local CLI change-request creation.
- Project-local native distribution with automatic macOS arm64 releases.
- Manually dispatched GitHub Actions builds for Linux x64 and Windows x64.
