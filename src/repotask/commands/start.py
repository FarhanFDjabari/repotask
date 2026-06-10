"""Start a human-controlled task."""

from __future__ import annotations

from repotask.agents.service import refresh_agents
from repotask.config.loader import load_config
from repotask.errors import RepoTaskError
from repotask.files import write_json, write_text
from repotask.git import assert_clean_worktree, create_branch
from repotask.prompts.service import (
    cr_placeholder,
    investigate_prompt,
    load_prompt_inputs,
    review_placeholder,
)
from repotask.providers import get_provider_adapter
from repotask.tasks import now_iso, slugify, task_directory, validate_task_id


def start_task(task_id: str, title: str, mode: str) -> tuple[str, str]:
    config = load_config()
    if mode.strip().lower() != "human":
        raise RepoTaskError("Milestone 1 only supports --mode human.")
    task_id = validate_task_id(task_id)
    title = title.strip()
    if not title:
        raise RepoTaskError("--title must not be empty.")
    assert_clean_worktree(config.root)
    task_dir = task_directory(config, task_id)
    if task_dir.exists():
        raise RepoTaskError(f"Task workspace already exists: {task_dir}")
    branch = config.branch.feature_pattern.format(task_id=task_id, slug=slugify(title))
    created_at = now_iso()
    create_branch(config.root, branch, config.project.base_branch)
    provider = get_provider_adapter(config.task_provider)
    task_url = provider.task_url(task_id)
    references = [*config.workflow.documents, *config.rules.files]
    write_text(
        task_dir / "context.md",
        _context_markdown(
            task_id,
            title,
            branch,
            config.project.base_branch,
            created_at,
            config.task_provider.display_name,
            task_url,
            references,
        ),
    )
    write_text(task_dir / "checklist.md", _checklist())
    write_json(
        task_dir / "config.json",
        {
            "taskId": task_id,
            "provider": config.task_provider.provider,
            "taskUrl": task_url,
            "title": title,
            "mode": "human",
            "branch": branch,
            "baseBranch": config.project.base_branch,
            "createdAt": created_at,
        },
    )
    write_text(task_dir / "notes.md", _notes(task_id, title))
    context = (task_dir / "context.md").read_text(encoding="utf-8")
    if config.agents.auto_assign_on_start:
        refresh_agents(config, task_id, title, context, created_at=created_at)
    inputs = load_prompt_inputs(config, task_id)
    write_text(task_dir / "prompts/investigate.md", investigate_prompt(inputs))
    write_text(task_dir / "prompts/review.md", review_placeholder(inputs))
    write_text(task_dir / "prompts/cr.md", cr_placeholder(inputs))
    return branch, str(task_dir)


def _context_markdown(
    task_id: str,
    title: str,
    branch: str,
    base_branch: str,
    created_at: str,
    provider_name: str,
    task_url: str,
    references: list[str],
) -> str:
    reference_lines = "\n".join(f"- `{value}`" for value in references) or "- None"
    provider_line = f"- {provider_name} URL: {task_url}\n" if task_url else ""
    return f"""# Task Context

- Task ID: {task_id}
- Title: {title}
- Provider: {provider_name}
{provider_line}- Mode: human
- Branch: {branch}
- Base branch: {base_branch}
- Created at: {created_at}

## Description

TODO: Add task requirement, acceptance criteria, links, screenshots, and notes.

## Workflow References

{reference_lines}
"""


def _checklist() -> str:
    return """# Human Mode Checklist

- [ ] Read task requirement
- [ ] Confirm acceptance criteria
- [ ] Investigate affected files
- [ ] Implement manually
- [ ] Run the project locally
- [ ] Verify happy path
- [ ] Verify edge cases
- [ ] Run relevant tests
- [ ] Run `repo-task review`
- [ ] Prepare the change request
"""


def _notes(task_id: str, title: str) -> str:
    return f"""# Notes

Task: {task_id} - {title}

## Investigation

## Implementation Notes

## Verification

## Follow-ups
"""

