use crate::commands::git_porcelain;
use crate::config::AgtConfig;
use crate::gix_cli::find_git_binary;
use anyhow::Result;
use gix::Repository;
use std::io::{BufRead, BufReader};
use std::process::{self, Command, Stdio};

pub fn run(
    args: &[String],
    _is_git_mode: bool,
    disable_filter: bool,
    config: &AgtConfig,
    repo: &Repository,
) -> Result<()> {
    if args.is_empty() {
        // Show help if no git command provided
        Command::new(find_git_binary()?)
            .arg("--help")
            .status()?;
        return Ok(());
    }

    let is_git_mode = _is_git_mode;
    if is_git_mode && args.first().map(String::as_str) == Some("worktree") {
        anyhow::bail!("worktree operations are disabled in git mode");
    }

    if is_git_mode && git_porcelain::maybe_handle_git_command(args, repo)? {
        return Ok(());
    }

    if is_git_mode
        && !disable_filter
        && args.first().map(String::as_str) == Some("log")
        && args
            .iter()
            .any(|a| a == "--oneline" || a.starts_with("--pretty") || a.starts_with("--format"))
    {
        anyhow::bail!(
            "git log filtering is only supported for the default log format; rerun without custom formatting or use --disable-agt"
        );
    }

    let git_binary = find_git_binary()?;
    let cmd_name = args.first().map(String::as_str).unwrap_or("");

    // Spawn git with stdout piped for filtering, stderr inherited
    let mut child = Command::new(&git_binary)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    if is_git_mode && !disable_filter {
        // Line-by-line filtering for commands that need it
        match cmd_name {
            "branch" | "tag" => {
                for line in reader.lines() {
                    let line = line?;
                    if !has_branch_prefix(&line, &config.branch_prefix) {
                        println!("{}", line);
                    } else if debug_enabled() {
                        eprintln!("[agt] filtered {} line: {}", cmd_name, line);
                    }
                }
            }
            "log" => {
                // For log, we need to buffer blocks since commits span multiple lines
                let output: String = reader.lines().collect::<Result<Vec<_>, _>>()?.join("\n");
                let filtered = filter_log_output(&output, config);
                print!("{}", filtered);
            }
            _ => {
                // No filtering for other commands
                for line in reader.lines() {
                    println!("{}", line?);
                }
            }
        }
    } else {
        // No filtering - just pass through
        for line in reader.lines() {
            println!("{}", line?);
        }
    }

    let status = child.wait()?;
    process::exit(status.code().unwrap_or(1));
}

fn filter_log_output(output: &str, config: &AgtConfig) -> String {
    if !output.contains("Author:") {
        if debug_enabled() {
            eprintln!("[agt] log output not parseable for author filtering; leaving unfiltered");
        }
        return output.to_string();
    }

    let mut blocks = Vec::new();
    let mut current = Vec::new();

    for line in output.lines() {
        if line.starts_with("commit ") && !current.is_empty() {
            blocks.push(current);
            current = Vec::new();
        }
        current.push(line);
    }
    if !current.is_empty() {
        blocks.push(current);
    }

    let mut kept = Vec::new();
    for block in blocks {
        let mut hide = false;
        for line in &block {
            if line.starts_with("Author:") && line.contains(&config.agent_email) {
                hide = true;
                break;
            }
        }
        if !hide {
            kept.push(block.join("\n"));
        } else if debug_enabled() {
            eprintln!("[agt] filtered log commit block (agent author)");
        }
    }

    kept.join("\n")
}

fn has_branch_prefix(line: &str, prefix: &str) -> bool {
    let trimmed = line
        .trim_start()
        .trim_start_matches(['*', '+'])
        .trim_start();
    trimmed.starts_with(prefix) || trimmed.contains(&format!("/{prefix}"))
}

fn debug_enabled() -> bool {
    std::env::var("AGT_DEBUG").as_deref() == Ok("1")
}
