use crate::cli::SnapshotCommands;
use crate::config::AgtConfig;
use crate::snapshot;
use anyhow::Result;
use gix::Repository;

pub fn run(repo: &Repository, command: SnapshotCommands, config: &AgtConfig) -> Result<()> {
    match command {
        SnapshotCommands::Save {
            target,
            store,
            message,
        } => snapshot::save(repo, config, &target, store.as_deref(), message.as_deref()),
        SnapshotCommands::Diff {
            before,
            after,
            store,
        } => snapshot::check(repo, &before, &after, store.as_deref()),
        SnapshotCommands::Status { store, quiet } => {
            snapshot::status(repo, store.as_deref(), quiet)
        }
        SnapshotCommands::List { store, quiet } => snapshot::list(repo, store.as_deref(), quiet),
        SnapshotCommands::Restore {
            snapshot: snapshot_name,
            target,
            path,
            store,
        } => snapshot::restore(repo, &snapshot_name, &target, &path, store.as_deref()),
    }
}
