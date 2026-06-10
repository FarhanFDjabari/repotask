"""Idempotent generated block insertion."""

from __future__ import annotations

import re

GENERATED_START = "<!-- repo-task:generated:start -->"
GENERATED_END = "<!-- repo-task:generated:end -->"


def render_template(template: str, generated: str, placeholders: dict[str, str]) -> str:
    rendered = template
    for key, value in placeholders.items():
        rendered = rendered.replace("{" + key + "}", value)
    block = f"{GENERATED_START}\n{generated.strip()}\n{GENERATED_END}"
    pattern = re.compile(
        re.escape(GENERATED_START) + r".*?" + re.escape(GENERATED_END), re.DOTALL
    )
    if pattern.search(rendered):
        rendered = pattern.sub(block, rendered, count=1)
    else:
        rendered = rendered.rstrip() + "\n\n" + block
    return rendered.rstrip() + "\n"

