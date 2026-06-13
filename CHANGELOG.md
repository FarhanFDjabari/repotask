# Changelog

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
