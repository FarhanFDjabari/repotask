//! RepoTask command line entry point.
//!
//! Every command is agent-first: `--json` returns a stable envelope (see `output`),
//! and the CLI never calls a language model. It locates, parses, budgets, and returns.

mod commands;
mod config;
mod connectors;
mod discovery;
mod git;
mod index;
mod kb;
mod output;
mod skills;
mod workflow;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

use workflow::dedupe::MIN_SIMILARITY;

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
    /// Create `.repo-task/config.yaml` from repository discovery.
    Init {
        /// Knowledge base git URL. Omit to use an in-project directory.
        #[arg(long, default_value = "")]
        remote: String,
        /// In-project knowledge base directory.
        #[arg(long, default_value = ".repo-task/knowledge")]
        local: String,
        /// Override detected stacks. Repeatable.
        #[arg(long = "stack")]
        stacks: Vec<String>,
        /// Overwrite an existing configuration.
        #[arg(long)]
        force: bool,
        /// Preview without writing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Check configuration, knowledge base reachability, and tooling.
    Doctor,
    /// Upgrade a schema version 1 `.repo-task.yml` to `.repo-task/config.yaml`.
    Migrate {
        #[arg(long)]
        dry_run: bool,
    },
    /// Read architecture and stack conventions for this project.
    Convention {
        /// Convention id or search topic.
        topic: String,
        #[arg(long, default_value_t = 3)]
        limit: usize,
    },
    /// Read the project playbook for a specific task.
    Recipe {
        /// Recipe id or the task you want a playbook for.
        task: String,
        #[arg(long, default_value_t = 3)]
        limit: usize,
    },
    /// Search conventions and recipes; returns ids to read with `convention` or `recipe`.
    Search {
        /// Keywords to search across the knowledge base.
        query: String,
        /// Restrict to `convention` or `recipe`.
        #[arg(long, default_value = "")]
        layer: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Scan the project and write fact families into the knowledge base.
    Index {
        /// Index only files changed against the base branch.
        #[arg(long)]
        changed_only: bool,
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
    /// Read a curated project fact family produced by `index`.
    Fact {
        /// Fact family name. Omit to list available families.
        #[arg(default_value = "")]
        family: String,
        /// Filter entries by name substring.
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 100)]
        limit: usize,
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
    /// Pull a ticket or PRD into the local work directory.
    Fetch {
        ticket: String,
        /// Connector name. Defaults to the only one.
        #[arg(long, default_value = "")]
        system: String,
        /// Store this content as the source ('-' reads stdin).
        #[arg(long, default_value = "")]
        write: String,
    },
    /// Return the source plus the project's own context, filtered to this project's stacks.
    Summarize {
        ticket: String,
        /// Store this content as the summary ('-' reads stdin).
        #[arg(long, default_value = "")]
        write: String,
        #[arg(long)]
        budget: Option<usize>,
    },
    /// Compute the impact set from the indexed project facts.
    Analyze {
        ticket: String,
        #[arg(long, default_value_t = 40)]
        limit: usize,
    },
    /// Group the impact set into incremental, independently reviewable steps.
    Split {
        ticket: String,
        #[arg(long, default_value_t = 8)]
        max_files: usize,
    },
    /// Call a config-declared external system (REST first, MCP hint as fallback).
    Connect {
        /// System name as declared under `connectors:`.
        system: String,
        /// Declared verb. Omit to list what is available.
        #[arg(default_value = "")]
        verb: String,
        /// Verb arguments as key=value. Repeatable.
        #[arg(long = "arg")]
        args: Vec<String>,
    },
    /// Read design structure and map it to this project's code.
    Design {
        #[command(subcommand)]
        command: DesignCommand,
    },
    /// Bugfix workflow.
    Bug {
        #[command(subcommand)]
        command: BugCommand,
    },
    /// Manage the knowledge base source.
    Kb {
        #[command(subcommand)]
        command: KbCommand,
    },
    /// Generate agent skill files.
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
    /// Print a shell completion script, e.g. `repo-task completions zsh`.
    Completions {
        /// bash, zsh, fish, elvish, or powershell.
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum DesignCommand {
    /// Distilled structure of the whole file.
    File {
        #[arg(long, default_value_t = 3)]
        depth: usize,
        /// File key or link, overriding the configured one.
        #[arg(long, default_value = "")]
        file: String,
    },
    /// Distilled structure of one node.
    Node {
        node: String,
        #[arg(long, default_value_t = 5)]
        depth: usize,
        /// File key or link, overriding the configured one.
        #[arg(long, default_value = "")]
        file: String,
    },
    /// Design tokens behind colours, spacing, and typography.
    Variables {
        /// File key or link, overriding the configured one.
        #[arg(long, default_value = "")]
        file: String,
    },
    /// Rendered frame URLs for the agent to look at.
    Image {
        node: String,
        #[arg(long, default_value = "2")]
        scale: String,
        /// File key or link, overriding the configured one.
        #[arg(long, default_value = "")]
        file: String,
    },
    /// Line design component names up against the indexed code components.
    Map {
        /// Design component names, e.g. `Button/Primary`. Repeatable.
        names: Vec<String>,
        #[arg(long)]
        budget: Option<usize>,
    },
}

#[derive(Subcommand)]
enum BugCommand {
    /// Fetch a bug report together with the code and conventions it implicates.
    Fetch {
        ticket: String,
        #[arg(long, default_value = "")]
        system: String,
        #[arg(long, default_value = "")]
        write: String,
        #[arg(long)]
        budget: Option<usize>,
    },
    /// Group open bug tickets that resolve to the same code.
    Dedupe {
        #[arg(long, default_value = "")]
        system: String,
        /// Provider query selecting the tickets.
        #[arg(long, default_value = "")]
        query: String,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Minimum similarity to group two tickets.
        #[arg(long, default_value_t = MIN_SIMILARITY)]
        threshold: f64,
        /// Read tickets as JSON instead of calling a connector ('-' is stdin).
        #[arg(long = "tickets", default_value = "")]
        tickets_file: String,
    },
}

#[derive(Subcommand)]
enum SkillsCommand {
    /// Write Claude Code skills and the AGENTS.md block for this project.
    Sync {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum KbCommand {
    /// Scaffold a starter knowledge base: conventions, recipes, slices, and kb.yaml.
    Init {
        /// Where to scaffold. Defaults to the configured local directory.
        #[arg(long, default_value = "")]
        path: String,
        #[arg(long)]
        force: bool,
    },
    /// Clone or fetch the knowledge base and pin it to the configured ref.
    Sync,
    /// Show the resolved knowledge base without touching the network.
    Status,
    /// Stage generated facts in the knowledge base worktree and print PR instructions.
    Propose {
        #[arg(long, short, default_value = "chore: refresh project facts")]
        message: String,
        /// Branch name; defaults to a timestamped one.
        #[arg(long, default_value = "")]
        branch: String,
    },
}

/// Returns the command name for the envelope and whether the run succeeded.
fn dispatch(command: &Command) -> (&'static str, Result<bool>) {
    match command {
        Command::Init {
            remote,
            local,
            stacks,
            force,
            dry_run,
        } => (
            "init",
            commands::setup::init(remote, local, stacks, *force, *dry_run).map(|_| true),
        ),
        Command::Doctor => ("doctor", commands::setup::doctor()),
        Command::Migrate { dry_run } => {
            ("migrate", commands::setup::migrate(*dry_run).map(|_| true))
        }
        Command::Convention { topic, limit } => (
            "convention",
            commands::query::convention(topic, *limit).map(|_| true),
        ),
        Command::Recipe { task, limit } => (
            "recipe",
            commands::query::recipe(task, *limit).map(|_| true),
        ),
        Command::Search {
            query,
            layer,
            limit,
        } => (
            "search",
            commands::query::search(query, layer, *limit).map(|_| true),
        ),
        Command::Index { changed_only } => {
            ("index", commands::facts::index(*changed_only).map(|_| true))
        }
        Command::Symbol { query, kind, limit } => (
            "symbol",
            commands::facts::symbol(query, kind.as_deref(), *limit).map(|_| true),
        ),
        Command::Fact {
            family,
            query,
            limit,
        } => (
            "fact",
            commands::facts::fact(family, query, *limit).map(|_| true),
        ),
        Command::Brief {
            intent,
            paths,
            changed,
            budget,
        } => (
            "brief",
            commands::facts::brief(intent, paths, *changed, *budget).map(|_| true),
        ),
        Command::Fetch {
            ticket,
            system,
            write,
        } => (
            "fetch",
            commands::work::fetch(ticket, system, write).map(|_| true),
        ),
        Command::Summarize {
            ticket,
            write,
            budget,
        } => (
            "summarize",
            commands::work::summarize(ticket, write, *budget).map(|_| true),
        ),
        Command::Analyze { ticket, limit } => (
            "analyze",
            commands::work::analyze(ticket, *limit).map(|_| true),
        ),
        Command::Split { ticket, max_files } => (
            "split",
            commands::work::split(ticket, *max_files).map(|_| true),
        ),
        Command::Connect { system, verb, args } => (
            "connect",
            commands::connect::connect(system, verb, args).map(|_| true),
        ),
        Command::Design { command } => match command {
            DesignCommand::File { depth, file } => (
                "design",
                commands::design::design("file", "", *depth, "2", file).map(|_| true),
            ),
            DesignCommand::Node { node, depth, file } => (
                "design",
                commands::design::design("node", node, *depth, "2", file).map(|_| true),
            ),
            DesignCommand::Variables { file } => (
                "design",
                commands::design::design("variables", "", 0, "2", file).map(|_| true),
            ),
            DesignCommand::Image { node, scale, file } => (
                "design",
                commands::design::design("image", node, 0, scale, file).map(|_| true),
            ),
            DesignCommand::Map { names, budget } => (
                "design.map",
                commands::design::map(names, *budget).map(|_| true),
            ),
        },
        Command::Bug { command } => match command {
            BugCommand::Fetch {
                ticket,
                system,
                write,
                budget,
            } => (
                "bug.fetch",
                commands::bug::bug_fetch(ticket, system, write, *budget).map(|_| true),
            ),
            BugCommand::Dedupe {
                system,
                query,
                limit,
                threshold,
                tickets_file,
            } => (
                "bug.dedupe",
                commands::bug::bug_dedupe(system, query, *limit, *threshold, tickets_file)
                    .map(|_| true),
            ),
        },
        Command::Kb { command } => match command {
            KbCommand::Init { path, force } => {
                ("kb.init", commands::kb::init(path, *force).map(|_| true))
            }
            KbCommand::Sync => ("kb.sync", commands::kb::sync().map(|_| true)),
            KbCommand::Status => ("kb.status", commands::kb::status().map(|_| true)),
            KbCommand::Propose { message, branch } => (
                "kb.propose",
                commands::kb::propose(message, branch).map(|_| true),
            ),
        },
        Command::Skills { command } => match command {
            SkillsCommand::Sync { dry_run } => (
                "skills.sync",
                commands::skills::sync(*dry_run).map(|_| true),
            ),
        },
        // `main` prints the script and returns before dispatch: a completion script is
        // shell source, so it cannot be wrapped in the envelope.
        Command::Completions { .. } => unreachable!(),
    }
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    if let Command::Completions { shell } = cli.command {
        let mut command = Cli::command();
        let name = command.get_name().to_string();
        clap_complete::generate(shell, &mut command, name, &mut std::io::stdout());
        return std::process::ExitCode::SUCCESS;
    }

    output::set_json_mode(cli.json);

    let (name, result) = dispatch(&cli.command);
    match result {
        Ok(true) => std::process::ExitCode::SUCCESS,
        // `doctor` reports its findings through the envelope and still fails the run.
        Ok(false) => std::process::ExitCode::FAILURE,
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
