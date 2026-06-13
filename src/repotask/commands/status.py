"""Task status and provider reconciliation prompt."""

from __future__ import annotations

import re
from pathlib import Path

from repotask.config.loader import load_config
from repotask.files import read_json, read_optional, write_text
from repotask.git import current_branch, safe_git_output, untracked_files
from repotask.prompts.service import load_prompt_inputs, provider_status_prompt
from repotask.tasks import read_task_config, task_directory
from repotask.terminal import print_table


def task_status(task_id: str, provider: bool) -> Path | None:
    config = load_config()
    task_dir = task_directory(config, task_id)
    task_config = read_task_config(config, task_id)
    base = str(task_config.get("baseBranch") or config.project.base_branch)
    context = read_optional(task_dir / "context.md")
    branch = current_branch(config.root)
    agents = read_json(task_dir / "agents.json").get("assignedAgents", [])
    rows = [
        ("Task", str(task_config.get("taskId", task_id))),
        ("Title", str(task_config.get("title", "unknown"))),
        ("Provider", config.task_provider.display_name),
        ("Configured branch", str(task_config.get("branch", "unknown"))),
        ("Current branch", branch or "unknown"),
        ("Branch match", _yes_no(branch == task_config.get("branch"))),
        ("Base branch", base),
        ("Context", _file(task_dir / "context.md")),
        ("Context description", _description_state(context)),
        ("Investigation prompt", _file(task_dir / "prompts/investigate.md")),
        ("Review prompt", _file(task_dir / "prompts/review.md")),
        ("Review output", _file(task_dir / "review.md")),
        ("CR draft", _file(task_dir / "cr-description.md")),
        ("Assigned agents", f"{len(agents)} ({', '.join(agents)})" if agents else "none"),
        (
            "Committed diff",
            safe_git_output(config.root, ["diff", "--shortstat", f"{base}...HEAD"]).strip()
            or "none",
        ),
        (
            "Staged diff",
            safe_git_output(config.root, ["diff", "--cached", "--shortstat"]).strip() or "none",
        ),
        (
            "Unstaged diff",
            safe_git_output(config.root, ["diff", "--shortstat"]).strip() or "none",
        ),
        ("Untracked files", str(len(untracked_files(config.root)))),
    ]
    print_table(f"RepoTask status: {task_id}", ("Signal", "Value"), rows)
    if not provider:
        return None
    path = task_dir / "prompts/provider-status.md"
    write_text(path, provider_status_prompt(load_prompt_inputs(config, task_id), rows))
    return path


def _yes_no(value: bool) -> str:
    return "yes" if value else "no"


def _file(path: Path) -> str:
    return "exists" if path.is_file() else "missing"


def _description_state(context: str | None) -> str:
    if not context:
        return "missing"
    match = re.search(r"## Description\s*(.*?)(?=\n## |\Z)", context, flags=re.DOTALL)
    if not match:
        return "missing section"
    text = match.group(1).strip()
    if not text:
        return "empty"
    return "placeholder" if "TODO: Add task requirement" in text else "filled"
