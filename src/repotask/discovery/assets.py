"""Discover and validate importable project assets."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from repotask.errors import RepoTaskError

ALLOWED_SUFFIXES = {".md", ".markdown", ".txt", ".yml", ".yaml", ".json", ".toml"}
MAX_IMPORT_BYTES = 1_000_000
SENSITIVE_NAMES = {
    ".env",
    ".env.local",
    ".env.production",
    "credentials.json",
    "secrets.yml",
    "secrets.yaml",
    "id_rsa",
    "id_ed25519",
}
KNOWN_AGENT_ROLES = {
    "requirement-analyst",
    "code-investigator",
    "implementation-planner",
    "diff-reviewer",
    "qa-planner",
    "cr-writer",
}


@dataclass(frozen=True)
class AssetCandidate:
    relative_path: str
    category: str


def _classify(path: Path) -> str | None:
    value = path.as_posix().lower()
    name = path.name.lower()
    if "merge_request_templates" in value or "pull_request_template" in value:
        return "templates"
    if name in {"agents.md", "claude.md", "copilot-instructions.md"}:
        return "rules"
    if any(part in value for part in ("/agents/", ".agents/", ".claude/agents", ".codex/agents")):
        return "agents"
    if "workflow" in value:
        return "workflow"
    if any(part in value for part in ("/rules/", ".cursor/rules", ".ai/")):
        return "rules"
    return None


def discover_assets(root: Path) -> list[AssetCandidate]:
    candidates: list[AssetCandidate] = []
    seen: set[str] = set()
    for path in root.rglob("*"):
        if not path.is_file() and not path.is_symlink():
            continue
        relative = path.relative_to(root)
        excluded = {".git", ".repo-task", ".task", "node_modules", "build"}
        if any(part in excluded for part in relative.parts):
            continue
        category = _classify(relative)
        if category and relative.as_posix() not in seen:
            candidates.append(AssetCandidate(relative.as_posix(), category))
            seen.add(relative.as_posix())
    return sorted(candidates, key=lambda item: (item.category, item.relative_path))


def validate_import(root: Path, relative_path: str) -> tuple[Path, str]:
    source = root / relative_path
    if not source.exists():
        raise RepoTaskError(f"Selected import does not exist: {relative_path}")
    resolved_root = root.resolve()
    resolved = source.resolve()
    try:
        resolved.relative_to(resolved_root)
    except ValueError as error:
        raise RepoTaskError(f"Unsafe symlink escapes the repository: {relative_path}") from error
    if not resolved.is_file():
        raise RepoTaskError(f"Selected import is not a file: {relative_path}")
    if source.name.lower() in SENSITIVE_NAMES or source.name.lower().startswith(".env."):
        raise RepoTaskError(f"Refusing to import potential credentials: {relative_path}")
    if source.suffix.lower() not in ALLOWED_SUFFIXES:
        raise RepoTaskError(f"Unsupported import format: {relative_path}")
    if resolved.stat().st_size > MAX_IMPORT_BYTES:
        raise RepoTaskError(
            f"Import exceeds {MAX_IMPORT_BYTES} bytes: {relative_path}"
        )
    content = resolved.read_bytes()
    if b"\x00" in content:
        raise RepoTaskError(f"Refusing to import binary file: {relative_path}")
    try:
        return resolved, content.decode("utf-8")
    except UnicodeDecodeError as error:
        raise RepoTaskError(f"Import is not UTF-8 text: {relative_path}") from error


def normalized_agent_role(path: str) -> str | None:
    stem = Path(path).stem.lower().replace("_", "-").replace(" ", "-")
    aliases = {"mr-writer": "cr-writer", "pr-writer": "cr-writer"}
    stem = aliases.get(stem, stem)
    return stem if stem in KNOWN_AGENT_ROLES else None
