"""RepoTask Typer command line interface."""

from __future__ import annotations

from pathlib import Path
from typing import Annotated

import typer
from rich.console import Console

from repotask import __version__
from repotask.commands.change_request import change_request
from repotask.commands.context import update_context
from repotask.commands.doctor import run_doctor
from repotask.commands.init import initialize
from repotask.commands.investigate import investigate
from repotask.commands.list_tasks import list_tasks
from repotask.commands.review import review
from repotask.commands.start import start_task
from repotask.commands.status import task_status
from repotask.errors import RepoTaskError

app = typer.Typer(
    name="repo-task",
    help="Provider-neutral, human-controlled developer workflow CLI.",
    no_args_is_help=True,
    pretty_exceptions_enable=False,
)
console = Console()


def _version(value: bool) -> None:
    if value:
        typer.echo(f"repo-task {__version__}")
        raise typer.Exit()


@app.callback()
def main(
    version: Annotated[
        bool | None,
        typer.Option("--version", callback=_version, is_eager=True, help="Show version and exit."),
    ] = None,
) -> None:
    """RepoTask commands."""


@app.command("init")
def init_command(
    dry_run: Annotated[bool, typer.Option("--dry-run", help="Preview without writing.")] = False,
    answers: Annotated[
        Path | None,
        typer.Option("--answers", exists=True, dir_okay=False, readable=True),
    ] = None,
    non_interactive: Annotated[
        bool,
        typer.Option("--non-interactive", help="Require provider selection from answers."),
    ] = False,
    force: Annotated[
        bool,
        typer.Option("--force", help="Replace only manifest-owned files and create backups."),
    ] = False,
    yes: Annotated[bool, typer.Option("--yes", hidden=True)] = False,
) -> None:
    """Discover and initialize RepoTask at the Git root."""
    initialize(answers, non_interactive, dry_run, force, assume_yes=yes)


@app.command()
def doctor() -> None:
    """Check configuration, assets, Git, and CR tooling."""
    if not run_doctor():
        raise typer.Exit(1)


@app.command()
def start(
    task_id: Annotated[str, typer.Argument(metavar="TASK_ID")],
    title: Annotated[str, typer.Option("--title", help="Task title.")],
    mode: Annotated[str, typer.Option("--mode", help="Implementation mode.")] = "human",
) -> None:
    """Create a clean feature branch and local task workspace."""
    branch, workspace = start_task(task_id, title, mode)
    console.print(f"Created branch {branch}")
    console.print(f"Created task workspace {workspace}")


@app.command("context")
def context_command(
    task_id: Annotated[str, typer.Argument(metavar="TASK_ID")],
    paste: Annotated[
        bool, typer.Option("--paste", help="Read requirement text from stdin.")
    ] = False,
    from_file: Annotated[
        Path | None, typer.Option("--from-file", dir_okay=False, readable=True)
    ] = None,
    prompt_only: Annotated[
        bool, typer.Option("--prompt", help="Generate a context preparation prompt.")
    ] = False,
) -> None:
    """Update task context or generate its prompt."""
    selected = sum((paste, from_file is not None, prompt_only))
    if selected != 1:
        raise RepoTaskError("Choose exactly one of --paste, --from-file, or --prompt.")
    path = update_context(task_id, paste, from_file, prompt_only)
    console.print(f"{'Wrote' if prompt_only else 'Updated'} {path}")


@app.command("investigate")
def investigate_command(
    task_id: Annotated[str, typer.Argument(metavar="TASK_ID")],
) -> None:
    """Generate an investigation prompt."""
    console.print(f"Wrote {investigate(task_id)}")


@app.command("review")
def review_command(
    task_id: Annotated[str, typer.Argument(metavar="TASK_ID")],
    worktree: Annotated[
        bool, typer.Option("--worktree", help="Include staged and unstaged tracked changes.")
    ] = False,
) -> None:
    """Generate a diff-aware senior review prompt."""
    console.print(f"Wrote {review(task_id, worktree)}")


@app.command("cr")
def cr_command(
    task_id: Annotated[str, typer.Argument(metavar="TASK_ID")],
    create: Annotated[
        bool, typer.Option("--create", help="Create via the configured local VCS CLI.")
    ] = False,
    draft: Annotated[
        bool, typer.Option("--draft", help="With --create, open as draft.")
    ] = False,
) -> None:
    """Generate or create a provider-neutral change request."""
    if draft and not create:
        raise RepoTaskError("--draft requires --create.")
    for output in change_request(task_id, create, draft):
        console.print(f"Wrote {output}" if not create else output)


@app.command("status")
def status_command(
    task_id: Annotated[str, typer.Argument(metavar="TASK_ID")],
    provider: Annotated[
        bool, typer.Option("--provider", help="Write a read-only provider reconciliation prompt.")
    ] = False,
) -> None:
    """Show local task workflow status."""
    path = task_status(task_id, provider)
    if path:
        console.print(f"Wrote {path}")


@app.command("list")
def list_command() -> None:
    """List task workspaces."""
    list_tasks()


def run() -> None:
    try:
        app()
    except RepoTaskError as error:
        console.print(f"[red]Error:[/red] {error}", err=True)
        raise typer.Exit(1) from error


if __name__ == "__main__":
    run()
