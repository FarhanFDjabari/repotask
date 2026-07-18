"""Generate agent skill files that reference repo-task commands.

Skills stay thin on purpose: they tell the agent which command to run, never what the
project's conventions are. Inlined knowledge goes stale the moment the knowledge base
moves, and costs tokens on every session whether or not it is needed.
"""

from __future__ import annotations

from dataclasses import dataclass
from importlib import resources
from pathlib import Path

from repotask.config.models import RepoTaskConfig
from repotask.files import read_optional, write_text

SKILLS = ("repotask-search", "repotask-feature", "repotask-bugfix", "repotask-review")
CLAUDE_SKILLS_DIR = ".claude/skills"
AGENTS_FILE = "AGENTS.md"
BEGIN_MARKER = "<!-- repo-task:begin -->"
END_MARKER = "<!-- repo-task:end -->"


@dataclass(frozen=True)
class GeneratedFile:
    path: str
    action: str  # "created" | "updated" | "unchanged"


def template(name: str) -> str:
    return (
        resources.files("repotask.skills.templates")
        .joinpath(f"{name}.md")
        .read_text(encoding="utf-8")
    )


def agents_block(config: RepoTaskConfig) -> str:
    stacks = ", ".join(config.project.stacks)
    return f"""{BEGIN_MARKER}
## Project knowledge: use the `repo-task` CLI

This repository's architecture conventions, task playbooks, and code index live in a
knowledge base that the `repo-task` CLI reads. Query it instead of inferring
conventions from the source — it is reviewed and current.

Stacks: {stacks}

Start every task with one budgeted context pack:

```bash
repo-task --json brief "<what you are about to do>" --changed
```

Then, as needed:

| Need | Command |
| --- | --- |
| Find knowledge by keyword | `repo-task --json search "<keywords>"` |
| Architecture or stack rule | `repo-task --json convention <id>` |
| How this project does a task | `repo-task --json recipe <id>` |
| Curated project facts | `repo-task --json fact <family>` |
| Where a declaration lives | `repo-task --json symbol <name>` |
| Ticket to reviewable steps | `repo-task --json fetch\\|summarize\\|analyze\\|split <ticket>` |
| Bug triage and duplicates | `repo-task --json bug fetch <ticket>`, `repo-task --json bug dedupe` |

Every command returns `{{"ok": ..., "command": ..., "data": ..., "warnings": []}}`.
Read `data`; on failure `error.message` says what to do next.

Cite the document `id` behind any decision you justify with a project convention.
{END_MARKER}"""


def render_all(config: RepoTaskConfig) -> dict[str, str]:
    """Map of relative path -> content for every file `skills sync` owns."""
    files = {f"{CLAUDE_SKILLS_DIR}/{name}/SKILL.md": template(name) for name in SKILLS}
    files[AGENTS_FILE] = agents_block(config)
    return files


def sync(config: RepoTaskConfig, dry_run: bool = False) -> list[GeneratedFile]:
    results: list[GeneratedFile] = []
    for relative, content in render_all(config).items():
        path = config.root / relative
        if relative == AGENTS_FILE:
            content = _merge_agents(read_optional(path), content)
        existing = read_optional(path)
        if existing == content:
            results.append(GeneratedFile(relative, "unchanged"))
            continue
        if not dry_run:
            write_text(path, content)
        results.append(GeneratedFile(relative, "updated" if existing is not None else "created"))
    return results


def _merge_agents(existing: str | None, block: str) -> str:
    """Replace only our marked block so hand-written AGENTS.md content survives."""
    if existing is None:
        return block + "\n"
    if BEGIN_MARKER in existing and END_MARKER in existing:
        head, _, rest = existing.partition(BEGIN_MARKER)
        _, _, tail = rest.partition(END_MARKER)
        return f"{head}{block}{tail}"
    separator = "" if existing.endswith("\n\n") else ("\n" if existing.endswith("\n") else "\n\n")
    return f"{existing}{separator}{block}\n"


def find_agent_files(root: Path) -> list[str]:
    """Other harnesses' instruction files, reported so the user can point them here."""
    candidates = ("CLAUDE.md", ".cursorrules", ".github/copilot-instructions.md", "GEMINI.md")
    return [name for name in candidates if (root / name).is_file()]
