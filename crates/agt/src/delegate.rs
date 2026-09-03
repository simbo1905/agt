//! Git-style PATH delegation helpers: find `<program>-<name>` executables on
//! PATH and execute them, replacing this process on unix so stdio and exit
//! codes propagate by construction.

#[cfg(unix)]
use anyhow::Context;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Search `$PATH` for an executable named `program` and return its path.
pub fn find_on_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(program);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        if !program.ends_with(std::env::consts::EXE_SUFFIX) {
            let with_exe = dir.join(format!("{program}{}", std::env::consts::EXE_SUFFIX));
            if is_executable_file(&with_exe) {
                return Some(with_exe);
            }
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(windows)]
    path.is_file()
}

/// Execute `program` with `args`, replacing this process on unix (`exec`) so
/// stdio and the exit code are the child's by construction. On Windows, spawn,
/// wait, and exit with the child's status code.
pub fn exec_external(program: &Path, args: &[String]) -> Result<()> {
    let mut command = std::process::Command::new(program);
    command.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        Err(error).with_context(|| format!("Failed to execute {}", program.display()))
    }
    #[cfg(windows)]
    {
        let status = command.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }
}
