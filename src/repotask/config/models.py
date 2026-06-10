"""Validated RepoTask configuration models."""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from repotask.errors import RepoTaskError

SUPPORTED_SCHEMA_VERSION = 1
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
TASK_PROVIDERS = {
    "manual",
    "clickup",
    "jira",
    "linear",
    "github-issues",
    "gitlab-issues",
    "azure-devops",
    "custom",
}
VCS_PROVIDERS = {"gitlab", "github", "bitbucket", "other"}


def _mapping(data: Any, field_name: str) -> dict[str, Any]:
    if not isinstance(data, dict):
        raise RepoTaskError(f"Invalid configuration field '{field_name}': expected mapping.")
    return data


def _string(data: dict[str, Any], key: str, field_name: str, default: str = "") -> str:
    value = data.get(key, default)
    if not isinstance(value, str) or not value.strip():
        raise RepoTaskError(f"Invalid configuration field '{field_name}': expected non-empty text.")
    return value.strip()


def _strings(value: Any, field_name: str) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) for item in value):
        raise RepoTaskError(f"Invalid configuration field '{field_name}': expected a list of text.")
    return [item.strip() for item in value if item.strip()]


@dataclass(frozen=True)
class ProjectConfig:
    name: str
    stacks: list[str]
    base_branch: str


@dataclass(frozen=True)
class BranchConfig:
    feature_pattern: str = "feature/{task_id}-{slug}"


@dataclass(frozen=True)
class VcsConfig:
    provider: str
    remote: str = "origin"
    terminology: str = "change-request"
    create_enabled: bool = False
    adapter: str = "local"


@dataclass(frozen=True)
class TaskProviderConfig:
    provider: str
    display_name: str
    url_pattern: str = ""
    connector_hint: str = ""

    def task_url(self, task_id: str) -> str:
        return self.url_pattern.format(task_id=task_id) if self.url_pattern else ""


@dataclass(frozen=True)
class WorkflowConfig:
    documents: list[str] = field(default_factory=list)
    manual_gates: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class RulesConfig:
    files: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class ChangeRequestConfig:
    template: str = ""
    title_pattern: str = "[{task_id}] {title}"


@dataclass(frozen=True)
class ConditionalAgent:
    keywords: list[str] = field(default_factory=list)
    paths: list[str] = field(default_factory=list)
    phases: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class AgentsConfig:
    auto_assign_on_start: bool = True
    refresh_on_context_update: bool = True
    roles: dict[str, str] = field(default_factory=dict)
    default: list[str] = field(default_factory=list)
    conditional: dict[str, ConditionalAgent] = field(default_factory=dict)
    additional: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class RepoTaskConfig:
    root: Path
    schema_version: int
    project: ProjectConfig
    branch: BranchConfig
    vcs: VcsConfig
    task_provider: TaskProviderConfig
    workflow: WorkflowConfig
    rules: RulesConfig
    change_request: ChangeRequestConfig
    agents: AgentsConfig

    @classmethod
    def from_dict(cls, data: dict[str, Any], root: Path) -> RepoTaskConfig:
        schema = data.get("schema_version")
        if schema != SUPPORTED_SCHEMA_VERSION:
            raise RepoTaskError(
                f"Unsupported schema_version {schema!r}. RepoTask 0.1 supports schema version "
                f"{SUPPORTED_SCHEMA_VERSION}; migrate the configuration before continuing."
            )

        project_data = _mapping(data.get("project"), "project")
        stacks = _strings(project_data.get("stacks"), "project.stacks")
        unknown_stacks = sorted(set(stacks) - STACKS)
        if unknown_stacks:
            raise RepoTaskError(
                "Invalid configuration field 'project.stacks': unsupported values "
                + ", ".join(unknown_stacks)
            )

        branch_data = _mapping(data.get("branch", {}), "branch")
        pattern = _string(
            branch_data, "feature_pattern", "branch.feature_pattern", "feature/{task_id}-{slug}"
        )
        for placeholder in ("{task_id}", "{slug}"):
            if placeholder not in pattern:
                raise RepoTaskError(
                    f"Invalid configuration field 'branch.feature_pattern': missing {placeholder}."
                )

        vcs_data = _mapping(data.get("vcs"), "vcs")
        vcs_provider = _string(vcs_data, "provider", "vcs.provider")
        if vcs_provider not in VCS_PROVIDERS:
            raise RepoTaskError(f"Invalid configuration field 'vcs.provider': {vcs_provider}.")
        create_data = vcs_data.get("create", {})
        create_data = _mapping(create_data, "vcs.create")

        provider_data = _mapping(data.get("task_provider"), "task_provider")
        provider = _string(provider_data, "provider", "task_provider.provider")
        if provider not in TASK_PROVIDERS:
            raise RepoTaskError(
                f"Invalid configuration field 'task_provider.provider': {provider}."
            )

        workflow_data = _mapping(data.get("workflow", {}), "workflow")
        rules_data = _mapping(data.get("rules", {}), "rules")
        cr_data = _mapping(data.get("change_request", {}), "change_request")
        agents_data = _mapping(data.get("agents", {}), "agents")

        roles_raw = _mapping(agents_data.get("roles", {}), "agents.roles")
        roles = {str(key): str(value) for key, value in roles_raw.items()}
        conditional_raw = _mapping(agents_data.get("conditional", {}), "agents.conditional")
        conditional: dict[str, ConditionalAgent] = {}
        for name, raw in conditional_raw.items():
            raw = _mapping(raw, f"agents.conditional.{name}")
            conditional[str(name)] = ConditionalAgent(
                keywords=_strings(raw.get("keywords", []), f"agents.conditional.{name}.keywords"),
                paths=_strings(raw.get("paths", []), f"agents.conditional.{name}.paths"),
                phases=_strings(raw.get("phases", []), f"agents.conditional.{name}.phases"),
            )

        return cls(
            root=root,
            schema_version=schema,
            project=ProjectConfig(
                name=_string(project_data, "name", "project.name"),
                stacks=stacks,
                base_branch=_string(project_data, "base_branch", "project.base_branch", "main"),
            ),
            branch=BranchConfig(feature_pattern=pattern),
            vcs=VcsConfig(
                provider=vcs_provider,
                remote=str(vcs_data.get("remote", "origin")),
                terminology=str(vcs_data.get("terminology", "change-request")),
                create_enabled=bool(create_data.get("enabled", False)),
                adapter=str(create_data.get("adapter", "local")),
            ),
            task_provider=TaskProviderConfig(
                provider=provider,
                display_name=_string(
                    provider_data, "display_name", "task_provider.display_name", provider
                ),
                url_pattern=str(provider_data.get("url_pattern", "")),
                connector_hint=str(provider_data.get("connector_hint", "")),
            ),
            workflow=WorkflowConfig(
                documents=_strings(workflow_data.get("documents", []), "workflow.documents"),
                manual_gates=_strings(
                    workflow_data.get("manual_gates", []), "workflow.manual_gates"
                ),
            ),
            rules=RulesConfig(files=_strings(rules_data.get("files", []), "rules.files")),
            change_request=ChangeRequestConfig(
                template=str(cr_data.get("template", "")),
                title_pattern=str(cr_data.get("title_pattern", "[{task_id}] {title}")),
            ),
            agents=AgentsConfig(
                auto_assign_on_start=bool(agents_data.get("auto_assign_on_start", True)),
                refresh_on_context_update=bool(
                    agents_data.get("refresh_on_context_update", True)
                ),
                roles=roles,
                default=_strings(agents_data.get("default", list(roles)), "agents.default"),
                conditional=conditional,
                additional=_strings(agents_data.get("additional", []), "agents.additional"),
            ),
        )

    def path(self, relative: str) -> Path:
        return self.root / relative
