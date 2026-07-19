# Changelog

## 0.2.0 - Unreleased

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

- Configuration moved to `.repo-task/config.yaml` at schema version 2, validated with pydantic.
- The CLI now installs globally (`uv tool install` / `pipx install`) rather than per project;
  Python 3.10+ is required and third-party dependencies are used again.

### Removed

- The `start`, `context`, `investigate`, `review`, `cr`, `status`, and `list` commands, along
  with the prompt, agent-assignment, rules, and template services they used.
- The portable zipapp build and its release artifact.

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
