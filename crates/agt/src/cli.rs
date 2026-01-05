use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "agt")]
#[command(about = "Agent Git Tool - AI agent session management with immutable snapshots")]
#[command(version = env!("CARGO_PKG_VERSION"))]
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

#[derive(Subcommand, Clone)]
pub enum Commands {
    /// Clone a remote repository into agt-managed structure
    Clone {
        /// Remote repository URL
        remote_url: String,
        /// Target directory (default: current dir)
        #[arg(long)]
        path: Option<PathBuf>,
    },

    /// Session management commands
    #[command(subcommand)]
    Session(SessionCommands),

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
        /// Comma-separated list of sibling directories to archive (e.g. "xdg,config")
        #[arg(long, value_delimiter = ',')]
        siblings: Option<Vec<String>>,
    },

    /// Show agt-specific status
    Status,
}

#[derive(Subcommand, Clone)]
pub enum SessionCommands {
    /// Create a new agent session for a fresh ticket
    New {
        #[arg(long)]
        id: Option<String>,
        /// Starting point: branch name, commit, or session ID
        #[arg(long)]
        from: Option<String>,
        /// Tool profile for folder setup
        #[arg(long, default_value = "default")]
        profile: String,
    },

    /// Export session's user branch to remote
    Export {
        #[arg(long)]
        session_id: Option<String>,
    },

    /// Remove a session
    Remove {
        #[arg(long)]
        id: String,
        #[arg(long)]
        delete_branch: bool,
    },

    /// Fork a session for parallel work
    Fork {
        #[arg(long)]
        from: String,
        #[arg(long)]
        id: Option<String>,
    },

    /// List sessions
    List,
}
