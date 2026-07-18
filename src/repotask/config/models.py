"""RepoTask project configuration (schema version 2)."""

from __future__ import annotations

from pathlib import Path
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, field_validator

SUPPORTED_SCHEMA_VERSION = 2

STACKS = {
    "generic",
    "android",
    "kotlin",
    "jetpack-compose",
    "ios",
    "swift",
    "swiftui",
    "flutter",
    "dart",
    "react-native",
    "typescript",
    "web",
    "python",
    "go",
    "rust",
}

ConnectorMode = Literal["mcp", "rest", "off"]


class StrictModel(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)


class ProjectConfig(StrictModel):
    name: str
    stacks: list[str]
    base_branch: str = "main"

    @field_validator("stacks")
    @classmethod
    def _known_stacks(cls, value: list[str]) -> list[str]:
        if not value:
            raise ValueError("at least one stack is required")
        unknown = sorted(set(value) - STACKS)
        if unknown:
            raise ValueError(f"unsupported stacks: {', '.join(unknown)}")
        return value


class KnowledgeConfig(StrictModel):
    """Where the three-layer knowledge base comes from.

    `remote` wins when both are set; `local` is resolved against the project root.
    """

    remote: str = ""
    ref: str = "main"
    local: str = ".repo-task/knowledge"
    auto_sync: bool = True

    @field_validator("remote")
    @classmethod
    def _known_scheme(cls, value: str) -> str:
        if value and not value.startswith(("https://", "ssh://", "git@", "file://", "/")):
            raise ValueError(f"unsupported remote URL: {value}")
        return value


class IndexConfig(StrictModel):
    exclude: list[str] = Field(
        default_factory=lambda: ["**/build/**", "**/node_modules/**", "**/.git/**", "**/Pods/**"]
    )
    max_file_bytes: int = 512_000


class BriefConfig(StrictModel):
    default_budget: int = 6000


class ConnectorConfig(StrictModel):
    mode: ConnectorMode = "mcp"
    base_url: str = ""
    project: str = ""
    mcp_server: str = ""


class RepoTaskConfig(StrictModel):
    root: Path
    schema_version: int
    project: ProjectConfig
    knowledge: KnowledgeConfig = Field(default_factory=KnowledgeConfig)
    index: IndexConfig = Field(default_factory=IndexConfig)
    brief: BriefConfig = Field(default_factory=BriefConfig)
    connectors: dict[str, ConnectorConfig] = Field(default_factory=dict)

    @field_validator("schema_version")
    @classmethod
    def _supported(cls, value: int) -> int:
        if value != SUPPORTED_SCHEMA_VERSION:
            raise ValueError(
                f"unsupported schema_version {value}; this build supports "
                f"{SUPPORTED_SCHEMA_VERSION}. Run `repo-task migrate`."
            )
        return value

    @property
    def work_dir(self) -> Path:
        return self.root / ".repo-task/work"

    def path(self, relative: str) -> Path:
        return self.root / relative
