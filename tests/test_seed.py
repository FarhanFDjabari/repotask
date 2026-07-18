from __future__ import annotations

from pathlib import Path

import pytest
import yaml

from repotask.errors import RepoTaskError
from repotask.kb.schema import load_manifest, load_slice
from repotask.kb.seed import scaffold


def test_scaffold_copies_only_the_relevant_stacks(tmp_path: Path) -> None:
    target = tmp_path / "knowledge"

    scaffold(target, ["android", "kotlin"])

    conventions = {path.name for path in (target / "conventions").glob("*.md")}
    assert "android.md" in conventions
    assert "ios.md" not in conventions
    assert "rust.md" not in conventions


def test_scaffold_follows_slice_inheritance(tmp_path: Path) -> None:
    target = tmp_path / "knowledge"

    scaffold(target, ["jetpack-compose"])

    slices = {path.stem for path in (target / "slices").glob("*.yaml")}
    assert slices == {"jetpack-compose", "android", "generic"}


def test_scaffold_falls_back_to_generic_for_unknown_stacks(tmp_path: Path) -> None:
    target = tmp_path / "knowledge"

    scaffold(target, ["cobol"])

    assert (target / "slices/generic.yaml").is_file()


def test_scaffolded_knowledge_base_is_valid(tmp_path: Path) -> None:
    target = tmp_path / "knowledge"

    scaffold(target, ["android"])

    manifest = load_manifest(target / "kb.yaml")
    assert manifest.schema_version == 1
    assert manifest.family("viewmodels") is not None
    for path in (target / "slices").glob("*.yaml"):
        assert load_slice(path).stack == path.stem


def test_scaffold_references_only_documents_it_copied(tmp_path: Path) -> None:
    target = tmp_path / "knowledge"

    scaffold(target, ["android"])

    copied = {
        path.stem.replace(".md", "") for path in (target / "recipes").glob("*.md")
    }
    for path in (target / "slices").glob("*.yaml"):
        document = yaml.safe_load(path.read_text(encoding="utf-8"))
        assert set(document["recipes"]) <= copied


def test_scaffold_refuses_to_clobber_without_force(tmp_path: Path) -> None:
    target = tmp_path / "knowledge"
    scaffold(target, ["android"])

    with pytest.raises(RepoTaskError, match="--force"):
        scaffold(target, ["android"])

    assert scaffold(target, ["android"], force=True)
