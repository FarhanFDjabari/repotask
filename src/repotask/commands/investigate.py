"""Investigation prompt command."""

from __future__ import annotations

from pathlib import Path

from repotask.agents.service import ensure_agents
from repotask.config.loader import load_config
from repotask.files import write_text
from repotask.prompts.service import investigate_prompt, load_prompt_inputs
from repotask.tasks import read_task_config, task_directory


def investigate(task_id: str) -> Path:
    config = load_config()
    inputs = load_prompt_inputs(config, task_id)
    task_config = read_task_config(config, task_id)
    ensure_agents(config, task_id, str(task_config.get("title", "")), inputs.context)
    path = task_directory(config, task_id) / "prompts/investigate.md"
    write_text(path, investigate_prompt(load_prompt_inputs(config, task_id)))
    return path

