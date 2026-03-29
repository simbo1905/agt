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
        SnapshotCommands::Status {
            store,
            ignored,
            quiet,
        } => snapshot::status(repo, store.as_deref(), ignored, quiet),
        SnapshotCommands::CheckIgnore {
            verbose,
            non_matching,
            nul,
            stdin,
            paths,
        } => {
            let format = if verbose {
                if non_matching {
                    snapshot::CheckIgnoreFormat::VerboseWithNonMatching
                } else {
                    snapshot::CheckIgnoreFormat::Verbose
                }
            } else {
                snapshot::CheckIgnoreFormat::Plain
            };
            let terminator = if nul {
                snapshot::CheckIgnoreTerminator::Nul
            } else {
                snapshot::CheckIgnoreTerminator::Newline
            };
            let source = if stdin {
                snapshot::CheckIgnoreSource::Stdin
            } else {
                snapshot::CheckIgnoreSource::Paths
            };
            snapshot::check_ignore(
                repo,
                snapshot::CheckIgnoreOptions {
                    format,
                    terminator,
                    source,
                },
                &paths,
            )
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
