use crate::commands::git_porcelain;
use crate::config::AgtConfig;
use crate::gix_cli::find_git_binary;
use crate::logging::{debug_log, is_enabled};
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
    let is_git_mode = _is_git_mode;
    debug_log(&format!(
        "passthrough: start is_git_mode={is_git_mode} disable_filter={disable_filter} args={args:?}"
    ));
    if args.is_empty() {
        // Show help if no git command provided
        Command::new(find_git_binary()?).arg("--help").status()?;
        return Ok(());
    }
    if is_git_mode && args.first().map(String::as_str) == Some("worktree") {
        anyhow::bail!("worktree operations are disabled in git mode");
    }

    if is_git_mode && git_porcelain::maybe_handle_git_command(args, repo)? {
        debug_log("passthrough: handled by git_porcelain");
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
    debug_log(&format!(
        "passthrough: delegating to {} args={args:?}",
        git_binary.display()
    ));

    // Spawn git with stdout piped for filtering, stderr inherited
    let mut child = match Command::new(&git_binary)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            debug_log(&format!(
                "passthrough: delegated command spawn failed path={} args={args:?} error={err}",
                git_binary.display()
            ));
            return Err(err.into());
        }
    };
    debug_log("passthrough: host git spawned");

    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    if is_git_mode && !disable_filter {
        // Line-by-line filtering for commands that need it
        match cmd_name {
            "branch" | "tag" => {
                debug_log(&format!(
                    "passthrough: filtering line-based output for {cmd_name}"
                ));
                for line in reader.lines() {
                    let line = line?;
                    if !has_branch_prefix(&line, &config.branch_prefix) {
                        println!("{}", line);
                    } else if is_enabled() {
                        debug_log(&format!("filtered {cmd_name} line: {line}"));
                    }
                }
            }
            "log" => {
                // For log, we need to buffer blocks since commits span multiple lines
                debug_log("passthrough: buffering log output for filtering");
                let output: String = reader.lines().collect::<Result<Vec<_>, _>>()?.join("\n");
                let filtered = filter_log_output(&output, config);
                print!("{}", filtered);
            }
            _ => {
                // No filtering for other commands
                debug_log(&format!(
                    "passthrough: passthrough line copy for {cmd_name}"
                ));
                for line in reader.lines() {
                    println!("{}", line?);
                }
            }
        }
    } else {
        // No filtering - just pass through
        debug_log("passthrough: unfiltered line copy");
        for line in reader.lines() {
            println!("{}", line?);
        }
    }

    debug_log("passthrough: waiting for host git exit");
    let status = child.wait()?;
    if status.success() {
        debug_log(&format!(
            "passthrough: delegated command succeeded path={} args={args:?} code={:?}",
            git_binary.display(),
            status.code()
        ));
    } else {
        debug_log(&format!(
            "passthrough: delegated command failed path={} args={args:?} code={:?}",
            git_binary.display(),
            status.code()
        ));
    }
    process::exit(status.code().unwrap_or(1));
}

fn filter_log_output(output: &str, config: &AgtConfig) -> String {
    if !output.contains("Author:") {
        if is_enabled() {
            debug_log("log output not parseable for author filtering; leaving unfiltered");
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
        } else if is_enabled() {
            debug_log("filtered log commit block (agent author)");
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
