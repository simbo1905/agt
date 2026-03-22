use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn find_git_binary() -> Result<PathBuf> {
    // 1. Check AGT_GIT_PATH env var first
    if let Ok(path) = std::env::var("AGT_GIT_PATH") {
        let candidate = PathBuf::from(&path);
        if candidate.exists() {
            debug_log(&format!(
                "find_git_binary: using AGT_GIT_PATH {}",
                candidate.display()
            ));
            return Ok(candidate);
        }
        debug_log(&format!(
            "find_git_binary: AGT_GIT_PATH does not exist {}",
            candidate.display()
        ));
    }

    // 2. Search PATH
    #[cfg(windows)]
    if let Ok(output) = Command::new("where.exe").arg("git.exe").output() {
        if output.status.success() {
            if let Some(path) = String::from_utf8_lossy(&output.stdout)
                .lines()
                .find(|line| !line.trim().is_empty())
            {
                debug_log(&format!(
                    "find_git_binary: using where.exe result {}",
                    path.trim()
                ));
                return Ok(PathBuf::from(path.trim()));
            }
        }
        debug_log(&format!(
            "find_git_binary: where.exe status {:?}",
            output.status.code()
        ));
    }

    #[cfg(not(windows))]
    if let Ok(output) = Command::new("which").arg("git").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                debug_log(&format!("find_git_binary: using which result {}", path));
                return Ok(PathBuf::from(path));
            }
        }
        debug_log(&format!(
            "find_git_binary: which status {:?}",
            output.status.code()
        ));
    }

    // 3. Fallback to common locations
    let fallbacks = [
        #[cfg(windows)]
        "C:/Program Files/Git/bin/git.exe",
        #[cfg(windows)]
        "C:/Program Files/Git/cmd/git.exe",
        "/usr/bin/git",
        "/usr/local/bin/git",
        "/opt/homebrew/bin/git",
    ];

    for path in fallbacks {
        let p = PathBuf::from(path);
        if p.exists() {
            debug_log(&format!("find_git_binary: using fallback {}", p.display()));
            return Ok(p);
        }
    }

    debug_log("find_git_binary: failed to resolve git binary");
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

fn debug_log(message: &str) {
    if std::env::var("AGT_DEBUG").as_deref() == Ok("1") {
        eprintln!("[agt] {message}");
    }
    if let Ok(path) = std::env::var("AGT_DEBUG_LOG") {
        let _ = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .and_then(|mut file| {
                use std::io::Write;
                writeln!(file, "[agt] {message}")
            });
    }
}
