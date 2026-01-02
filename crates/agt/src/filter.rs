use crate::config::AgtConfig;
use gix::Commit;

/// Check if a reference should be hidden from user view (agent branches)
#[allow(dead_code)]
pub fn should_hide_ref(ref_name: &str, config: &AgtConfig) -> bool {
    ref_name.contains(&config.branch_prefix)
}

/// Check if a commit should be hidden from user view (agent commits)
#[allow(dead_code)]
pub fn should_hide_commit(commit: &Commit, config: &AgtConfig) -> bool {
    commit
        .author()
        .map(|a| *a.email == config.agent_email)
        .unwrap_or(false)
}
