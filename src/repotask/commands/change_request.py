"""Change-request description and remote creation."""

from __future__ import annotations

import sys

from repotask.agents.service import ensure_agents
from repotask.config.loader import load_config
from repotask.errors import RepoTaskError
from repotask.files import read_optional, write_text
from repotask.git import current_branch, diff_summary
from repotask.prompts.service import cr_prompt, load_prompt_inputs
from repotask.providers import get_provider_adapter
from repotask.tasks import task_directory
from repotask.templates import render_template
from repotask.vcs import get_vcs_adapter


def change_request(task_id: str, create: bool, draft: bool) -> list[str]:
    config = load_config()
    inputs = load_prompt_inputs(config, task_id)
    task_dir = task_directory(config, task_id)
    expected = str(inputs.task_config.get("branch", ""))
    branch = current_branch(config.root)
    if expected and branch and expected != branch:
        print(
            f"Warning: current branch '{branch}' does not match task branch '{expected}'.",
            file=sys.stderr,
        )
    base = str(inputs.task_config.get("baseBranch") or config.project.base_branch)
    description_path = task_dir / "cr-description.md"
    raw_title = str(inputs.task_config.get("title", ""))
    sanitized_title = raw_title.replace("\n", " ").strip()
    title = config.change_request.title_pattern.format(
        task_id=task_id, title=sanitized_title
    )
    if create:
        if draft and not create:
            raise RepoTaskError("--draft requires --create.")
        if not description_path.is_file():
            raise RepoTaskError(
                f"CR description not found: {description_path}. Run repo-task cr {task_id} first."
            )
        description = description_path.read_text(encoding="utf-8")
        if "TODO:" in description:
            print("Warning: CR description contains TODO placeholders.", file=sys.stderr)
        if not config.vcs.create_enabled:
            raise RepoTaskError(
                f"{config.vcs.provider} is configured for description generation only."
            )
        output = get_vcs_adapter(config.vcs).create(
            config.root, title, description_path, base, expected or branch, draft
        )
        return [output or f"Created change request for {task_id}."]

    ensure_agents(
        config,
        task_id,
        str(inputs.task_config.get("title", "")),
        inputs.context,
    )
    template = read_optional(config.root / config.change_request.template)
    if template is None:
        raise RepoTaskError(f"CR template not found: {config.change_request.template}")
    summary = diff_summary(config.root, base)
    review_output = read_optional(task_dir / "review.md")
    provider = get_provider_adapter(config.task_provider)
    generated = _generated_content(
        task_id,
        str(inputs.task_config.get("title", "")),
        provider.reference(task_id),
        summary,
        review_output,
    )
    description = render_template(
        template,
        generated,
        {
            "task_id": task_id,
            "task_url": provider.task_url(task_id),
            "title": str(inputs.task_config.get("title", "")),
        },
    )
    write_text(description_path, description)
    prompt_path = task_dir / "prompts/cr.md"
    write_text(prompt_path, cr_prompt(inputs, template, review_output, summary))
    return [str(description_path), str(prompt_path)]


def _generated_content(
    task_id: str,
    title: str,
    task_reference: str,
    summary: str,
    review_output: str | None,
) -> str:
    review_note = (
        "Saved review output was available while generating this draft."
        if review_output
        else f"Review output is missing. Run the review prompt and save findings to "
        f"`.repo-task/tasks/{task_id}/review.md`."
    )
    excerpt = f"\n\n### Review Notes\n\n{review_output.strip()}" if review_output else ""
    return f"""## RepoTask Summary

- Task: {task_reference}
- Suggested title: {title}

## RepoTask Changes

TODO: Convert the diff summary into concise implementation bullets.

```text
{summary.strip()}
```

## RepoTask Testing

TODO: List local verification, relevant tests, and untested areas.

## RepoTask Risk

TODO: Describe regression risk and rollback considerations.

## RepoTask QA

{review_note}{excerpt}

## RepoTask Dependencies

TODO: List dependent change requests, configuration, or post-deployment actions.
"""

