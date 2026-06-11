# RepoTask

RepoTask is a standalone, provider-neutral developer workflow CLI. It creates deterministic local
task workspaces, composes project rules and workflows into AI-ready prompts, performs diff-aware
review preparation, and generates change-request descriptions while keeping implementation and
approval gates human-controlled.

## Status

Milestone 1 starts at version `0.1.0`. Python 3.11 or newer is required for source installs.
User-facing releases are distributed as one universal pure-Python wheel attached to the
repository release.

## Install From A Repository Release

Download `repotask-<version>-py3-none-any.whl` from the GitHub or GitLab release page, then install
it as an isolated command-line tool:

```bash
pipx install ./repotask-0.1.0-py3-none-any.whl
```

Alternatively, use uv:

```bash
uv tool install ./repotask-0.1.0-py3-none-any.whl
```

For a public GitHub release, `pipx` can install the wheel directly from its release URL:

```bash
pipx install \
  https://github.com/OWNER/repotask/releases/download/v0.1.0/repotask-0.1.0-py3-none-any.whl
```

For a private repository, download the authenticated release asset first, then install the local
wheel. The installed command is available as:

```bash
repo-task --version
```

## Install From Source

```bash
python3.11 -m pip install .
repo-task --version
```

For development:

```bash
python3.11 -m venv .venv
. .venv/bin/activate
pip install -e '.[dev]'
pytest
```

## Initialize

Run from any directory inside a Git repository:

```bash
repo-task init
repo-task init --dry-run
repo-task init --answers setup.yml
repo-task init --non-interactive --answers setup.yml
repo-task init --force
repo-task doctor
```

Setup explicitly confirms stacks, VCS provider, task provider, branch policy, workflow, CR
template, rules, and agents. Selected project assets are copied under `.repo-task/`; originals are
never modified. Only `.repo-task/tasks/` is added to `.gitignore`.

An answer file can use:

```yaml
project:
  name: example
  stacks: [python]
  base_branch: main
branch:
  feature_pattern: "feature/{task_id}-{slug}"
vcs:
  provider: github
task_provider:
  provider: jira
  display_name: Jira issue
  url_pattern: "https://example.atlassian.net/browse/{task_id}"
  connector_hint: jira
workflow:
  mode: bundled
assets:
  workflow: []
  template: ""
  rules: []
  agents: []
bundled_defaults: true
```

Task providers are always explicit. RepoTask does not infer them from task IDs and does not store
provider API keys or call provider APIs.

## Workflow

```bash
repo-task start TASK-123 --title "Fix checkout" --mode human
repo-task context TASK-123 --paste
repo-task context TASK-123 --from-file requirement.md
repo-task context TASK-123 --prompt
repo-task investigate TASK-123
repo-task review TASK-123
repo-task review TASK-123 --worktree
repo-task cr TASK-123
repo-task cr TASK-123 --create
repo-task cr TASK-123 --create --draft
repo-task status TASK-123
repo-task status TASK-123 --provider
repo-task list
```

Runtime files live under `.repo-task/tasks/<TASK_ID>/`. GitLab creation uses `glab mr create`;
GitHub creation uses `gh pr create`. Bitbucket and local/custom configurations generate
descriptions only.

RepoTask never merges, approves, changes task status, resolves conflicts, deploys, releases, or
makes QA decisions automatically.

## Releases

A `v*` tag builds `repotask-<version>-py3-none-any.whl` once, verifies that it is platform-neutral,
smoke-installs it, and attaches it to the matching GitHub or GitLab repository release. The same
wheel works on macOS, Linux, and Windows with Python 3.11 or newer.

No public package registry, platform-specific binary, signing certificate, notarization, or
self-update mechanism is required.

## License

Apache-2.0.
