"""Project setup: `init`, `doctor`, `migrate`."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import typer
from rich.console import RenderableType
from rich.table import Table

from repotask.config import dump_yaml, load_config, load_yaml_mapping
from repotask.config.loader import CONFIG_PATH, LEGACY_CONFIG_PATH, config_path
from repotask.config.models import SUPPORTED_SCHEMA_VERSION
from repotask.discovery import discover_project
from repotask.errors import RepoTaskError
from repotask.files import write_text
from repotask.git import resolve_git_root
from repotask.kb import resolve
from repotask.kb.store import load
from repotask.output import emit, guard

GITIGNORE_ENTRIES = (".repo-task/work/", ".repo-task/cache/")


def _config_document(
    name: str, stacks: list[str], base_branch: str, remote: str, local: str
) -> dict[str, Any]:
    knowledge: dict[str, Any] = {"ref": "main"}
    if remote:
        knowledge["remote"] = remote
    else:
        knowledge["local"] = local
    return {
        "schema_version": SUPPORTED_SCHEMA_VERSION,
        "project": {"name": name, "stacks": stacks, "base_branch": base_branch},
        "knowledge": knowledge,
        "connectors": {},
    }


@guard
def init(
    remote: str = typer.Option(
        "", "--remote", help="Knowledge base git URL. Omit to use an in-project directory."
    ),
    local: str = typer.Option(
        ".repo-task/knowledge", "--local", help="In-project knowledge base directory."
    ),
    stacks: list[str] = typer.Option(
        [], "--stack", help="Override detected stacks. Repeatable."
    ),
    force: bool = typer.Option(False, "--force", help="Overwrite an existing configuration."),
    dry_run: bool = typer.Option(False, "--dry-run", help="Preview without writing."),
) -> None:
    """Create `.repo-task/config.yaml` from repository discovery."""
    discovery = discover_project()
    path = config_path(discovery.root)
    if path.exists() and not force and not dry_run:
        raise RepoTaskError(f"{CONFIG_PATH} already exists. Pass --force to overwrite.")

    document = _config_document(
        name=discovery.project_name,
        stacks=list(stacks) or discovery.stacks,
        base_branch=discovery.base_branch,
        remote=remote,
        local=local,
    )
    content = dump_yaml(document)
    created: list[str] = []
    if not dry_run:
        write_text(path, content)
        created.append(CONFIG_PATH)
        if not remote:
            (discovery.root / local).mkdir(parents=True, exist_ok=True)
            created.append(local)
        created.extend(_ensure_gitignore(discovery.root))

    data = {
        "root": str(discovery.root),
        "config": document,
        "created": created,
        "dryRun": dry_run,
        "nextSteps": _next_steps(bool(remote)),
    }
    emit("init", data, lambda payload: _render_init(payload, content))


def _render_init(data: dict[str, Any], content: str) -> RenderableType:
    prefix = "Would write" if data["dryRun"] else "Wrote"
    steps = "\n".join(f"  {index}. {step}" for index, step in enumerate(data["nextSteps"], 1))
    return f"{prefix} {CONFIG_PATH}\n\n{content}\nNext:\n{steps}"


def _next_steps(has_remote: bool) -> list[str]:
    steps = ["repo-task kb sync"]
    if not has_remote:
        steps.insert(0, "Add conventions/, recipes/, slices/ and kb.yaml to the knowledge dir")
    steps.extend(["repo-task index", "repo-task skills sync"])
    return steps


def _ensure_gitignore(root: Path) -> list[str]:
    path = root / ".gitignore"
    existing = path.read_text(encoding="utf-8") if path.is_file() else ""
    missing = [entry for entry in GITIGNORE_ENTRIES if entry not in existing.splitlines()]
    if not missing:
        return []
    separator = "" if existing.endswith("\n") or not existing else "\n"
    path.write_text(existing + separator + "\n".join(missing) + "\n", encoding="utf-8")
    return [".gitignore"]


@guard
def doctor() -> None:
    """Check configuration, knowledge base reachability, and tooling."""
    checks: list[dict[str, Any]] = []

    def record(name: str, ok: bool, detail: str) -> None:
        checks.append({"name": name, "ok": ok, "detail": detail})

    try:
        config = load_config()
        record("config", True, str(config_path(config.root)))
    except RepoTaskError as error:
        record("config", False, str(error))
        emit("doctor", {"ok": False, "checks": checks}, _render_doctor)
        raise typer.Exit(1) from error

    record("stacks", True, ", ".join(config.project.stacks))
    try:
        kb = load(resolve(config, sync=False))
        record("knowledge", True, f"{kb.source.kind}: {kb.source.path}")
        record(
            "layers",
            bool(kb.conventions or kb.recipes),
            f"{len(kb.conventions)} conventions, {len(kb.recipes)} recipes, "
            f"{len(kb.fact_families())} fact families",
        )
        record(
            "slices",
            bool(kb.slices),
            ", ".join(sorted(kb.slices)) or "no slices/*.yaml found",
        )
    except RepoTaskError as error:
        record("knowledge", False, str(error))

    ok = all(check["ok"] for check in checks)
    emit("doctor", {"ok": ok, "checks": checks}, _render_doctor)
    if not ok:
        raise typer.Exit(1)


def _render_doctor(data: dict[str, Any]) -> RenderableType:
    table = Table(title="repo-task doctor")
    table.add_column("check")
    table.add_column("status")
    table.add_column("detail", overflow="fold")
    for check in data["checks"]:
        status = "[green]ok[/green]" if check["ok"] else "[red]fail[/red]"
        table.add_row(check["name"], status, check["detail"])
    return table


@guard
def migrate(
    dry_run: bool = typer.Option(False, "--dry-run", help="Preview without writing."),
) -> None:
    """Upgrade a schema version 1 `.repo-task.yml` to `.repo-task/config.yaml`."""
    root = resolve_git_root()
    legacy = root / LEGACY_CONFIG_PATH
    if not legacy.is_file():
        raise RepoTaskError(f"No {LEGACY_CONFIG_PATH} found at {root}; nothing to migrate.")
    target = config_path(root)
    if target.exists() and not dry_run:
        raise RepoTaskError(f"{CONFIG_PATH} already exists; remove it before migrating.")

    old = load_yaml_mapping(legacy)
    project = old.get("project", {})
    document = _config_document(
        name=str(project.get("name") or root.name),
        stacks=list(project.get("stacks") or ["generic"]),
        base_branch=str(project.get("base_branch") or "main"),
        remote="",
        local=".repo-task/knowledge",
    )
    content = dump_yaml(document)
    if not dry_run:
        write_text(target, content)

    v1_sections = {
        "vcs",
        "task_provider",
        "workflow",
        "rules",
        "change_request",
        "agents",
        "branch",
    }
    dropped = sorted(key for key in old if key in v1_sections)
    data = {
        "from": LEGACY_CONFIG_PATH,
        "to": CONFIG_PATH,
        "config": document,
        "droppedSections": dropped,
        "dryRun": dry_run,
        "nextSteps": [
            f"Review and delete {LEGACY_CONFIG_PATH}",
            "Move your rules/*.md into the knowledge base as conventions/",
            "repo-task kb sync",
        ],
    }
    emit(
        "migrate",
        data,
        lambda payload: (
            f"{'Would migrate' if payload['dryRun'] else 'Migrated'} {payload['from']} -> "
            f"{payload['to']}\n\n{content}\n"
            f"Dropped v1 sections (now knowledge-base concerns): "
            f"{', '.join(payload['droppedSections']) or 'none'}"
        ),
    )
