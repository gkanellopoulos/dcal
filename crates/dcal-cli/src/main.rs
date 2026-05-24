use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod commands;
mod error;
mod output;
mod resolve;

/// dcal — project lifecycle management for Claude Code
#[derive(Parser)]
#[command(name = "dcal", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show full project dashboard
    Info {
        /// Project name or ID
        target: String,
    },

    /// Setup: create ~/.dcal/ structure and personal config wizard
    Init,

    /// Print the full journal for a project
    Journal {
        /// Project name or ID
        target: String,
    },

    /// Guided project creation with CLAUDE.md generation
    New {
        /// Create project at a specific path (default: cwd/project_name)
        #[arg(long)]
        path: Option<PathBuf>,

        /// Claude Code model for sessions in this project (e.g. opus, sonnet)
        #[arg(long)]
        model: Option<String>,
    },

    /// List all registered projects
    List {
        /// Filter by status (e.g. active, paused)
        #[arg(long)]
        status: Option<String>,

        /// Show only projects inactive for N days (e.g. 30d)
        #[arg(long)]
        stale: Option<String>,
    },

    /// Resume a project with a reengagement brief
    Resume {
        /// Project name or ID
        target: String,

        /// Claude Code model for this session (e.g. opus, sonnet)
        #[arg(long)]
        model: Option<String>,
    },

    /// Pause a project
    Pause {
        /// Project name or ID
        target: String,

        /// Note to append to the journal
        #[arg(long)]
        note: Option<String>,
    },

    /// Update a project's development phase
    Phase {
        /// Project name or ID
        target: String,

        /// New phase: ideation, design, implementation, testing, maintenance
        phase: String,
    },

    /// Record a session journal entry
    Checkin {
        /// Project name or ID (manual mode)
        target: Option<String>,

        /// Run in automatic hook mode
        #[arg(long)]
        auto: bool,

        /// Detect project from current working directory
        #[arg(long)]
        project_from_cwd: bool,
    },

    /// Import an existing project into the registry
    Onboard {
        /// Path to the project directory
        path: PathBuf,
    },

    /// Search projects by name
    Search {
        /// Search string (case-insensitive substring match)
        query: String,
    },

    /// Print session history for a project
    Sessions {
        /// Project name or ID
        target: String,
    },

    /// Print the current snapshot for a project
    Snapshot {
        /// Project name or ID
        target: String,
    },

    /// Sync unprocessed CC sessions
    Sync {
        /// Project name or ID (omit to sync all)
        target: Option<String>,
    },

    #[command(external_subcommand)]
    External(Vec<OsString>),
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Info { target } => commands::info::run(target),
        Command::Init => commands::init::run(),
        Command::Journal { target } => commands::journal::run(target),
        Command::New { path, model } => commands::new::run(path, model),
        Command::List { status, stale } => commands::list::run(status, stale),
        Command::Resume { target, model } => commands::resume::run(target, model),
        Command::Pause { target, note } => commands::pause::run(target, note),
        Command::Phase { target, phase } => commands::phase::run(target, phase),
        Command::Checkin {
            target,
            auto,
            project_from_cwd,
        } => commands::checkin::run(target, auto, project_from_cwd),
        Command::Onboard { path } => commands::onboard::run(path),
        Command::Search { query } => commands::search::run(query),
        Command::Sessions { target } => commands::sessions::run(target),
        Command::Snapshot { target } => commands::snapshot::run(target),
        Command::Sync { target } => commands::sync::run(target),
        Command::External(args) => {
            let target = args
                .first()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if target.is_empty() {
                error::exit_with_error(&anyhow::anyhow!("no command provided. Run 'dcal --help' for usage."));
            }
            commands::info::run(target.to_string())
        }
    };

    if let Err(err) = result {
        error::exit_with_error(&err);
    }
}
