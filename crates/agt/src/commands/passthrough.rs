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

    // Check if -a or --all flag is present (show all branches including agent branches)
    let show_all = args.iter().any(|a| a == "-a" || a == "--all");

    if !disable_filter && !show_all {
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
        .filter(|line| !line.contains(&config.branch_prefix))
        .collect::<Vec<_>>()
        .join("\n")
}

fn filter_tag_output(output: &str, config: &AgtConfig) -> String {
    output
        .lines()
        .filter(|line| !line.contains(&config.branch_prefix))
        .collect::<Vec<_>>()
        .join("\n")
}

fn filter_log_output(output: &str, _config: &AgtConfig) -> String {
    // This is a simplified filter - in a real implementation, we'd parse
    // the commit objects and check author emails
    output.to_string()
}
