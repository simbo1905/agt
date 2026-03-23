use clap::ArgAction;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "agt")]
#[command(about = "Agent Git Tool - AI agent session management with immutable snapshots")]
#[command(version = env!("AGT_BUILD_VERSION"))]
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
    /// Bootstrap standalone snapshot storage for the current directory
    Setup {
        /// Snapshot store directory to create
        #[arg(long)]
        store: Option<PathBuf>,
    },

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

    /// Snapshot commands for generated output and restore
    #[command(subcommand)]
    Snapshot(SnapshotCommands),

    /// Show agt-specific status
    Status,
}

#[derive(Subcommand, Clone)]
pub enum SnapshotCommands {
    /// Save a filesystem snapshot into the snapshot store
    Save {
        /// Directory to scan
        #[arg(long, default_value = ".")]
        target: PathBuf,
        /// Override snapshot store location
        #[arg(long)]
        store: Option<PathBuf>,
        /// Message stored with the snapshot tag
        #[arg(short = 'm', long)]
        message: Option<String>,
    },

    /// Compare two saved snapshots and report deleted, modified, and added paths
    Diff {
        /// Earlier snapshot tag (or newer if you want additions reported as deletions)
        #[arg(value_name = "snapshot-a")]
        before: String,
        /// Later snapshot tag (or older if you want deletions reported as additions)
        #[arg(value_name = "snapshot-b")]
        after: String,
        /// Override snapshot store location
        #[arg(long)]
        store: Option<PathBuf>,
    },

    /// Compare the current filesystem state against the latest snapshot
    Status {
        /// Override snapshot store location
        #[arg(long)]
        store: Option<PathBuf>,
        /// Reduce output; repeat for no output and exit status only
        #[arg(short = 'q', action = ArgAction::Count)]
        quiet: u8,
    },

    /// List saved standalone snapshots
    List {
        /// Reduce output; show only tags without messages
        #[arg(short = 'q', action = ArgAction::Count)]
        quiet: u8,
        /// Override snapshot store location
        #[arg(long)]
        store: Option<PathBuf>,
    },

    /// Restore files from a saved snapshot
    Restore {
        /// Snapshot tag name to restore from
        #[arg(long)]
        snapshot: String,
        /// Directory to restore into
        #[arg(long, default_value = ".")]
        target: PathBuf,
        /// Restore only selected paths within the snapshot
        #[arg(long)]
        path: Vec<PathBuf>,
        /// Override snapshot store location
        #[arg(long)]
        store: Option<PathBuf>,
    },
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

    /// Restore session to a prior shadow commit state
    Restore {
        #[arg(long)]
        session_id: String,
        /// Shadow commit SHA to restore to
        #[arg(long)]
        commit: String,
    },

    /// List sessions
    List,
}
