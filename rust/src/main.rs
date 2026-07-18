//! RepoTask command line entry point (Rust spike: index, brief, symbol).

mod commands;
mod config;
mod git;
mod index;
mod kb;
mod output;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "repo-task",
    version,
    about = "Agent-orchestrated knowledge and workflow CLI."
)]
struct Cli {
    /// Emit the machine-readable envelope instead of human output.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Scan the project and write fact families into the knowledge base.
    Index {
        /// Index only files changed against the base branch.
        #[arg(long)]
        changed_only: bool,
    },
    /// Assemble the conventions, recipes, and project facts that apply to this work.
    Brief {
        /// What you are about to do, e.g. 'add pagination to the feed'.
        intent: String,
        /// Relevant file paths. Repeatable. Sharpens ranking.
        #[arg(long = "path")]
        paths: Vec<String>,
        /// Use files changed against the base branch as the paths.
        #[arg(long)]
        changed: bool,
        /// Token ceiling. Defaults to the project value.
        #[arg(long)]
        budget: Option<usize>,
    },
    /// Search indexed declarations by name.
    Symbol {
        /// Substring or regular expression to match.
        query: String,
        /// Restrict to one declaration kind.
        #[arg(long)]
        kind: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    output::set_json_mode(cli.json);

    let (name, result) = match &cli.command {
        Command::Index { changed_only } => ("index", commands::index(*changed_only)),
        Command::Brief {
            intent,
            paths,
            changed,
            budget,
        } => ("brief", commands::brief(intent, paths, *changed, *budget)),
        Command::Symbol { query, kind, limit } => {
            ("symbol", commands::symbol(query, kind.as_deref(), *limit))
        }
    };

    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // anyhow chains read outermost-first; join them into one line so the
            // JSON envelope carries the full context.
            let message = error
                .chain()
                .map(|cause| cause.to_string())
                .collect::<Vec<_>>()
                .join(": ");
            output::emit_error(name, &message, "error");
            std::process::ExitCode::FAILURE
        }
    }
}
