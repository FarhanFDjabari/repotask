"""Aggregate project discovery."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from repotask.discovery.assets import AssetCandidate, discover_assets
from repotask.discovery.stacks import detect_stacks
from repotask.discovery.vcs import detect_base_branch, detect_vcs_provider
from repotask.git import resolve_git_root


@dataclass(frozen=True)
class ProjectDiscovery:
    root: Path
    project_name: str
    stacks: list[str]
    vcs_provider: str
    base_branch: str
    assets: list[AssetCandidate]


def discover_project(start: Path | None = None) -> ProjectDiscovery:
    root = resolve_git_root(start)
    return ProjectDiscovery(
        root=root,
        project_name=root.name,
        stacks=detect_stacks(root),
        vcs_provider=detect_vcs_provider(root),
        base_branch=detect_base_branch(root),
        assets=discover_assets(root),
    )

