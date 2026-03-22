#[path = "../../build/build_version_support.rs"]
mod build_version_support;

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=AGT_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=AGT_BUILD_DATE");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../build/build_version_support.rs");

    emit_git_rerun_hints();

    let base_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION should be set");
    let short_sha = git(&["rev-parse", "--short=12", "HEAD"]);
    let dirty = git(&["status", "--porcelain"])
        .map(|status| !status.is_empty())
        .unwrap_or(false);

    let version = match env::var("AGT_BUILD_CHANNEL").ok().as_deref() {
        Some("nightly") => {
            let build_date = env::var("AGT_BUILD_DATE")
                .expect("AGT_BUILD_DATE must be set when AGT_BUILD_CHANNEL=nightly");
            let sha = short_sha.clone().unwrap_or_else(|| String::from("unknown"));
            build_version_support::format_nightly_version(&build_date, &sha)
        }
        Some(other) => panic!("unsupported AGT_BUILD_CHANNEL: {other}"),
        None => {
            if !dirty {
                if let Some(version) = exact_release_version() {
                    version
                } else if let Some(sha) = short_sha.as_deref() {
                    build_version_support::format_local_version(&base_version, sha, dirty)
                } else {
                    base_version
                }
            } else if let Some(sha) = short_sha.as_deref() {
                build_version_support::format_local_version(&base_version, sha, dirty)
            } else {
                format!("{base_version}+dirty")
            }
        }
    };

    println!("cargo:rustc-env=AGT_BUILD_VERSION={version}");
}

fn exact_release_version() -> Option<String> {
    let tag = git(&[
        "describe",
        "--tags",
        "--exact-match",
        "--match",
        "release/*",
    ])?;
    build_version_support::parse_release_ref(&tag).map(ToOwned::to_owned)
}

fn emit_git_rerun_hints() {
    let Some(git_dir) = git(&["rev-parse", "--git-dir"]) else {
        return;
    };

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let git_dir = absolutize(&manifest_dir, Path::new(&git_dir));

    for path in [
        git_dir.join("HEAD"),
        git_dir.join("index"),
        git_dir.join("packed-refs"),
        git_dir.join("refs/heads"),
        git_dir.join("refs/tags"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn absolutize(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
