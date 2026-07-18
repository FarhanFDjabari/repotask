"""Token budgeting for context packs.

Estimation is deliberately crude — roughly four characters per token. The goal is
a predictable ceiling on what the agent receives, not an exact token count.
"""

from __future__ import annotations

CHARS_PER_TOKEN = 4
TRUNCATION_NOTE = (
    "\n\n[truncated by repo-task: read the full document with `repo-task {command} {id}`]"
)


def estimate(text: str) -> int:
    return max(1, len(text) // CHARS_PER_TOKEN)


def fit(text: str, tokens: int, command: str = "convention", doc_id: str = "") -> tuple[str, bool]:
    """Trim `text` to roughly `tokens`, cutting at a paragraph boundary when possible."""
    if tokens <= 0:
        return "", True
    limit = tokens * CHARS_PER_TOKEN
    if len(text) <= limit:
        return text, False
    # The note is part of what the agent receives, so it has to fit inside the budget too.
    note = TRUNCATION_NOTE.format(command=command, id=doc_id)
    limit -= len(note)
    if limit <= 0:
        return "", True
    cut = text[:limit]
    boundary = cut.rfind("\n\n")
    if boundary > limit // 2:
        cut = cut[:boundary]
    return cut.rstrip() + note, True
