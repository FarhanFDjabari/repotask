"""Access bundled files in source installs and the portable zipapp."""

from __future__ import annotations

from importlib.resources import files
from pathlib import Path

try:  # Python 3.11+ moved Traversable to importlib.resources.abc
    from importlib.resources.abc import Traversable
except ModuleNotFoundError:  # Python 3.9-3.10
    from importlib.abc import Traversable


def _bundled_resource(*parts: str) -> Traversable:
    # Anchor at the top-level package and treat "bundled" as a path part. On Windows with
    # Python 3.9/3.10 zipapps, resolving the "repotask.bundled" subpackage leaks OS path
    # separators ("repotask\\bundled") into the archive lookup; the single top-level anchor
    # keeps the path forward-slashed. joinpath is called one part at a time so Traversables
    # that only accept a single child argument keep working.
    resource = files("repotask").joinpath("bundled")
    for part in parts:
        resource = resource.joinpath(part)
    return resource


def bundled_path(*parts: str) -> Path:
    return Path(str(_bundled_resource(*parts)))


def read_bundled(*parts: str) -> str:
    try:
        return _bundled_resource(*parts).read_text(encoding="utf-8")
    except (FileNotFoundError, KeyError) as error:
        # zipfile (zipapp on Python 3.9/3.10) raises KeyError for a missing member;
        # normalize so callers can rely on FileNotFoundError across versions and backends.
        raise FileNotFoundError(f"bundled resource not found: {'/'.join(parts)}") from error
