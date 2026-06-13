"""Run RepoTask with ``python -m repotask`` or from a zipapp."""

from __future__ import annotations

import sys

if sys.version_info < (3, 9):  # noqa: UP036 - runtime guard for older interpreters
    sys.exit(
        "repo-task requires Python 3.9 or newer; "
        f"found {sys.version_info.major}.{sys.version_info.minor}."
    )

from repotask.cli import run

raise SystemExit(run())
