"""Environment and repository diagnostics."""

from __future__ import annotations

import shutil

from rich.console import Console
from rich.table import Table

from repotask.config.loader import load_config
from repotask.errors import RepoTaskError
from repotask.git import resolve_git_root


def run_doctor() -> bool:
    table = Table(title="RepoTask doctor")
    table.add_column("Check")
    table.add_column("Result")
    ok = True
    try:
        root = resolve_git_root()
        table.add_row("Git repository", f"ok: {root}")
    except RepoTaskError as error:
        table.add_row("Git repository", f"failed: {error}")
        Console().print(table)
        return False
    for executable in ("git",):
        found = shutil.which(executable)
        ok = ok and bool(found)
        table.add_row(executable, f"ok: {found}" if found else "missing")
    try:
        config = load_config(root)
        table.add_row("Configuration", f"ok: schema {config.schema_version}")
        for relative in (
            config.rules.files
            + config.workflow.documents
            + list(config.agents.roles.values())
            + ([config.change_request.template] if config.change_request.template else [])
        ):
            exists = (root / relative).is_file()
            ok = ok and exists
            table.add_row(relative, "ok" if exists else "missing")
        if config.vcs.create_enabled:
            executable = config.vcs.adapter
            found = shutil.which(executable)
            table.add_row(f"CR adapter ({executable})", "ok" if found else "missing")
    except RepoTaskError as error:
        ok = False
        table.add_row("Configuration", f"failed: {error}")
    manifest = root / ".repo-task/setup-manifest.json"
    table.add_row("Setup manifest", "ok" if manifest.is_file() else "missing")
    Console().print(table)
    return ok

