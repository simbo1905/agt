mod cli;

use crate::cli::{Cli, Commands, SnapshotCommands};
use agt_core::config::AgtConfig;
use agt_core::snapshot;
use anyhow::{Context, Result};
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Setup { store } => snapshot::setup(store.as_deref()),
        Commands::Snapshot(command) => run(command),
    }
}

fn run(command: SnapshotCommands) -> Result<()> {
    match command {
        SnapshotCommands::Save {
            target,
            store,
            message,
            verbose,
        } => {
            let config = AgtConfig::load().with_context(|| "Failed to load AGT configuration")?;
            snapshot::save(
                &config,
                &target,
                store.as_deref(),
                message.as_deref(),
                verbose,
            )
        }
        SnapshotCommands::Diff {
            before,
            after,
            store,
        } => snapshot::check(&before, &after, store.as_deref()),
        SnapshotCommands::Status {
            store,
            ignored,
            quiet,
        } => snapshot::status(store.as_deref(), ignored, quiet),
        SnapshotCommands::CheckIgnore {
            verbose,
            non_matching,
            nul,
            stdin,
            paths,
        } => snapshot::check_ignore_flags(verbose, non_matching, nul, stdin, &paths),
        SnapshotCommands::List { store, quiet } => snapshot::list(store.as_deref(), quiet),
        SnapshotCommands::Restore {
            snapshot: snapshot_name,
            target,
            path,
            store,
        } => snapshot::restore(&snapshot_name, &target, &path, store.as_deref()),
    }
}
