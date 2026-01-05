use crate::config::AgtConfig;
use anyhow::Result;
use gix::Repository;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize)]
struct SessionMetadata {
    session_id: String,
    branch: String,
    sandbox: String,
}

pub fn run(repo: &Repository, _config: &AgtConfig) -> Result<()> {
    let sessions_dir = repo.common_dir().join("agt/sessions");

    if !sessions_dir.exists() {
        println!("No agent sessions found");
        return Ok(());
    }

    let mut sessions = Vec::new();

    for entry in fs::read_dir(&sessions_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let raw = fs::read_to_string(entry.path())?;
        let meta: SessionMetadata = match serde_json::from_str(&raw) {
            Ok(meta) => meta,
            Err(_) => continue,
        };

        sessions.push(meta);
    }

    if sessions.is_empty() {
        println!("No agent sessions found");
        return Ok(());
    }

    println!("Agent Sessions:");
    for meta in sessions {
        println!("  {}:", meta.session_id);
        println!("    Branch: {}", meta.branch);
        println!("    Sandbox: {}", meta.sandbox);
    }

    Ok(())
}
