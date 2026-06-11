from __future__ import annotations

from pathlib import Path

import pytest

from repotask.discovery.assets import validate_import
from repotask.errors import RepoTaskError


def test_rejects_credentials(git_repo: Path) -> None:
    (git_repo / ".env").write_text("TOKEN=secret\n", encoding="utf-8")
    with pytest.raises(RepoTaskError, match="credentials"):
        validate_import(git_repo, ".env")


def test_rejects_binary(git_repo: Path) -> None:
    (git_repo / "rules.md").write_bytes(b"hello\x00world")
    with pytest.raises(RepoTaskError, match="binary"):
        validate_import(git_repo, "rules.md")


def test_rejects_escaping_symlink(git_repo: Path, tmp_path: Path) -> None:
    outside = tmp_path / "outside.md"
    outside.write_text("secret\n", encoding="utf-8")
    (git_repo / "link.md").symlink_to(outside)
    with pytest.raises(RepoTaskError, match="escapes"):
        validate_import(git_repo, "link.md")


def test_accepts_utf8_markdown(git_repo: Path) -> None:
    (git_repo / "rules.md").write_text("# Rules\n", encoding="utf-8")
    source, content = validate_import(git_repo, "rules.md")
    assert source == git_repo / "rules.md"
    assert content == "# Rules\n"


def test_normalizes_imported_newlines(git_repo: Path) -> None:
    (git_repo / "rules.md").write_bytes(b"# Rules\r\n\rMore\r")
    _source, content = validate_import(git_repo, "rules.md")
    assert content == "# Rules\n\nMore\n"


def test_rejects_oversized_text(git_repo: Path) -> None:
    (git_repo / "large.md").write_text("x" * 1_000_001, encoding="utf-8")
    with pytest.raises(RepoTaskError, match="exceeds"):
        validate_import(git_repo, "large.md")
