"""Task context updates and prompt generation."""

from __future__ import annotations

import re
import sys
from pathlib import Path

from repotask.agents.service import ensure_agents, refresh_agents
from repotask.config.loader import load_config
from repotask.errors import RepoTaskError
from repotask.files import read_text, write_text
from repotask.prompts.service import context_prompt, investigate_prompt, load_prompt_inputs
from repotask.tasks import read_task_config, task_directory


def update_context(
    task_id: str,
    paste: bool,
    from_file: Path | None,
    prompt_only: bool,
) -> Path:
    config = load_config()
    inputs = load_prompt_inputs(config, task_id)
    task_dir = task_directory(config, task_id)
    task_config = read_task_config(config, task_id)
    title = str(task_config.get("title", ""))
    if prompt_only:
        ensure_agents(config, task_id, title, inputs.context)
        inputs = load_prompt_inputs(config, task_id)
        path = task_dir / "prompts/context.md"
        write_text(path, context_prompt(inputs))
        return path
    if from_file:
        if not from_file.is_file():
            raise RepoTaskError(f"Requirement file not found: {from_file}")
        requirement = read_text(from_file)
        source = str(from_file)
    elif paste:
        requirement = sys.stdin.read()
        source = "stdin"
    else:
        raise RepoTaskError("Choose exactly one of --paste, --from-file, or --prompt.")
    if not requirement.strip():
        raise RepoTaskError("No requirement text provided.")
    path = task_dir / "context.md"
    updated = _replace_description(read_text(path), _description(requirement, source))
    write_text(path, updated)
    if config.agents.refresh_on_context_update:
        refresh_agents(config, task_id, title, updated)
        write_text(
            task_dir / "prompts/investigate.md",
            investigate_prompt(load_prompt_inputs(config, task_id)),
        )
    return path


def _description(requirement: str, source: str) -> str:
    return f"""Source: {source}

### Raw Requirement

{requirement.strip()}

### Problem Statement

TODO: Summarize the problem.

### Expected Behavior

TODO: Summarize expected behavior.

### Acceptance Criteria

TODO: Add acceptance criteria.

### Important Context

TODO: Add links, screenshots, comments, or constraints.

### Open Questions

TODO: Add unresolved questions or write `None`.
"""


def _replace_description(context: str, description: str) -> str:
    replacement = f"## Description\n\n{description.strip()}\n"
    pattern = r"## Description\s*.*?(?=\n## |\Z)"
    if re.search(pattern, context, flags=re.DOTALL):
        updated = re.sub(pattern, replacement, context, count=1, flags=re.DOTALL)
        return updated if updated.endswith("\n") else updated + "\n"
    return context.rstrip() + "\n\n" + replacement

