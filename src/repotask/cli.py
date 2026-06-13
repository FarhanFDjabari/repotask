"""RepoTask argparse command line interface."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

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
from repotask.terminal import print_error


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="repo-task",
        description="Provider-neutral, human-controlled developer workflow CLI.",
    )
    parser.add_argument("--version", action="store_true", help="Show version and exit.")
    parser.add_argument(
        "--show-completion",
        nargs="?",
        const="bash",
        choices=("bash", "zsh", "fish"),
        help="Show a static shell completion script.",
    )
    subparsers = parser.add_subparsers(dest="command")

    init_parser = subparsers.add_parser("init", help="Discover and initialize RepoTask.")
    init_parser.add_argument("--dry-run", action="store_true", help="Preview without writing.")
    init_parser.add_argument("--answers", type=Path, help="Read setup answers from a YAML file.")
    init_parser.add_argument(
        "--non-interactive",
        action="store_true",
        help="Require provider selection from answers.",
    )
    init_parser.add_argument(
        "--force",
        action="store_true",
        help="Replace only manifest-owned files and create backups.",
    )
    init_parser.add_argument("--yes", action="store_true", help=argparse.SUPPRESS)
    init_parser.set_defaults(func=_cmd_init)

    doctor_parser = subparsers.add_parser("doctor", help="Check configuration and tooling.")
    doctor_parser.set_defaults(func=_cmd_doctor)

    start_parser = subparsers.add_parser(
        "start", help="Create a clean feature branch and local task workspace."
    )
    start_parser.add_argument("task_id", metavar="TASK_ID")
    start_parser.add_argument("--title", required=True, help="Task title.")
    start_parser.add_argument("--mode", default="human", help="Implementation mode.")
    start_parser.set_defaults(func=_cmd_start)

    context_parser = subparsers.add_parser(
        "context", help="Update task context or generate its prompt."
    )
    context_parser.add_argument("task_id", metavar="TASK_ID")
    context_group = context_parser.add_mutually_exclusive_group(required=True)
    context_group.add_argument(
        "--paste", action="store_true", help="Read requirement text from stdin."
    )
    context_group.add_argument("--from-file", type=Path, help="Read requirement text from a file.")
    context_group.add_argument("--prompt", action="store_true", help="Generate a context prompt.")
    context_parser.set_defaults(func=_cmd_context)

    investigate_parser = subparsers.add_parser(
        "investigate", help="Generate an investigation prompt."
    )
    investigate_parser.add_argument("task_id", metavar="TASK_ID")
    investigate_parser.set_defaults(func=_cmd_investigate)

    review_parser = subparsers.add_parser(
        "review", help="Generate a diff-aware senior review prompt."
    )
    review_parser.add_argument("task_id", metavar="TASK_ID")
    review_parser.add_argument(
        "--worktree",
        action="store_true",
        help="Include staged and unstaged tracked changes.",
    )
    review_parser.set_defaults(func=_cmd_review)

    cr_parser = subparsers.add_parser(
        "cr", help="Generate or create a provider-neutral change request."
    )
    cr_parser.add_argument("task_id", metavar="TASK_ID")
    cr_parser.add_argument(
        "--create",
        action="store_true",
        help="Create via the configured local VCS CLI.",
    )
    cr_parser.add_argument("--draft", action="store_true", help="With --create, open as draft.")
    cr_parser.set_defaults(func=_cmd_cr)

    status_parser = subparsers.add_parser("status", help="Show local task workflow status.")
    status_parser.add_argument("task_id", metavar="TASK_ID")
    status_parser.add_argument(
        "--provider",
        action="store_true",
        help="Write a read-only provider reconciliation prompt.",
    )
    status_parser.set_defaults(func=_cmd_status)

    list_parser = subparsers.add_parser("list", help="List task workspaces.")
    list_parser.set_defaults(func=_cmd_list)
    return parser


def _cmd_init(args: argparse.Namespace) -> int:
    if args.answers and not args.answers.is_file():
        raise RepoTaskError(f"Answers file not found: {args.answers}")
    initialize(args.answers, args.non_interactive, args.dry_run, args.force, assume_yes=args.yes)
    return 0


def _cmd_doctor(_args: argparse.Namespace) -> int:
    return 0 if run_doctor() else 1


def _cmd_start(args: argparse.Namespace) -> int:
    branch, workspace = start_task(args.task_id, args.title, args.mode)
    print(f"Created branch {branch}")
    print(f"Created task workspace {workspace}")
    return 0


def _cmd_context(args: argparse.Namespace) -> int:
    if args.from_file and not args.from_file.is_file():
        raise RepoTaskError(f"Context file not found: {args.from_file}")
    path = update_context(args.task_id, args.paste, args.from_file, args.prompt)
    print(f"{'Wrote' if args.prompt else 'Updated'} {path}")
    return 0


def _cmd_investigate(args: argparse.Namespace) -> int:
    print(f"Wrote {investigate(args.task_id)}")
    return 0


def _cmd_review(args: argparse.Namespace) -> int:
    print(f"Wrote {review(args.task_id, args.worktree)}")
    return 0


def _cmd_cr(args: argparse.Namespace) -> int:
    if args.draft and not args.create:
        raise RepoTaskError("--draft requires --create.")
    for output in change_request(args.task_id, args.create, args.draft):
        print(f"Wrote {output}" if not args.create else output)
    return 0


def _cmd_status(args: argparse.Namespace) -> int:
    path = task_status(args.task_id, args.provider)
    if path:
        print(f"Wrote {path}")
    return 0


def _cmd_list(_args: argparse.Namespace) -> int:
    list_tasks()
    return 0


def run(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.version:
        print(f"repo-task {__version__}")
        return 0
    if args.show_completion:
        print(_completion_script(args.show_completion).rstrip())
        return 0
    if not args.command:
        parser.print_help()
        return 0
    try:
        return int(args.func(args))
    except RepoTaskError as error:
        print_error(str(error))
        return 1
    except KeyboardInterrupt:
        print("Interrupted.", file=sys.stderr)
        return 130


def _completion_script(shell: str) -> str:
    commands = "init doctor start context investigate review cr status list"
    options = "--version --show-completion"
    if shell == "zsh":
        return f"""#compdef repo-task

_repo_task() {{
  local -a commands
  commands=({commands})
  _arguments '1:command:(({commands}))' '*::arg:->args'
}}
compdef _repo_task repo-task
"""
    if shell == "fish":
        lines = [f"complete -c repo-task -l {option[2:]}" for option in options.split()]
        lines.extend(f"complete -c repo-task -f -a {command}" for command in commands.split())
        return "\n".join(lines) + "\n"
    return f"""_repo_task_completion() {{
  local cur
  COMPREPLY=()
  cur="${{COMP_WORDS[COMP_CWORD]}}"
  if [[ $COMP_CWORD -eq 1 ]]; then
    COMPREPLY=( $(compgen -W "{options} {commands}" -- "$cur") )
  fi
}}
complete -F _repo_task_completion repo-task
"""


if __name__ == "__main__":
    raise SystemExit(run())
