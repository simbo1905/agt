use clap::{ArgAction, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "agt-snapshot")]
#[command(
    about = "Standalone filesystem snapshot tool - save, diff, and restore directory snapshots"
)]
#[command(version = env!("AGT_BUILD_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    /// Bootstrap standalone snapshot storage for the current directory
    Setup {
        /// Snapshot store directory to create
        #[arg(long)]
        store: Option<PathBuf>,
    },

    /// Snapshot commands for generated output and restore
    #[command(flatten)]
    Snapshot(SnapshotCommands),
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
        /// Show progress with an accurate percentage (TTY only; enables a
        /// metadata pre-walk before the snapshot walk)
        #[arg(short = 'v', long)]
        verbose: bool,
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
        /// Show paths currently ignored by snapshot rules instead of diffing
        #[arg(long)]
        ignored: bool,
        /// Reduce output; repeat for no output and exit status only
        #[arg(short = 'q', action = ArgAction::Count)]
        quiet: u8,
    },

    /// Check whether snapshot ignore rules would skip the given paths
    CheckIgnore {
        /// Show source file, line, and matching pattern
        #[arg(short = 'v', long)]
        verbose: bool,
        /// Also print paths with no matching rule (verbose mode only)
        #[arg(short = 'n', long)]
        non_matching: bool,
        /// Use NUL-delimited machine-readable output
        #[arg(short = 'z')]
        nul: bool,
        /// Read paths from standard input
        #[arg(long)]
        stdin: bool,
        /// Paths to inspect
        #[arg(value_name = "path")]
        paths: Vec<String>,
    },

    /// List saved standalone snapshots
    List {
        /// Override snapshot store location
        #[arg(long)]
        store: Option<PathBuf>,
        /// Print only snapshot tags
        #[arg(short = 'q', long)]
        quiet: bool,
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
