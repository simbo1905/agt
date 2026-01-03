use crate::config::AgtConfig;
use crate::gix_cli::{find_gix_binary, repo_base_path};
use anyhow::Result;
use gix::Repository;
use std::io::Write;
use std::process;

pub fn run(
    args: &[String],
    _is_git_mode: bool,
    disable_filter: bool,
    config: &AgtConfig,
    repo: &Repository,
) -> Result<()> {
    if args.is_empty() {
        // Show help if no git command provided
        process::Command::new(find_gix_binary(&repo_base_path(repo))?)
            .arg("--help")
            .status()?;
        return Ok(());
    }

    let is_git_mode = _is_git_mode;
    if is_git_mode && args.first().map(String::as_str) == Some("worktree") {
        anyhow::bail!("worktree operations are disabled in git mode");
    }

    if is_git_mode && !disable_filter && args.first().map(String::as_str) == Some("log") {
        if args
            .iter()
            .any(|a| a == "--oneline" || a.starts_with("--pretty") || a.starts_with("--format"))
        {
            anyhow::bail!(
                "git log filtering is only supported for the default log format; rerun without custom formatting or use --disable-agt"
            );
        }
    }

    let mapped_args = map_args_for_gix(args);
    let output = process::Command::new(find_gix_binary(&repo_base_path(repo))?)
        .args(&mapped_args)
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    if is_git_mode && !disable_filter {
        // Filter output based on command
        let filtered = filter_output(&stdout, &mapped_args, config);
        print!("{filtered}");
    } else {
        print!("{stdout}");
    }

    std::io::stderr().write_all(&output.stderr)?;

    if let Some(code) = output.status.code() {
        process::exit(code);
    }

    Ok(())
}

fn map_args_for_gix(args: &[String]) -> Vec<String> {
    let mut mapped = args.to_vec();
    match mapped.first().map(String::as_str) {
        Some("branch") => {
            if mapped.get(1).map(String::as_str) != Some("list") {
                mapped.insert(1, "list".to_string());
            }
        }
        Some("tag") => {
            if mapped.get(1).map(String::as_str) != Some("list") {
                mapped.insert(1, "list".to_string());
            }
        }
        _ => {}
    }
    mapped
}

fn filter_output(output: &str, args: &[String], config: &AgtConfig) -> String {
    let cmd = args.first().map_or("", std::string::String::as_str);

    match cmd {
        "branch" => filter_branch_output(output, config),
        "tag" => filter_tag_output(output, config),
        "log" => filter_log_output(output, config),
        _ => output.to_string(),
    }
}

fn filter_branch_output(output: &str, config: &AgtConfig) -> String {
    output
        .lines()
        .filter(|line| {
            let hide = has_branch_prefix(line, &config.branch_prefix);
            if hide && debug_enabled() {
                eprintln!("[agt] filtered branch line: {line}");
            }
            !hide
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn filter_tag_output(output: &str, config: &AgtConfig) -> String {
    output
        .lines()
        .filter(|line| {
            let hide = has_branch_prefix(line, &config.branch_prefix);
            if hide && debug_enabled() {
                eprintln!("[agt] filtered tag line: {line}");
            }
            !hide
        })
        .collect::<Vec<_>>()
        .join("\n")
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
