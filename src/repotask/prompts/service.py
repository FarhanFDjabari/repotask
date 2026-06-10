"""Provider-neutral prompt generation."""

from __future__ import annotations

from dataclasses import dataclass

from repotask.agents.service import orchestration
from repotask.config.models import RepoTaskConfig
from repotask.errors import RepoTaskError
from repotask.files import read_optional
from repotask.providers import get_provider_adapter
from repotask.rules import compose_rules
from repotask.tasks import read_task_config, task_directory
from repotask.workflow import load_workflow_documents


@dataclass(frozen=True)
class PromptInputs:
    config: RepoTaskConfig
    task_id: str
    context: str
    task_config: dict
    workflow_docs: list[tuple[str, str]]
    rules: list[tuple[str, str]]


def load_prompt_inputs(config: RepoTaskConfig, task_id: str) -> PromptInputs:
    task_dir = task_directory(config, task_id)
    context = read_optional(task_dir / "context.md")
    if context is None:
        raise RepoTaskError(
            f"Task context not found: {task_dir / 'context.md'}. Run repo-task start."
        )
    return PromptInputs(
        config=config,
        task_id=task_id,
        context=context,
        task_config=read_task_config(config, task_id),
        workflow_docs=load_workflow_documents(config),
        rules=compose_rules(config),
    )


def _documents(title: str, documents: list[tuple[str, str]], missing: str) -> str:
    if not documents:
        return f"# {title}\n\n{missing}"
    body = "\n\n".join(f"## {path}\n\n{content.strip()}" for path, content in documents)
    return f"# {title}\n\n{body}"


def header(inputs: PromptInputs) -> str:
    return "\n\n".join(
        [
            _documents(
                "Workflow Docs", inputs.workflow_docs, "No workflow document is configured."
            ),
            _documents("Engineering Rules", inputs.rules, "No engineering rules were found."),
            "# Task Context\n\n" + inputs.context.strip(),
        ]
    )


def context_prompt(inputs: PromptInputs) -> str:
    provider = inputs.config.task_provider.display_name
    return f"""{header(inputs)}

{orchestration(inputs.config, inputs.task_id, "context")}

# Instruction

Prepare concise task context from content supplied by the user or the configured {provider}.

Do not edit source code. Do not update provider status or automate approval gates.

Use:

## Description

### Problem Statement
### Expected Behavior
### Acceptance Criteria
### Important Context
### Open Questions
"""


def investigate_prompt(inputs: PromptInputs) -> str:
    return f"""{header(inputs)}

{orchestration(inputs.config, inputs.task_id, "investigate")}

# Instruction

Analyze this task without editing source code. Provide:

1. Requirement interpretation and assumptions
2. Likely affected files or packages
3. Possible root cause or implementation direction
4. Minimal implementation plan
5. Edge cases
6. Verification steps

# Capture

The parent updates `.repo-task/tasks/{inputs.task_id}/context.md` and `notes.md` once after
synthesizing worker findings. Human implementation mode remains primary.
"""


def review_placeholder(inputs: PromptInputs) -> str:
    return f"""{header(inputs)}

# Instruction

This placeholder is replaced after manual implementation by:

```bash
repo-task review {inputs.task_id}
```
"""


def review_prompt(
    inputs: PromptInputs,
    diff_text: str,
    base_branch: str,
    include_worktree: bool,
    callouts: list[str],
) -> str:
    command = (
        f"git diff {base_branch}...HEAD plus staged and unstaged tracked changes"
        if include_worktree
        else f"git diff {base_branch}...HEAD"
    )
    callout_text = "\n\n".join(callouts)
    if callout_text:
        callout_text += "\n\n"
    return f"""{header(inputs)}

{orchestration(inputs.config, inputs.task_id, "review")}

{callout_text}# Diff

Collected from `{command}`:

```diff
{diff_text.rstrip()}
```

# Instruction

Review this diff as a senior engineer. Do not rewrite the implementation unless a short snippet is
needed to explain a finding. Use these exact sections:

## Must fix
## Should fix
## Nice to have
## Regression risk
## Missing tests
## QA notes
## Suggested CR summary
"""


def cr_placeholder(inputs: PromptInputs) -> str:
    return f"""{header(inputs)}

# Instruction

After implementation and review, run:

```bash
repo-task cr {inputs.task_id}
```
"""


def cr_prompt(
    inputs: PromptInputs,
    template: str,
    review_output: str | None,
    diff_summary: str,
) -> str:
    review = review_output.strip() if review_output else "No saved review output was found."
    return f"""{header(inputs)}

{orchestration(inputs.config, inputs.task_id, "cr")}

# Change Request Template

{template.strip()}

# Review Output

{review}

# Diff Summary

{diff_summary}

# Instruction

Refine the prepared change-request description while preserving the template and its checklists.
Do not claim testing, QA, merge, deployment, or release completion without supplied evidence.
"""


def provider_status_prompt(
    inputs: PromptInputs, local_signals: list[tuple[str, str]]
) -> str:
    adapter = get_provider_adapter(inputs.config.task_provider)
    signals = "\n".join(f"- {label}: {value}" for label, value in local_signals)
    task_url = adapter.task_url(inputs.task_id) or "not configured"
    return f"""{header(inputs)}

# Local Workflow Signals

{signals}

# Provider Reference

- Provider: {inputs.config.task_provider.display_name}
- Task ID: {inputs.task_id}
- URL: {task_url}

# Instruction

{adapter.reconciliation_instruction(inputs.task_id)}
"""
