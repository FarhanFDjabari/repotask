from __future__ import annotations

from pathlib import Path

from repotask import resources


class SingleArgumentTraversable:
    def __init__(self, path: Path) -> None:
        self.path = path

    def joinpath(self, child: str) -> SingleArgumentTraversable:
        return SingleArgumentTraversable(self.path / child)

    def read_text(self, encoding: str) -> str:
        return self.path.read_text(encoding=encoding)

    def __str__(self) -> str:
        return str(self.path)


def test_bundled_resources_support_single_argument_joinpath(
    tmp_path: Path,
    monkeypatch,
) -> None:
    bundled = tmp_path / "bundled"
    resource = bundled / "rules" / "common.md"
    resource.parent.mkdir(parents=True)
    resource.write_text("common rules\n", encoding="utf-8")
    monkeypatch.setattr(
        resources,
        "files",
        lambda package: SingleArgumentTraversable(bundled),
    )

    assert resources.bundled_path("rules", "common.md") == resource
    assert resources.read_bundled("rules", "common.md") == "common rules\n"
