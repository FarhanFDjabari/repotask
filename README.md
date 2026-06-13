# RepoTask

RepoTask is a standalone, provider-neutral developer workflow CLI. It creates deterministic local
task workspaces, composes project rules and workflows into AI-ready prompts, performs diff-aware
review preparation, and generates change-request descriptions while keeping implementation and
approval gates human-controlled.

## Status

Milestone 1 starts at version `0.1.0`. Python 3.11 or newer is required for source installs.
Tagged releases publish a self-contained macOS Apple Silicon binary. Linux and Windows binaries
can be built manually with the included GitHub Actions workflow or locally on the target platform.

## Project-Local Installation

Keep a separate RepoTask binary inside each project so projects can pin different versions without
installing RepoTask globally. From the target Git repository:

```bash
mkdir -p .repo-task-bin
printf '%s\n' '.repo-task-bin/' >> .git/info/exclude
```

Download `repo-task-v<version>-macos-arm64.tar.gz` from the GitHub release page and extract it:

```bash
tar -xzf ~/Downloads/repo-task-v0.1.0-macos-arm64.tar.gz -C .repo-task-bin
chmod +x .repo-task-bin/repo-task
./.repo-task-bin/repo-task --version
```

`.git/info/exclude` is local to the clone, so the binary neither appears in `git status` nor gets
committed. This is required because `repo-task start` intentionally requires a clean worktree.

If macOS blocks the downloaded binary, verify that it came from this repository release and remove
the quarantine attribute:

```bash
xattr -d com.apple.quarantine .repo-task-bin/repo-task
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

A `v*` tag automatically builds, smoke-tests, and publishes:

```text
repo-task-v<version>-macos-arm64.tar.gz
```

The macOS artifact is built on GitHub's `macos-14` arm64 runner. It is self-contained and does not
require Python on the user's machine. It is ad-hoc signed, but not Developer ID signed or notarized.

### Build Linux Or Windows With GitHub Actions

Linux and Windows builds do not run automatically:

1. Open the repository's **Actions** tab.
2. Select **Build Native Binary**.
3. Choose **Run workflow**.
4. Select `linux-x64`, `windows-x64`, or `all`.
5. Download the generated workflow artifact.

The manually generated artifacts are retained by GitHub Actions for 14 days:

```text
repo-task-linux-x64.tar.gz
repo-task-windows-x64.zip
```

Extract the selected binary into the target project's `.repo-task-bin/` directory and add that
directory to `.git/info/exclude` as shown above.

### Build Locally On A Target Platform

PyInstaller must run on the same operating system as the binary it produces:

```bash
python3.11 -m venv .build-venv
. .build-venv/bin/activate
python -m pip install -e '.[dev]'
pyinstaller --clean --noconfirm repotask.spec
```

On Linux and macOS, the result is `dist/repo-task`. On Windows, it is
`dist\repo-task.exe`. Run the smoke test before distributing it:

```bash
python scripts/binary_smoke.py dist/repo-task
```

On Windows:

```powershell
python scripts/binary_smoke.py dist\repo-task.exe
```

There is no self-update mechanism. Replace the project-local binary when changing versions.

## License

Apache-2.0.
