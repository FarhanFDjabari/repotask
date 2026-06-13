"""Build a single-file Python RepoTask executable without bundling Python."""

from __future__ import annotations

import argparse
import shutil
import stat
import tempfile
import zipapp
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("dist/repo-task"))
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    package_source = root / "src" / "repotask"
    if not package_source.is_dir():
        raise SystemExit(f"RepoTask package not found: {package_source}")
    args.output.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="repotask-portable-") as temporary:
        app_root = Path(temporary)
        shutil.copytree(
            package_source,
            app_root / "repotask",
            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"),
        )
        (app_root / "__main__.py").write_text(
            "import sys\n\n"
            "if sys.version_info < (3, 9):\n"
            '    sys.exit("repo-task requires Python 3.9 or newer; found "\n'
            '             f"{sys.version_info.major}.{sys.version_info.minor}.")\n\n'
            "from repotask.cli import run\n\n"
            "raise SystemExit(run())\n",
            encoding="utf-8",
        )
        zipapp.create_archive(
            app_root,
            target=args.output,
            interpreter="/usr/bin/env python3",
            compressed=True,
        )
    current_mode = args.output.stat().st_mode
    args.output.chmod(current_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
