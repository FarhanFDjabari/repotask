"""Agent assignment, snapshots, and task-scoped prompts."""

from __future__ import annotations

import fnmatch
import re
from pathlib import Path
from typing import Any

from repotask.config.models import RepoTaskConfig
from repotask.files import read_json, read_optional, write_json, write_text
from repotask.tasks import now_iso, task_directory


def _keyword_matches(haystack: str, keyword: str) -> bool:
    return bool(re.search(rf"(?<![a-z0-9]){re.escape(keyword)}(?![a-z0-9])", haystack))


def _path_matches(paths: list[str], patterns: list[str]) -> list[str]:
    return [
        pattern
        for pattern in patterns
        if any(
            fnmatch.fnmatch(path, pattern) or fnmatch.fnmatch(Path(path).name, pattern)
            for path in paths
        )
    ]


def assign_agents(
    config: RepoTaskConfig,
    title: str,
    context: str,
    changed_paths: list[str] | None = None,
) -> dict[str, Any]:
    haystack = f"{title}\n{context}".lower()
    paths = changed_paths or []
    conditional: list[str] = []
    keywords: dict[str, list[str]] = {}
    matched_paths: dict[str, list[str]] = {}
    for name, rule in config.agents.conditional.items():
        keyword_hits = [
            keyword for keyword in rule.keywords if _keyword_matches(haystack, keyword.lower())
        ]
        path_hits = _path_matches(paths, rule.paths)
        if keyword_hits or path_hits:
            conditional.append(name)
            if keyword_hits:
                keywords[name] = keyword_hits
            if path_hits:
                matched_paths[name] = path_hits
    assigned: list[str] = []
    for name in [*config.agents.default, *conditional]:
        if name not in assigned:
            assigned.append(name)
    return {
        "assignedAgents": assigned,
        "defaultAgents": config.agents.default,
        "conditionalAgents": conditional,
        "matchedKeywords": keywords,
        "matchedPaths": matched_paths,
    }


def refresh_agents(
    config: RepoTaskConfig,
    task_id: str,
    title: str,
    context: str,
    created_at: str | None = None,
    changed_paths: list[str] | None = None,
) -> dict[str, Any]:
    task_dir = task_directory(config, task_id)
    existing = read_json(task_dir / "agents.json")
    timestamp = now_iso()
    assignment = assign_agents(config, title, context, changed_paths)
    snapshot = {
        "taskId": task_id,
        **assignment,
        "createdAt": created_at or existing.get("createdAt") or timestamp,
        "refreshedAt": timestamp,
    }
    write_json(task_dir / "agents.json", snapshot)
    _write_agent_prompts(config, task_dir, task_id, assignment)
    return snapshot


def ensure_agents(
    config: RepoTaskConfig, task_id: str, title: str, context: str
) -> dict[str, Any]:
    task_dir = task_directory(config, task_id)
    existing = read_json(task_dir / "agents.json")
    if existing:
        return existing
    if config.agents.auto_assign_on_start:
        return refresh_agents(config, task_id, title, context)
    return {}


def _write_agent_prompts(
    config: RepoTaskConfig,
    task_dir: Path,
    task_id: str,
    assignment: dict[str, Any],
) -> None:
    agents_dir = task_dir / "prompts/agents"
    agents_dir.mkdir(parents=True, exist_ok=True)
    for existing in agents_dir.glob("*.md"):
        existing.unlink()
    for name in assignment["assignedAgents"]:
        role_path = config.agents.roles.get(name)
        role_text = read_optional(config.root / role_path) if role_path else None
        role_text = role_text or f"# {name}\n\nReturn focused findings for this specialty."
        reasons = []
        if assignment["matchedKeywords"].get(name):
            reasons.append("Keywords: " + ", ".join(assignment["matchedKeywords"][name]))
        if assignment["matchedPaths"].get(name):
            reasons.append("Paths: " + ", ".join(assignment["matchedPaths"][name]))
        reason_text = "\n".join(reasons) or "Assigned by default."
        capture = _capture_instruction(task_id, name)
        write_text(
            agents_dir / f"{name}.md",
            f"""# Assigned Prompt Agent

Agent: `{name}`

{reason_text}

{role_text.strip()}

{capture}

Do not edit source code unless explicitly asked. Worker agents return findings to the parent; only
the parent writes task documents during orchestrated runs. Do not automate provider status, QA,
merge, conflict resolution, deployment, or release gates.
""",
        )


def _capture_instruction(task_id: str, name: str) -> str:
    targets = {
        "requirement-analyst": ("context.md", "Description"),
        "code-investigator": ("notes.md", "Investigation"),
        "implementation-planner": ("notes.md", "Implementation Notes"),
        "qa-planner": ("notes.md", "Verification"),
    }
    document, section = targets.get(name, ("notes.md", "Follow-ups"))
    if name in {"diff-reviewer", "cr-writer"}:
        return ""
    return (
        f"# Capture\n\nWhen used directly, update `.repo-task/tasks/{task_id}/{document}` "
        f"under `## {section}`. Preserve every other section."
    )


def assigned_for_phase(snapshot: dict[str, Any], phase: str, config: RepoTaskConfig) -> list[str]:
    assigned = [str(value) for value in snapshot.get("assignedAgents", [])]
    conditional = set(str(value) for value in snapshot.get("conditionalAgents", []))
    phase_roles = {
        "context": {"requirement-analyst"},
        "investigate": {
            "requirement-analyst",
            "code-investigator",
            "implementation-planner",
            "qa-planner",
        },
        "review": {"diff-reviewer", "qa-planner"},
        "cr": {"cr-writer"},
    }
    allowed = phase_roles.get(phase, set(assigned))
    result = []
    for name in assigned:
        rule = config.agents.conditional.get(name)
        phase_match = name in conditional and (not rule or not rule.phases or phase in rule.phases)
        if name in allowed or phase_match:
            result.append(name)
    return result


def orchestration(config: RepoTaskConfig, task_id: str, phase: str) -> str:
    snapshot = read_json(task_directory(config, task_id) / "agents.json")
    selected = assigned_for_phase(snapshot, phase, config)
    if not selected:
        return "# Assigned Agent Orchestration\n\nNo prompt agents are assigned for this phase."
    lines = [
        f"- `{name}`: `.repo-task/tasks/{task_id}/prompts/agents/{name}.md`"
        for name in selected
    ]
    return f"""# Assigned Agent Orchestration

Use only these task agents for the `{phase}` phase:

{chr(10).join(lines)}

If subagents are supported, delegate independent roles in parallel. Otherwise apply them
sequentially. Workers return concise findings only. The parent resolves overlap and performs
document capture or final output exactly once."""
