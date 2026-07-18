"""RepoTask command line entry point.

Every command is agent-first: `--json` returns a stable envelope (see `repotask.output`),
and the CLI never calls a language model. It locates, parses, budgets, and returns.
"""

from __future__ import annotations

import typer

from repotask import __version__
from repotask.commands import brief as brief_commands
from repotask.commands import bug as bug_commands
from repotask.commands import facts as fact_commands
from repotask.commands import kb as kb_commands
from repotask.commands import query as query_commands
from repotask.commands import setup as setup_commands
from repotask.commands import skills as skills_commands
from repotask.commands import work as work_commands
from repotask.output import state

app = typer.Typer(
    name="repo-task",
    help="Agent-orchestrated knowledge and workflow CLI.",
    no_args_is_help=True,
    add_completion=False,
)
app.add_typer(kb_commands.app, name="kb", help="Manage the knowledge base source.")
app.add_typer(bug_commands.app, name="bug", help="Bugfix workflow.")
app.add_typer(skills_commands.app, name="skills", help="Generate agent skill files.")

for command in (
    setup_commands.init,
    setup_commands.doctor,
    setup_commands.migrate,
    query_commands.convention,
    query_commands.recipe,
    query_commands.search,
    fact_commands.index,
    fact_commands.symbol,
    fact_commands.fact,
    brief_commands.brief,
    work_commands.fetch,
    work_commands.summarize,
    work_commands.analyze,
    work_commands.split,
):
    app.command()(command)


def _version_callback(value: bool) -> None:
    if value:
        typer.echo(__version__)
        raise typer.Exit()


@app.callback()
def main(
    json_output: bool = typer.Option(
        False, "--json", help="Emit the machine-readable envelope instead of human output."
    ),
    _version: bool = typer.Option(
        False, "--version", callback=_version_callback, is_eager=True, help="Show the version."
    ),
) -> None:
    state.json_mode = json_output


def run() -> int:
    """Console entry point. Expected errors are handled by `output.guard` per command."""
    app()
    return 0
