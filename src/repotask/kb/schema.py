"""Knowledge base manifest, document frontmatter, and slice definitions."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Literal

import yaml
from pydantic import BaseModel, ConfigDict, Field, ValidationError

from repotask.errors import RepoTaskError

KB_SCHEMA_VERSION = 1
MANIFEST_NAME = "kb.yaml"

Layer = Literal["convention", "recipe"]

FRONTMATTER_FENCE = "---"


class KbModel(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)


class FactFamily(KbModel):
    """A curated project-fact family the indexer knows how to populate."""

    name: str
    description: str = ""
    stacks: list[str] = Field(default_factory=list)
    languages: list[str] = Field(default_factory=list)
    kinds: list[str] = Field(default_factory=list)
    name_pattern: str = ""
    path_pattern: str = ""


class Manifest(KbModel):
    schema_version: int
    name: str = "knowledge-base"
    description: str = ""
    default_budget: int = 6000
    fact_families: list[FactFamily] = Field(default_factory=list)

    def family(self, name: str) -> FactFamily | None:
        return next((item for item in self.fact_families if item.name == name), None)


class Slice(KbModel):
    """Which knowledge applies to a stack, optionally narrowed per intent."""

    stack: str
    extends: list[str] = Field(default_factory=list)
    conventions: list[str] = Field(default_factory=list)
    recipes: list[str] = Field(default_factory=list)
    facts: list[str] = Field(default_factory=list)
    intents: dict[str, list[str]] = Field(default_factory=dict)


class Document(KbModel):
    """A convention or recipe markdown file with parsed frontmatter."""

    id: str
    title: str
    layer: Layer
    stacks: list[str] = Field(default_factory=list)
    tags: list[str] = Field(default_factory=list)
    applies_to: list[str] = Field(default_factory=list)
    budget_hint: int = 0
    body: str = ""
    path: str = ""


def parse_frontmatter(text: str) -> tuple[dict[str, Any], str]:
    """Split `---` delimited YAML frontmatter from the markdown body."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != FRONTMATTER_FENCE:
        return {}, text
    for index in range(1, len(lines)):
        if lines[index].strip() == FRONTMATTER_FENCE:
            raw = yaml.safe_load("\n".join(lines[1:index])) or {}
            if not isinstance(raw, dict):
                raise RepoTaskError("Document frontmatter must be a YAML mapping.")
            return raw, "\n".join(lines[index + 1 :]).strip()
    raise RepoTaskError("Document frontmatter is missing its closing `---`.")


def load_document(path: Path, layer: Layer, kb_root: Path) -> Document:
    front, body = parse_frontmatter(path.read_text(encoding="utf-8"))
    front.setdefault("id", path.stem)
    front.setdefault("title", path.stem.replace("-", " ").title())
    # The directory decides the layer. Frontmatter may restate it, but not contradict it.
    declared = front.pop("layer", layer)
    if declared != layer:
        raise RepoTaskError(
            f"{path} declares layer '{declared}' but sits in the {layer} directory."
        )
    try:
        return Document(
            **front, layer=layer, body=body, path=str(path.relative_to(kb_root))
        )
    except ValidationError as error:
        raise RepoTaskError(f"Invalid frontmatter in {path}: {_first_message(error)}") from error


def load_manifest(path: Path) -> Manifest:
    raw = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    if not isinstance(raw, dict):
        raise RepoTaskError(f"{path} must contain a YAML mapping.")
    try:
        manifest = Manifest(**raw)
    except ValidationError as error:
        raise RepoTaskError(f"Invalid {MANIFEST_NAME}: {_first_message(error)}") from error
    if manifest.schema_version != KB_SCHEMA_VERSION:
        raise RepoTaskError(
            f"Knowledge base schema_version {manifest.schema_version} is not supported; "
            f"this build reads version {KB_SCHEMA_VERSION}."
        )
    return manifest


def load_slice(path: Path) -> Slice:
    raw = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    if not isinstance(raw, dict):
        raise RepoTaskError(f"{path} must contain a YAML mapping.")
    raw.setdefault("stack", path.stem)
    try:
        return Slice(**raw)
    except ValidationError as error:
        raise RepoTaskError(f"Invalid slice {path}: {_first_message(error)}") from error


def _first_message(error: ValidationError) -> str:
    item = error.errors()[0]
    location = ".".join(str(part) for part in item["loc"]) or "<root>"
    return f"{location}: {item['msg']}"
