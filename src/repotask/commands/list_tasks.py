"""List task workspaces."""

from __future__ import annotations

from repotask.config.loader import load_config
from repotask.files import read_json
from repotask.git import current_branch
from repotask.terminal import print_table


def list_tasks() -> int:
    config = load_config()
    root = config.root / ".repo-task/tasks"
    current = current_branch(config.root)
    rows = []
    task_dirs = sorted(path for path in root.iterdir() if path.is_dir()) if root.exists() else []
    for task_dir in task_dirs:
        task = read_json(task_dir / "config.json")
        if not task:
            continue
        branch = str(task.get("branch", "unknown"))
        rows.append(
            (
                "*" if current == branch else "",
                str(task.get("taskId", task_dir.name)),
                str(task.get("title", "unknown")),
                branch,
            )
        )
    if not rows:
        print("No tasks found. Run repo-task start first.")
        return 0
    print_table("RepoTask tasks", ("", "Task", "Title", "Branch"), rows)
    return len(rows)
