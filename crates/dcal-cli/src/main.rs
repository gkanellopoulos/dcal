use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod commands;
mod error;

/// dcal — project lifecycle management for Claude Code
#[derive(Parser)]
#[command(name = "dcal", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Setup: create ~/.dcal/ structure, personal config wizard, install hooks
    Init,

    /// Guided project creation with CLAUDE.md generation
    New {
        /// Create project at a specific path (default: cwd/project_name)
        #[arg(long)]
        path: Option<PathBuf>,
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
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Init => commands::init::run(),
        Command::New { path } => commands::new::run(path),
        Command::List { status, stale } => commands::list::run(status, stale),
        Command::Resume { target } => commands::resume::run(target),
        Command::Pause { target, note } => commands::pause::run(target, note),
        Command::Phase { target, phase } => commands::phase::run(target, phase),
        Command::Checkin {
            target,
            auto,
            project_from_cwd,
        } => commands::checkin::run(target, auto, project_from_cwd),
        Command::Onboard { path } => commands::onboard::run(path),
    };

    if let Err(err) = result {
        error::exit_with_error(&err);
    }
}
