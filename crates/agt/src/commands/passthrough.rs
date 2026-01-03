use crate::config::AgtConfig;
use anyhow::Result;
use std::io::Write;
use std::process;

pub fn run(
    args: &[String],
    _is_git_mode: bool,
    disable_filter: bool,
    config: &AgtConfig,
) -> Result<()> {
    if args.is_empty() {
        // Show help if no git command provided
        process::Command::new("git").arg("--help").status()?;
        return Ok(());
    }

    let output = process::Command::new("git").args(args).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let is_git_mode = _is_git_mode;
    if is_git_mode && !disable_filter {
        // Filter output based on command
        let filtered = filter_output(&stdout, args, config);
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
        .filter(|line| !has_branch_prefix(line, &config.branch_prefix))
        .collect::<Vec<_>>()
        .join("\n")
}

fn filter_tag_output(output: &str, config: &AgtConfig) -> String {
    output
        .lines()
        .filter(|line| !has_branch_prefix(line, &config.branch_prefix))
        .collect::<Vec<_>>()
        .join("\n")
}

fn filter_log_output(output: &str, config: &AgtConfig) -> String {
    if !output.contains("Author:") {
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
        }
    }

    kept.join("\n")
}

fn has_branch_prefix(line: &str, prefix: &str) -> bool {
    let trimmed = line.trim_start_matches('*').trim_start();
    trimmed.starts_with(prefix) || trimmed.contains(&format!("/{prefix}"))
}
