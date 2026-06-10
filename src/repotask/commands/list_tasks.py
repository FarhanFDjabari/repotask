"""List task workspaces."""

from __future__ import annotations

from rich.console import Console
from rich.table import Table

from repotask.config.loader import load_config
from repotask.files import read_json
from repotask.git import current_branch


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
        Console().print("No tasks found. Run repo-task start first.")
        return 0
    table = Table(title="RepoTask tasks")
    table.add_column("")
    table.add_column("Task")
    table.add_column("Title")
    table.add_column("Branch")
    for row in rows:
        table.add_row(*row)
    Console().print(table)
    return len(rows)
