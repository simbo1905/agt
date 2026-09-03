//! Regression test for issue #14: `agt clone <https-url>` must fail with a
//! network error, not the gix "not compiled in" transport error.
//!
//! CI-safe: it never touches the network. Port 1 on 127.0.0.1 refuses the
//! connection immediately, so the clone fails fast at the transport layer.

#[cfg(unix)]
use assert_cmd::Command as AgtCommand;
#[cfg(unix)]
use tempfile::TempDir;

#[cfg(unix)]
#[test]
fn clone_over_https_reports_network_error_not_missing_feature(
) -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new()?;

    let output = AgtCommand::new(assert_cmd::cargo::cargo_bin!("agt"))
        .args(["clone", "https://127.0.0.1:1/x"])
        .current_dir(tmp.path())
        .output()?;

    assert!(
        !output.status.success(),
        "clone of an unreachable https URL should fail"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("not compiled in"),
        "agt clone https:// must not report the missing-transport error, got: {stderr}"
    );
    assert!(
        stderr.contains("error"),
        "expected a network-style error for an unreachable https URL, got: {stderr}"
    );

    Ok(())
}
