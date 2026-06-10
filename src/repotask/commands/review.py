"""Diff-aware review command."""

from __future__ import annotations

import sys
from pathlib import Path

from repotask.agents.service import refresh_agents
from repotask.config.loader import load_config
from repotask.errors import RepoTaskError
from repotask.files import write_text
from repotask.git import (
    changed_paths,
    current_branch,
    diff,
    has_tracked_worktree_changes,
    has_uncommitted_changes,
    untracked_files,
)
from repotask.prompts.service import load_prompt_inputs, review_prompt
from repotask.rules import detector_callouts
from repotask.tasks import task_directory


def review(task_id: str, include_worktree: bool) -> Path:
    config = load_config()
    inputs = load_prompt_inputs(config, task_id)
    expected = str(inputs.task_config.get("branch", ""))
    current = current_branch(config.root)
    if expected and current and expected != current:
        print(
            f"Warning: current branch '{current}' does not match task branch '{expected}'. "
            "The diff may not reflect this task.",
            file=sys.stderr,
        )
    base = str(inputs.task_config.get("baseBranch") or config.project.base_branch)
    diff_text = diff(config.root, base, include_worktree)
    if not diff_text.strip():
        untracked = untracked_files(config.root)
        if has_tracked_worktree_changes(config.root) and not include_worktree:
            raise RepoTaskError(
                f"No committed diff found for {base}...HEAD, but tracked worktree changes exist. "
                f"Run repo-task review {task_id} --worktree."
            )
        message = f"No diff found for review against {base}."
        if untracked:
            message += " Untracked files are excluded until staged."
        raise RepoTaskError(message)
    if has_uncommitted_changes(config.root) and not include_worktree:
        print(
            "Warning: uncommitted changes are not included. Use --worktree to review them.",
            file=sys.stderr,
        )
    paths = changed_paths(config.root, base, include_worktree)
    if config.agents.refresh_on_context_update:
        refresh_agents(
            config,
            task_id,
            str(inputs.task_config.get("title", "")),
            inputs.context,
            changed_paths=paths,
        )
        inputs = load_prompt_inputs(config, task_id)
    path = task_directory(config, task_id) / "prompts/review.md"
    write_text(
        path,
        review_prompt(
            inputs,
            diff_text,
            base,
            include_worktree,
            detector_callouts(config, paths),
        ),
    )
    return path

