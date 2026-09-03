//! Progress rendering contract tests for `agt-snapshot save` (issue #28).
//!
//! The test runner pipes stdout/stderr, so these tests exercise the
//! non-TTY half of the gating truth table byte-for-byte: no progress line
//! may ever reach stderr, with or without `-v`, and stdout must be
//! unchanged. The full gating matrix (including TTY cases) is unit-tested
//! exhaustively in `crates/agt-core/src/progress.rs`.
#![cfg(unix)]

use assert_cmd::Command as AgtCommand;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn snapshot_cmd() -> AgtCommand {
    AgtCommand::new(env!("CARGO_BIN_EXE_agt-snapshot"))
}

fn setup_plain_dir() -> Result<TempDir, Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;
    fs::write(tmp.path().join("a.txt"), "alpha")?;
    fs::write(tmp.path().join("b.txt"), "beta")?;
    fs::create_dir(tmp.path().join("sub"))?;
    fs::write(tmp.path().join("sub/c.txt"), "gamma")?;
    Ok(tmp)
}

fn run_save(dir: &Path, extra_args: &[&str]) -> Result<std::process::Output, std::io::Error> {
    snapshot_cmd().args(extra_args).current_dir(dir).output()
}

#[test]
fn save_non_tty_stderr_has_no_progress_line() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = setup_plain_dir()?;

    let output = run_save(tmp.path(), &["save"])?;

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        !stderr.contains("snapshot:"),
        "non-TTY save must not render a progress line, got stderr: {stderr}"
    );
    Ok(())
}

#[test]
fn save_verbose_non_tty_has_no_tui() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = setup_plain_dir()?;

    let output = run_save(tmp.path(), &["save", "-v"])?;

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        !stderr.contains("snapshot:"),
        "-v on a non-TTY must not render a TUI, got stderr: {stderr}"
    );
    Ok(())
}

#[test]
fn save_no_porceline_non_tty_has_no_progress_line() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = setup_plain_dir()?;

    let output = snapshot_cmd()
        .args(["save", "-v"])
        .env("AGT_NO_PORCELINE", "1")
        .current_dir(tmp.path())
        .output()?;

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        !stderr.contains("snapshot:"),
        "AGT_NO_PORCELINE must suppress progress, got stderr: {stderr}"
    );
    Ok(())
}

#[test]
fn save_verbose_stdout_unchanged() -> Result<(), Box<dyn std::error::Error>> {
    let plain = setup_plain_dir()?;
    let verbose = setup_plain_dir()?;

    let plain_out = run_save(plain.path(), &["save", "-m", "msg"])?;
    let verbose_out = run_save(verbose.path(), &["save", "-v", "-m", "msg"])?;

    assert!(plain_out.status.success());
    assert!(verbose_out.status.success());

    let plain_stdout = String::from_utf8(plain_out.stdout)?;
    let verbose_stdout = String::from_utf8(verbose_out.stdout)?;

    let plain_lines: Vec<&str> = plain_stdout.lines().collect();
    let verbose_lines: Vec<&str> = verbose_stdout.lines().collect();
    assert_eq!(plain_lines.len(), verbose_lines.len(), "same line count");

    // The tag line differs by timestamp, everything else must be identical
    // in shape: Store:, Files:, Ignored: lines are byte-stable.
    assert!(plain_lines[0].starts_with("Saved snapshot 0"));
    assert!(verbose_lines[0].starts_with("Saved snapshot 0"));
    assert!(
        plain_lines[1].starts_with("Store: ") && verbose_lines[1].starts_with("Store: "),
        "both runs report the store path"
    );
    assert_eq!(plain_lines[2], "Files: 3");
    assert_eq!(verbose_lines[2], "Files: 3");
    assert_eq!(plain_lines[3], "Ignored: 0");
    assert_eq!(verbose_lines[3], "Ignored: 0");

    Ok(())
}
