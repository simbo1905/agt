use anyhow::Result;
use gix::Repository;

pub struct AgtConfig {
    pub agent_email: String,
    pub branch_prefix: String,
    pub user_email: Option<String>,
}

impl Default for AgtConfig {
    fn default() -> Self {
        Self {
            agent_email: "agt@local".to_string(),
            branch_prefix: "agtsessions/".to_string(),
            user_email: None,
        }
    }
}

impl AgtConfig {
    #[allow(clippy::unnecessary_wraps)]
    pub fn load(repo: &Repository) -> Result<Self> {
        let config = repo.config_snapshot();

        Ok(Self {
            agent_email: config
                .string("agt.agentEmail")
                .map_or_else(|| "agt@local".to_string(), |s| s.to_string()),
            branch_prefix: config
                .string("agt.branchPrefix")
                .map_or_else(|| "agtsessions/".to_string(), |s| s.to_string()),
            user_email: config.string("agt.userEmail").map(|s| s.to_string()),
        })
    }
}
