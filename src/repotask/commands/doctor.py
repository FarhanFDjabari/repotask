"""Environment and repository diagnostics."""

from __future__ import annotations

import shutil

from repotask.config.loader import load_config
from repotask.errors import RepoTaskError
from repotask.git import resolve_git_root
from repotask.terminal import print_table


def run_doctor() -> bool:
    rows = []
    ok = True
    try:
        root = resolve_git_root()
        rows.append(("Git repository", f"ok: {root}"))
    except RepoTaskError as error:
        rows.append(("Git repository", f"failed: {error}"))
        print_table("RepoTask doctor", ("Check", "Result"), rows)
        return False
    for executable in ("git",):
        found = shutil.which(executable)
        ok = ok and bool(found)
        rows.append((executable, f"ok: {found}" if found else "missing"))
    try:
        config = load_config(root)
        rows.append(("Configuration", f"ok: schema {config.schema_version}"))
        for relative in (
            config.rules.files
            + config.workflow.documents
            + list(config.agents.roles.values())
            + ([config.change_request.template] if config.change_request.template else [])
        ):
            exists = (root / relative).is_file()
            ok = ok and exists
            rows.append((relative, "ok" if exists else "missing"))
        if config.vcs.create_enabled:
            executable = config.vcs.adapter
            found = shutil.which(executable)
            rows.append((f"CR adapter ({executable})", "ok" if found else "missing"))
    except RepoTaskError as error:
        ok = False
        rows.append(("Configuration", f"failed: {error}"))
    manifest = root / ".repo-task/setup-manifest.json"
    rows.append(("Setup manifest", "ok" if manifest.is_file() else "missing"))
    print_table("RepoTask doctor", ("Check", "Result"), rows)
    return ok
