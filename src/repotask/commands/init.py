"""Repository initialization and setup wizard."""

from __future__ import annotations

import hashlib
import json
import shutil
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import yaml
from prompt_toolkit import prompt
from rich.console import Console
from rich.table import Table

from repotask.config.writer import dump_yaml
from repotask.discovery.assets import normalized_agent_role, validate_import
from repotask.discovery.project import ProjectDiscovery, discover_project
from repotask.errors import RepoTaskError
from repotask.resources import read_bundled

STANDARD_ROLES = [
    "requirement-analyst",
    "code-investigator",
    "implementation-planner",
    "diff-reviewer",
    "qa-planner",
    "cr-writer",
]
MANUAL_GATES = [
    "requirement-approval",
    "qa-decision",
    "merge-approval",
    "conflict-resolution",
    "deployment-approval",
    "release-approval",
]
PROVIDER_DEFAULTS = {
    "manual": ("Task", "", ""),
    "clickup": ("ClickUp task", "https://app.clickup.com/t/{task_id}", "clickup"),
    "jira": ("Jira issue", "", "jira"),
    "linear": ("Linear issue", "", "linear"),
    "github-issues": ("GitHub issue", "", "github"),
    "gitlab-issues": ("GitLab issue", "", "gitlab"),
    "azure-devops": ("Azure DevOps work item", "", "azure-devops"),
    "custom": ("Task", "", ""),
}
VCS_DEFAULTS = {
    "gitlab": ("merge-request", True, "glab"),
    "github": ("pull-request", True, "gh"),
    "bitbucket": ("pull-request", False, "local"),
    "other": ("change-request", False, "local"),
}


@dataclass(frozen=True)
class PlannedWrite:
    destination: str
    content: str
    source: str
    category: str
    ownership: str = "owned"

    @property
    def digest(self) -> str:
        return hashlib.sha256(self.content.encode("utf-8")).hexdigest()


def _load_answers(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {}
    if not path.exists():
        raise RepoTaskError(f"Answers file not found: {path}")
    try:
        data = yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as error:
        raise RepoTaskError(f"Could not parse answers file {path}: {error}") from error
    if not isinstance(data, dict):
        raise RepoTaskError(f"Answers file {path} must contain a YAML mapping.")
    return data


def _answer(data: dict[str, Any], path: str, default: Any = None) -> Any:
    current: Any = data
    for part in path.split("."):
        if not isinstance(current, dict) or part not in current:
            return default
        current = current[part]
    return current


def _csv(value: str) -> list[str]:
    return [item.strip().lower().replace(" ", "-") for item in value.split(",") if item.strip()]


def _paths(discovery: ProjectDiscovery, category: str) -> list[str]:
    return [asset.relative_path for asset in discovery.assets if asset.category == category]


def _interactive_answers(discovery: ProjectDiscovery, seed: dict[str, Any]) -> dict[str, Any]:
    Console().print("[bold]RepoTask setup[/bold] (press Enter to accept each recommendation)")
    project_name = prompt(
        "Project name: ", default=str(_answer(seed, "project.name", discovery.project_name))
    )
    stacks_default = ",".join(_answer(seed, "project.stacks", discovery.stacks))
    stacks = _csv(prompt("Stacks (comma separated): ", default=stacks_default))
    base_branch = prompt(
        "Base branch: ", default=str(_answer(seed, "project.base_branch", discovery.base_branch))
    )
    branch_pattern = prompt(
        "Feature branch pattern: ",
        default=str(_answer(seed, "branch.feature_pattern", "feature/{task_id}-{slug}")),
    )
    vcs_provider = prompt(
        "VCS provider [gitlab/github/bitbucket/other]: ",
        default=str(_answer(seed, "vcs.provider", discovery.vcs_provider)),
    ).strip().lower()
    task_provider = prompt(
        "Task provider [manual/clickup/jira/linear/github-issues/gitlab-issues/"
        "azure-devops/custom]: ",
        default=str(_answer(seed, "task_provider.provider", "manual")),
    ).strip().lower()
    display_name, url_pattern, connector_hint = PROVIDER_DEFAULTS.get(
        task_provider, PROVIDER_DEFAULTS["custom"]
    )
    display_name = prompt(
        "Task provider display name: ",
        default=str(_answer(seed, "task_provider.display_name", display_name)),
    )
    url_pattern = prompt(
        "Task URL pattern (optional, use {task_id}): ",
        default=str(_answer(seed, "task_provider.url_pattern", url_pattern)),
    )
    connector_hint = prompt(
        "Read-only connector hint (optional): ",
        default=str(_answer(seed, "task_provider.connector_hint", connector_hint)),
    )
    workflow_mode = prompt(
        "Workflow [imported/bundled/combined/none]: ",
        default=str(
            _answer(
                seed,
                "workflow.mode",
                "combined" if _paths(discovery, "workflow") else "bundled",
            )
        ),
    ).strip().lower()
    workflow_assets = _csv_paths(
        prompt(
            "Workflow files (comma separated): ",
            default=",".join(_answer(seed, "assets.workflow", _paths(discovery, "workflow"))),
        )
    )
    template = prompt(
        "CR template (optional): ",
        default=str(
            _answer(
                seed,
                "assets.template",
                (_paths(discovery, "templates") or [""])[0],
            )
        ),
    ).strip()
    rules = _csv_paths(
        prompt(
            "Rules/instruction files (comma separated): ",
            default=",".join(_answer(seed, "assets.rules", _paths(discovery, "rules"))),
        )
    )
    agents = _csv_paths(
        prompt(
            "Agent files (comma separated): ",
            default=",".join(_answer(seed, "assets.agents", _paths(discovery, "agents"))),
        )
    )
    bundled_defaults = prompt(
        "Supplement missing assets with bundled defaults? [Y/n]: ",
        default="y" if _answer(seed, "bundled_defaults", True) else "n",
    ).strip().lower() not in {"n", "no"}
    return {
        "project": {"name": project_name, "stacks": stacks, "base_branch": base_branch},
        "branch": {"feature_pattern": branch_pattern},
        "vcs": {"provider": vcs_provider},
        "task_provider": {
            "provider": task_provider,
            "display_name": display_name,
            "url_pattern": url_pattern,
            "connector_hint": connector_hint,
        },
        "workflow": {"mode": workflow_mode},
        "assets": {
            "workflow": workflow_assets,
            "template": template,
            "rules": rules,
            "agents": agents,
        },
        "bundled_defaults": bundled_defaults,
    }


def _csv_paths(value: str) -> list[str]:
    return [item.strip() for item in value.split(",") if item.strip()]


def _normalized_answers(
    discovery: ProjectDiscovery, answers: dict[str, Any], non_interactive: bool
) -> dict[str, Any]:
    if not non_interactive:
        return _interactive_answers(discovery, answers)
    provider = _answer(answers, "task_provider.provider")
    if not provider:
        raise RepoTaskError(
            "Non-interactive initialization requires task_provider.provider in the answers file. "
            "RepoTask never infers a task provider from task IDs."
        )
    provider_defaults = PROVIDER_DEFAULTS.get(str(provider), PROVIDER_DEFAULTS["custom"])
    vcs_provider = str(_answer(answers, "vcs.provider", discovery.vcs_provider))
    return {
        "project": {
            "name": str(_answer(answers, "project.name", discovery.project_name)),
            "stacks": list(_answer(answers, "project.stacks", discovery.stacks)),
            "base_branch": str(
                _answer(answers, "project.base_branch", discovery.base_branch)
            ),
        },
        "branch": {
            "feature_pattern": str(
                _answer(answers, "branch.feature_pattern", "feature/{task_id}-{slug}")
            )
        },
        "vcs": {"provider": vcs_provider},
        "task_provider": {
            "provider": str(provider),
            "display_name": str(
                _answer(answers, "task_provider.display_name", provider_defaults[0])
            ),
            "url_pattern": str(
                _answer(answers, "task_provider.url_pattern", provider_defaults[1])
            ),
            "connector_hint": str(
                _answer(answers, "task_provider.connector_hint", provider_defaults[2])
            ),
        },
        "workflow": {
            "mode": str(
                _answer(
                    answers,
                    "workflow.mode",
                    "combined" if _paths(discovery, "workflow") else "bundled",
                )
            )
        },
        "assets": {
            "workflow": list(
                _answer(answers, "assets.workflow", _paths(discovery, "workflow"))
            ),
            "template": str(_answer(answers, "assets.template", "")),
            "rules": list(_answer(answers, "assets.rules", _paths(discovery, "rules"))),
            "agents": list(_answer(answers, "assets.agents", _paths(discovery, "agents"))),
        },
        "bundled_defaults": bool(_answer(answers, "bundled_defaults", True)),
    }


def _import_destination(category: str, relative: str) -> str:
    return f".repo-task/{category}/imported/{relative}"


def _import_write(
    discovery: ProjectDiscovery, category: str, relative: str
) -> PlannedWrite:
    _source, content = validate_import(discovery.root, relative)
    return PlannedWrite(
        destination=_import_destination(category, relative),
        content=content if content.endswith("\n") else content + "\n",
        source=relative,
        category=category,
    )


def _build_plan(discovery: ProjectDiscovery, answers: dict[str, Any]) -> list[PlannedWrite]:
    stacks = [str(value).lower().replace(" ", "-") for value in answers["project"]["stacks"]]
    vcs_provider = answers["vcs"]["provider"]
    task_provider = answers["task_provider"]
    workflow_mode = answers["workflow"]["mode"]
    if task_provider["provider"] not in PROVIDER_DEFAULTS:
        raise RepoTaskError(f"Unsupported task provider: {task_provider['provider']}")
    if vcs_provider not in VCS_DEFAULTS:
        raise RepoTaskError(f"Unsupported VCS provider: {vcs_provider}")
    if workflow_mode not in {"imported", "bundled", "combined", "none"}:
        raise RepoTaskError(f"Unsupported workflow mode: {workflow_mode}")

    writes: list[PlannedWrite] = []
    rule_files = [".repo-task/rules/bundled/common.md"]
    writes.append(
        PlannedWrite(
            rule_files[0], read_bundled("rules", "common.md"), "bundled:rules/common.md", "rules"
        )
    )
    for stack in stacks:
        try:
            content = read_bundled("rules", f"{stack}.md")
        except FileNotFoundError:
            raise RepoTaskError(f"Unsupported stack profile: {stack}") from None
        destination = f".repo-task/rules/bundled/{stack}.md"
        rule_files.append(destination)
        writes.append(
            PlannedWrite(destination, content, f"bundled:rules/{stack}.md", "rules")
        )

    for relative in answers["assets"]["rules"]:
        write = _import_write(discovery, "rules", relative)
        writes.append(write)
        rule_files.append(write.destination)

    workflow_files: list[str] = []
    if workflow_mode in {"bundled", "combined"}:
        destination = ".repo-task/workflow/bundled/generic.md"
        writes.append(
            PlannedWrite(
                destination,
                read_bundled("workflows", "generic.md"),
                "bundled:workflows/generic.md",
                "workflow",
            )
        )
        workflow_files.append(destination)
    if workflow_mode in {"imported", "combined"}:
        for relative in answers["assets"]["workflow"]:
            write = _import_write(discovery, "workflow", relative)
            writes.append(write)
            workflow_files.append(write.destination)

    template_source = answers["assets"]["template"]
    if template_source:
        template_write = _import_write(discovery, "templates", template_source)
    else:
        template_name = vcs_provider if vcs_provider in {"gitlab", "github"} else "generic"
        template_write = PlannedWrite(
            f".repo-task/templates/bundled/{template_name}.md",
            read_bundled("templates", f"{template_name}.md"),
            f"bundled:templates/{template_name}.md",
            "templates",
        )
    writes.append(template_write)

    imported_roles: dict[str, str] = {}
    additional: list[str] = []
    for relative in answers["assets"]["agents"]:
        write = _import_write(discovery, "agents", relative)
        writes.append(write)
        role = normalized_agent_role(relative)
        if role:
            imported_roles[role] = write.destination
        else:
            additional.append(write.destination)

    roles = dict(imported_roles)
    if answers["bundled_defaults"]:
        for role in STANDARD_ROLES:
            if role in roles:
                continue
            destination = f".repo-task/agents/bundled/{role}.md"
            writes.append(
                PlannedWrite(
                    destination,
                    read_bundled("agents", f"{role}.md"),
                    f"bundled:agents/{role}.md",
                    "agents",
                )
            )
            roles[role] = destination

    conditional = _conditional_agents(stacks)
    terminology, create_enabled, adapter = VCS_DEFAULTS[vcs_provider]
    config = {
        "schema_version": 1,
        "project": {
            "name": answers["project"]["name"],
            "stacks": stacks,
            "base_branch": answers["project"]["base_branch"],
        },
        "branch": {"feature_pattern": answers["branch"]["feature_pattern"]},
        "vcs": {
            "provider": vcs_provider,
            "remote": "origin",
            "terminology": terminology,
            "create": {"enabled": create_enabled, "adapter": adapter},
        },
        "task_provider": task_provider,
        "workflow": {"documents": workflow_files, "manual_gates": MANUAL_GATES},
        "rules": {"files": rule_files},
        "change_request": {
            "template": template_write.destination,
            "title_pattern": "[{task_id}] {title}",
        },
        "agents": {
            "auto_assign_on_start": True,
            "refresh_on_context_update": True,
            "roles": roles,
            "default": list(roles),
            "conditional": conditional,
            "additional": additional,
        },
    }
    writes.append(
        PlannedWrite(".repo-task.yml", dump_yaml(config), "generated:configuration", "config")
    )
    gitignore_path = discovery.root / ".gitignore"
    existing_ignore = gitignore_path.read_text(encoding="utf-8") if gitignore_path.exists() else ""
    ignore_lines = existing_ignore.splitlines()
    if ".repo-task/tasks/" not in ignore_lines:
        ignore_lines.append(".repo-task/tasks/")
    gitignore = "\n".join(ignore_lines).lstrip("\n") + "\n"
    writes.append(
        PlannedWrite(
            ".gitignore", gitignore, "generated:gitignore-entry", "runtime", "managed-entry"
        )
    )
    return _deduplicate_writes(writes)


def _conditional_agents(stacks: list[str]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for stack in stacks:
        try:
            profile = yaml.safe_load(read_bundled("profiles", f"{stack}.yml")) or {}
        except FileNotFoundError:
            continue
        result.update(profile.get("conditional_agents", {}))
    return result


def _deduplicate_writes(writes: list[PlannedWrite]) -> list[PlannedWrite]:
    by_destination: dict[str, PlannedWrite] = {}
    for write in writes:
        existing = by_destination.get(write.destination)
        if existing and existing.content != write.content:
            raise RepoTaskError(f"Multiple assets target {write.destination}.")
        by_destination[write.destination] = write
    return list(by_destination.values())


def _preview(writes: list[PlannedWrite], root: Path) -> None:
    table = Table(title=f"Planned RepoTask writes in {root}")
    table.add_column("Action")
    table.add_column("Destination")
    table.add_column("Source")
    table.add_column("Category")
    for write in sorted(writes, key=lambda item: item.destination):
        path = root / write.destination
        action = (
            "unchanged"
            if path.exists() and path.read_text(encoding="utf-8") == write.content
            else ("replace" if path.exists() else "create")
        )
        table.add_row(action, write.destination, write.source, write.category)
    Console().print(table)


def _read_manifest(root: Path) -> dict[str, Any]:
    path = root / ".repo-task/setup-manifest.json"
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise RepoTaskError(f"Could not parse {path}: {error}") from error
    return data if isinstance(data, dict) else {}


def _owned_destinations(manifest: dict[str, Any]) -> set[str]:
    return {
        str(entry["destination"])
        for entry in manifest.get("files", [])
        if isinstance(entry, dict) and entry.get("ownership") in {"owned", "managed-entry"}
    }


def _backup_owned(root: Path, writes: list[PlannedWrite], manifest: dict[str, Any]) -> Path | None:
    owned = _owned_destinations(manifest)
    replacements = [
        write
        for write in writes
        if write.destination in owned
        and (root / write.destination).exists()
        and (root / write.destination).read_text(encoding="utf-8") != write.content
    ]
    if not replacements:
        return None
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    backup_root = root / ".repo-task/backups" / stamp
    for write in replacements:
        source = root / write.destination
        destination = backup_root / write.destination
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    return backup_root


def _apply(root: Path, writes: list[PlannedWrite], force: bool) -> tuple[int, Path | None]:
    manifest = _read_manifest(root)
    owned = _owned_destinations(manifest)
    conflicts = []
    changed = []
    for write in writes:
        path = root / write.destination
        if not path.exists() or path.read_text(encoding="utf-8") == write.content:
            if not path.exists():
                changed.append(write)
            continue
        if write.ownership == "managed-entry":
            changed.append(write)
        elif not force:
            conflicts.append(write.destination)
        elif write.destination not in owned:
            conflicts.append(write.destination)
        else:
            changed.append(write)
    if conflicts:
        suffix = " Use --force to replace manifest-owned files." if not force else ""
        raise RepoTaskError(
            "Initialization would replace files not authorized for replacement: "
            + ", ".join(conflicts)
            + suffix
        )

    backup = _backup_owned(root, writes, manifest) if force else None
    for write in writes:
        path = root / write.destination
        if path.exists() and path.read_text(encoding="utf-8") == write.content:
            continue
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(write.content, encoding="utf-8")

    entries = [
        {
            "source": write.source,
            "destination": write.destination,
            "sha256": write.digest,
            "category": write.category,
            "ownership": write.ownership,
            "selected": True,
        }
        for write in sorted(writes, key=lambda item: item.destination)
    ]
    manifest_path = root / ".repo-task/setup-manifest.json"
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(
        json.dumps({"schema_version": 1, "files": entries}, indent=2) + "\n",
        encoding="utf-8",
    )
    (root / ".repo-task/tasks").mkdir(parents=True, exist_ok=True)
    return len(changed), backup


def initialize(
    answers_path: Path | None,
    non_interactive: bool,
    dry_run: bool,
    force: bool,
    assume_yes: bool = False,
) -> None:
    discovery = discover_project()
    seed = _load_answers(answers_path)
    answers = _normalized_answers(discovery, seed, non_interactive)
    writes = _build_plan(discovery, answers)
    _preview(writes, discovery.root)
    if dry_run:
        Console().print("[yellow]Dry run: no files were written.[/yellow]")
        return
    if not non_interactive and not assume_yes:
        confirmed = prompt("Apply this plan? [y/N]: ", default="n").strip().lower()
        if confirmed not in {"y", "yes"}:
            Console().print("Initialization cancelled.")
            return
    changed, backup = _apply(discovery.root, writes, force)
    Console().print(f"[green]RepoTask initialized.[/green] {changed} file(s) changed.")
    if backup:
        Console().print(f"Backup created at {backup}")
    Console().print(
        "Configuration and imported assets are ready to commit; RepoTask did not create a commit."
    )
