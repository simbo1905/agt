use anyhow::{Context, Result};
use clap::Parser;

mod cli;
mod commands;
mod config;
mod filter;
mod gix_cli;
mod scanner;

pub use cli::*;

fn main() -> Result<()> {
    // Dual-mode detection based on how the binary was invoked
    let invoked_as = std::env::args().next().unwrap_or_default();
    let is_git_mode = invoked_as.contains("git") && !invoked_as.contains("agt");

    // Parse CLI arguments
    let cli = Cli::parse();

    // Set up working directory
    if let Some(dir) = cli.directory {
        std::env::set_current_dir(&dir)
            .with_context(|| format!("Failed to change to directory: {}", dir.display()))?;
    }

    // Handle init command before discovering repo (init doesn't need existing repo)
    if let Some(Commands::Init { remote_url, path }) = cli.command {
        let config = config::AgtConfig::default();
        return commands::init::run(&remote_url, path.as_deref(), &config);
    }

    // Determine if filtering should be disabled
    let disable_filter = cli.disable_agt || std::env::var("AGT_DISABLE_FILTER").is_ok();

    // Load configuration
    let repo = gix::discover(".").with_context(|| "Failed to discover Git repository")?;
    let config =
        config::AgtConfig::load(&repo).with_context(|| "Failed to load AGT configuration")?;

    // Route to appropriate command handler
    match cli.command {
        Some(Commands::Init { .. }) => unreachable!(), // Handled above
        Some(Commands::Fork { session_id, from }) => {
            commands::fork::run(&repo, &session_id, from.as_deref(), &config)
        }
        Some(Commands::Autocommit {
            session_id,
            timestamp,
            dry_run,
        }) => {
            let worktree_path = std::env::current_dir()?;
            commands::autocommit::run(
                &repo,
                &worktree_path,
                &session_id,
                timestamp,
                dry_run,
                &config,
            )
        }
        Some(Commands::ListSessions) => commands::list_sessions::run(&repo, &config),
        Some(Commands::PruneSession {
            session_id,
            delete_branch,
        }) => commands::prune_session::run(&repo, &session_id, delete_branch, &config),
        Some(Commands::Status) => commands::status::run(&repo, &config),
        None => {
            // Git passthrough mode
            commands::passthrough::run(&cli.args, is_git_mode, disable_filter, &config, &repo)
        }
    }
}
