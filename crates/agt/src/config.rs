use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct AgtConfig {
    pub git_path: PathBuf,
    pub agent_email: String,
    pub branch_prefix: String,
    pub user_email: Option<String>,
}

impl Default for AgtConfig {
    fn default() -> Self {
        Self {
            git_path: PathBuf::from("/usr/bin/git"),
            agent_email: "agt@local".to_string(),
            branch_prefix: "agtsessions/".to_string(),
            user_email: None,
        }
    }
}

impl AgtConfig {
    pub fn load() -> Result<Self> {
        let mut config = Self::default();

        // Read global config first: ~/.agtconfig
        if let Some(home) = dirs::home_dir() {
            let global_path = home.join(".agtconfig");
            if global_path.exists() {
                let global_settings = parse_ini_file(&global_path)?;
                apply_settings(&mut config, &global_settings);
            }
        }

        // Then read local config: .agt/config (overrides global)
        if let Some(repo_root) = find_repo_root() {
            let local_path = repo_root.join(".agt").join("config");
            if local_path.exists() {
                let local_settings = parse_ini_file(&local_path)?;
                apply_settings(&mut config, &local_settings);
            }
        }

        // Environment variable overrides everything
        if let Ok(path) = std::env::var("AGT_GIT_PATH") {
            config.git_path = PathBuf::from(path);
        }

        Ok(config)
    }

    pub fn load_for_init() -> Self {
        let mut config = Self::default();

        // For init, only read global config (repo doesn't exist yet)
        if let Some(home) = dirs::home_dir() {
            let global_path = home.join(".agtconfig");
            if global_path.exists() {
                if let Ok(global_settings) = parse_ini_file(&global_path) {
                    apply_settings(&mut config, &global_settings);
                }
            }
        }

        // Environment variable override
        if let Ok(path) = std::env::var("AGT_GIT_PATH") {
            config.git_path = PathBuf::from(path);
        }

        config
    }
}

fn find_repo_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if current.join(".git").exists() || current.join(".agt").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn parse_ini_file(path: &PathBuf) -> Result<HashMap<String, String>> {
    let content =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    parse_ini(&content)
}

fn parse_ini(content: &str) -> Result<HashMap<String, String>> {
    let mut settings = HashMap::new();
    let mut current_section = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        // Section header: [section]
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].to_string();
            continue;
        }

        // Key = value
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim();
            let value = trimmed[eq_pos + 1..].trim();
            let full_key = if current_section.is_empty() {
                key.to_string()
            } else {
                format!("{}.{}", current_section, key)
            };
            settings.insert(full_key, value.to_string());
        }
    }

    Ok(settings)
}

fn apply_settings(config: &mut AgtConfig, settings: &HashMap<String, String>) {
    if let Some(v) = settings.get("agt.gitPath") {
        config.git_path = PathBuf::from(v);
    }
    if let Some(v) = settings.get("agt.agentEmail") {
        config.agent_email.clone_from(v);
    }
    if let Some(v) = settings.get("agt.branchPrefix") {
        config.branch_prefix.clone_from(v);
    }
    if let Some(v) = settings.get("agt.userEmail") {
        config.user_email = Some(v.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ini_basic() {
        let content = r#"
[agt]
    gitPath = /opt/git/bin/git
    agentEmail = agt.opencode@local
    branchPrefix = agtsessions/
"#;
        let settings = parse_ini(content).unwrap();
        assert_eq!(settings.get("agt.gitPath"), Some(&"/opt/git/bin/git".to_string()));
        assert_eq!(settings.get("agt.agentEmail"), Some(&"agt.opencode@local".to_string()));
        assert_eq!(settings.get("agt.branchPrefix"), Some(&"agtsessions/".to_string()));
    }

    #[test]
    fn test_parse_ini_with_comments() {
        let content = r#"
# This is a comment
[agt]
    ; Another comment
    gitPath = /usr/bin/git
"#;
        let settings = parse_ini(content).unwrap();
        assert_eq!(settings.get("agt.gitPath"), Some(&"/usr/bin/git".to_string()));
    }

    #[test]
    fn test_apply_settings() {
        let mut config = AgtConfig::default();
        let mut settings = HashMap::new();
        settings.insert("agt.gitPath".to_string(), "/custom/git".to_string());
        settings.insert("agt.agentEmail".to_string(), "custom@test".to_string());

        apply_settings(&mut config, &settings);

        assert_eq!(config.git_path, PathBuf::from("/custom/git"));
        assert_eq!(config.agent_email, "custom@test");
    }
}
