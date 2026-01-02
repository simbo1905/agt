use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "agt")]
#[command(about = "Agent Git Tool - AI agent session management with immutable snapshots")]
pub struct Cli {
    /// Disable agt filtering (git mode only)
    #[arg(long, global = true)]
    pub disable_agt: bool,

    /// Run in directory
    #[arg(short = 'C', global = true)]
    pub directory: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Passthrough args for git commands
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new agt-managed repository
    Init {
        /// Remote repository URL
        remote_url: String,
        /// Target directory (default: current dir)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Create a new agent session with its own worktree and branch
    Fork {
        /// Unique identifier for the session
        #[arg(long)]
        session_id: String,
        /// Starting point: branch name, commit, or existing session ID
        #[arg(long)]
        from: Option<String>,
    },

    /// Auto-commit all modified files to the agent session branch
    Autocommit {
        /// Session identifier
        #[arg(long)]
        session_id: String,
        /// Override scan timestamp (Unix epoch) for testing
        #[arg(long)]
        timestamp: Option<i64>,
        /// Show what would be committed without committing
        #[arg(long)]
        dry_run: bool,
    },

    /// List all agent sessions with their status
    ListSessions,

    /// Remove an agent session's worktree and optionally its branch
    PruneSession {
        /// Session to prune
        #[arg(long)]
        session_id: String,
        /// Also delete the session branch
        #[arg(long)]
        delete_branch: bool,
    },

    /// Show agt-specific status
    Status,
}
