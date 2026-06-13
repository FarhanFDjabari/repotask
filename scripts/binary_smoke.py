"""Cross-platform smoke test for a frozen RepoTask executable."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

import yaml


def run(args: list[str], root: Path) -> None:
    subprocess.run(args, cwd=root, check=True)


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: binary_smoke.py PATH_TO_REPO_TASK")
    executable = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="repotask-binary-smoke-") as temporary:
        root = Path(temporary)
        run(["git", "init", "-b", "main"], root)
        run(["git", "config", "user.email", "smoke@example.com"], root)
        run(["git", "config", "user.name", "RepoTask Smoke"], root)
        (root / "README.md").write_text("# Smoke\n", encoding="utf-8")
        run(["git", "add", "README.md"], root)
        run(["git", "commit", "-m", "initial"], root)
        answers = {
            "project": {"name": "smoke", "stacks": ["generic"], "base_branch": "main"},
            "vcs": {"provider": "other"},
            "task_provider": {"provider": "manual", "display_name": "Task"},
            "workflow": {"mode": "bundled"},
            "assets": {"workflow": [], "template": "", "rules": [], "agents": []},
            "bundled_defaults": True,
        }
        answer_path = root / "answers.yml"
        answer_path.write_text(yaml.safe_dump(answers, sort_keys=False), encoding="utf-8")
        command = str(executable)
        run([command, "--version"], root)
        run(
            [command, "init", "--dry-run", "--non-interactive", "--answers", str(answer_path)],
            root,
        )
        run([command, "init", "--non-interactive", "--answers", str(answer_path)], root)
        run([command, "doctor"], root)
        run(["git", "add", "."], root)
        run(["git", "commit", "-m", "setup"], root)
        run([command, "start", "TEST-1", "--title", "Smoke test", "--mode", "human"], root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
