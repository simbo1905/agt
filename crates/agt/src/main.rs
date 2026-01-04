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

    if is_git_mode {
        return run_git_mode();
    }

    // Parse CLI arguments (agt mode)
    let cli = Cli::parse();

    // Set up working directory
    if let Some(dir) = cli.directory {
        std::env::set_current_dir(&dir)
            .with_context(|| format!("Failed to change to directory: {}", dir.display()))?;
    }

    // Handle init command before discovering repo (init doesn't need existing repo)
    if let Some(Commands::Init { remote_url, path }) = cli.command {
        let config = config::AgtConfig::load_for_init();
        return commands::init::run(&remote_url, path.as_deref(), &config);
    }

    // Determine if filtering should be disabled
    let disable_filter = cli.disable_agt || std::env::var("AGT_DISABLE_FILTER").is_ok();

    // Load configuration (from ~/.agtconfig and .agt/config)
    let config =
        config::AgtConfig::load().with_context(|| "Failed to load AGT configuration")?;

    // Discover repo
    let repo = gix::discover(".").with_context(|| "Failed to discover Git repository")?;

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

fn run_git_mode() -> Result<()> {
    // In git mode we do not use clap parsing of subcommands: we must accept arbitrary
    // git-style flags (e.g. `-c`, `--work-tree`, etc.) and pass them through.
    let mut args = Vec::<String>::new();
    let mut disable_filter = std::env::var("AGT_DISABLE_FILTER").is_ok();
    let mut directory: Option<std::path::PathBuf> = None;

    let mut it = std::env::args().skip(1).peekable();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--disable-agt" => {
                disable_filter = true;
            }
            "-C" => {
                let dir = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("Expected path after -C"))?;
                directory = Some(std::path::PathBuf::from(dir));
            }
            _ if arg.starts_with("-C") && arg.len() > 2 => {
                directory = Some(std::path::PathBuf::from(arg.trim_start_matches("-C")));
            }
            _ => args.push(arg),
        }
    }

    if let Some(dir) = directory {
        std::env::set_current_dir(&dir)
            .with_context(|| format!("Failed to change to directory: {}", dir.display()))?;
    }

    // Load configuration (from ~/.agtconfig and .agt/config)
    let config =
        config::AgtConfig::load().with_context(|| "Failed to load AGT configuration")?;

    let repo = gix::discover(".").with_context(|| "Failed to discover Git repository")?;

    commands::passthrough::run(&args, true, disable_filter, &config, &repo)
}
