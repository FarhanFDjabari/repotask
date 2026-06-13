# RepoTask

RepoTask is a standalone, provider-neutral developer workflow CLI. It creates deterministic local
task workspaces, composes project rules and workflows into AI-ready prompts, performs diff-aware
review preparation, and generates change-request descriptions while keeping implementation and
approval gates human-controlled.

## Status

Milestone 1 starts at version `0.1.0`. RepoTask ships as a single portable Python file with no
third-party dependencies. The only requirement is Python 3.9 or newer already on the machine —
the version bundled with current macOS satisfies this, so no extra install is needed.

## Project-Local Installation

`repo-task` is a self-contained Python [zipapp](https://docs.python.org/3/library/zipapp.html): one
executable file of a few dozen KB. Keep a copy inside each project so projects can pin different
versions without installing RepoTask globally. From the target Git repository:

```bash
mkdir -p .repo-task-bin
printf '%s\n' '.repo-task-bin/' >> .git/info/exclude
```

Download `repo-task-v<version>.tar.gz` from the GitHub release page and extract it:

```bash
tar -xzf ~/Downloads/repo-task-v0.1.0.tar.gz -C .repo-task-bin
chmod +x .repo-task-bin/repo-task
./.repo-task-bin/repo-task --version
```

The file runs through the system `python3` via its shebang. To pin a specific interpreter, run it
explicitly:

```bash
python3 .repo-task-bin/repo-task --version
```

`.git/info/exclude` is local to the clone, so the file neither appears in `git status` nor gets
committed. This is required because `repo-task start` intentionally requires a clean worktree.

## Install From Source

```bash
python3 -m pip install .
repo-task --version
```

For development:

```bash
python3 -m venv .venv
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

A `v*` tag automatically builds, smoke-tests, and publishes one cross-platform artifact:

```text
repo-task-v<version>.tar.gz
```

It contains the portable `repo-task` zipapp. The same file runs on Linux, macOS, and Windows; the
only requirement is Python 3.9 or newer on the user's machine.

### Build Locally

The build is pure standard library and works on any platform:

```bash
python3 scripts/build_portable.py
```

The result is `dist/repo-task`. Run the smoke test before distributing it:

```bash
python3 scripts/binary_smoke.py dist/repo-task
```

On Windows the file is invoked through `python`:

```powershell
python scripts\binary_smoke.py dist\repo-task
```

There is no self-update mechanism. Replace the project-local file when changing versions.

## License

Apache-2.0.
