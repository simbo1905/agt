-- OpenCode Deck Schema
-- PostgreSQL schema for agt session management

-- Projects registered with the deck
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    bare_repo_path TEXT NOT NULL,
    main_worktree_path TEXT NOT NULL,
    remote_url TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Agent sessions (synced from agt metadata)
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    branch TEXT NOT NULL,
    worktree_path TEXT NOT NULL,
    from_commit TEXT,
    user_branch TEXT,
    opencode_port INTEGER,
    opencode_pid INTEGER,
    status TEXT DEFAULT 'idle' CHECK (status IN ('idle', 'running', 'stopped')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Snapshots (autocommit records)
CREATE TABLE IF NOT EXISTS snapshots (
    id SERIAL PRIMARY KEY,
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    commit_sha TEXT NOT NULL,
    files_changed INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_sessions_project_id ON sessions(project_id);
CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
CREATE INDEX IF NOT EXISTS idx_snapshots_session_id ON snapshots(session_id);
CREATE INDEX IF NOT EXISTS idx_snapshots_created_at ON snapshots(created_at DESC);

-- Trigger to update updated_at on sessions
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

DROP TRIGGER IF EXISTS update_sessions_updated_at ON sessions;
CREATE TRIGGER update_sessions_updated_at
    BEFORE UPDATE ON sessions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
