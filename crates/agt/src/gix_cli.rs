use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn find_git_binary() -> Result<PathBuf> {
    // 1. Check AGT_GIT_PATH env var first
    if let Ok(path) = std::env::var("AGT_GIT_PATH") {
        let candidate = PathBuf::from(&path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    // 2. Search PATH using `which git`
    if let Ok(output) = Command::new("which").arg("git").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    // 3. Fallback to common locations
    let fallbacks = [
        "/usr/bin/git",
        "/usr/local/bin/git",
        "/opt/homebrew/bin/git",
    ];

    for path in fallbacks {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    anyhow::bail!("git binary not found; set AGT_GIT_PATH or ensure git is in PATH")
}

pub fn find_worktree_binary(base: &Path) -> Result<PathBuf> {
    find_named_binary("agt-worktree", "AGT_WORKTREE_PATH", base)
}

fn find_named_binary(name: &str, env_var: &str, base: &Path) -> Result<PathBuf> {
    if let Ok(path) = std::env::var(env_var) {
        return Ok(PathBuf::from(path));
    }

    let exe_suffix = std::env::consts::EXE_SUFFIX;
    let bin_name = format!("{name}{exe_suffix}");

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join(&bin_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    let candidates = [
        base.join("target/release").join(&bin_name),
        base.join("target/debug").join(&bin_name),
        base.join("dist").join(&bin_name),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    anyhow::bail!("{name} binary not found; set {env_var} or build it")
}

pub fn repo_base_path(repo: &gix::Repository) -> PathBuf {
    repo.work_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo.common_dir().to_path_buf())
}
