use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn find_gix_binary(base: &Path) -> Result<PathBuf> {
    find_named_binary("gix", "AGT_GIX_PATH", base)
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
        base.join("vendor/gitoxide/target/release").join(&bin_name),
        base.join("vendor/gitoxide/target/debug").join(&bin_name),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    anyhow::bail!("{name} binary not found; set {env_var} or build it in target/ or vendor/")
}

pub fn repo_base_path(repo: &gix::Repository) -> PathBuf {
    repo.work_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo.common_dir().to_path_buf())
}
