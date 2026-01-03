# OpenCode Deck Architecture

A local development environment that orchestrates OpenCode agents across multiple projects and workspaces, with streaming portal interfaces. This showcases the `agt` approach to continuously snapshotting the file system.

## Core Vision

**Hypothesis**: Multiple isolated OpenCode sessions (one per git worktree) can be coordinated through a unified web UI served by OpenResty, with real-time state from PostgreSQL.

**Design Goal**: Minimise complexity with a thin Lua controller that manages sessions and proxies `opencode serve` traffic.

## Technology Stack

- **Web Server**: OpenResty (nginx + LuaJIT)
- **Database**: PostgreSQL 14 with pgmoon driver
- **Frontend**: No-build-step React SPA (Babel + Tailwind CDN)
- **Session Backend**: OpenCode serve (proxied through OpenResty)
- **Agent Management**: `agt` CLI for worktree/session lifecycle

## Implementation

```
deck/
├── conf/
│   └── nginx.conf          # OpenResty configuration
├── lua/
│   ├── api.lua             # REST API handlers
│   ├── db.lua              # PostgreSQL connection helpers
│   └── proxy.lua           # OpenCode serve proxy logic
├── html/
│   └── index.html          # React SPA (CAT4-style, no build)
└── sql/
    └── schema.sql          # PostgreSQL schema
```

## Quick Start

```bash
# Prerequisites
brew install openresty/brew/openresty
brew install postgresql@14
brew services start postgresql@14

# Install pgmoon
/opt/homebrew/opt/openresty/bin/opm get leafo/pgmoon

# Create database and schema
/opt/homebrew/opt/postgresql@14/bin/createdb agt_deck
/opt/homebrew/opt/postgresql@14/bin/psql agt_deck < deck/sql/schema.sql

# Start OpenResty
/opt/homebrew/opt/openresty/bin/openresty -p $(pwd)/deck -c conf/nginx.conf

# Open browser
open http://localhost:8080
```

### Exploratory Tests

See `tests/exploratory/suite11-opencode-deck/RUNBOOK.md` for manual validation steps.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ Browser (React SPA)                                             │
│   http://localhost:8080                                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Project Selector                                         │   │
│  │  • project-foo (~/projects/foo)                          │   │
│  │  • project-bar (~/projects/bar)                          │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Session Tabs (agt sessions per project)                  │   │
│  │  [main] [agent-001] [agent-002] [+ Fork Session]         │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Main Content Area                           │   │
│  ├───────────────────┬──────────────────────────────────────┤   │
│  │                   │                                      │   │
│  │  Session Info     │  OpenCode Portal (iframe)            │   │
│  │  (Left)           │                                      │   │
│  │                   │  Streams from opencode serve         │   │
│  │  • Branch         │  via WebSocket proxy                 │   │
│  │  • Worktree       │                                      │   │
│  │  • Last Commit    │  ws://localhost:8080/ws/session_id   │   │
│  │  • Snapshots      │                                      │   │
│  │                   │                                      │   │
│  └───────────────────┴──────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ OpenResty (localhost:8080)                                      │
├─────────────────────────────────────────────────────────────────┤
│  GET  /                    → deck/html/index.html               │
│  GET  /api/projects        → Lua: list projects from DB         │
│  GET  /api/sessions        → Lua: list agt sessions from DB     │
│  POST /api/sessions/fork   → Lua: agt fork + record in DB       │
│  POST /api/sessions/:id/autocommit → Lua: agt autocommit        │
│  DELETE /api/sessions/:id  → Lua: agt prune-session             │
│  ANY  /opencode/*          → proxy_pass to opencode serve       │
│  WS   /ws/:session_id      → WebSocket proxy to opencode        │
└─────────────────────────────────────────────────────────────────┘
                            │
            ┌───────────────┴───────────────┐
            ▼                               ▼
┌───────────────────────┐     ┌───────────────────────────────────┐
│ PostgreSQL            │     │ opencode serve (per session)      │
│ (agt_deck database)   │     │ localhost:4096+N                  │
├───────────────────────┤     ├───────────────────────────────────┤
│ Tables:               │     │ • One instance per active session │
│ • projects            │     │ • Runs in agt worktree directory  │
│ • sessions            │     │ • Exposes REST + WebSocket API    │
│ • snapshots           │     │ • Managed by deck coordinator     │
└───────────────────────┘     └───────────────────────────────────┘
                                            │
                                            ▼
┌─────────────────────────────────────────────────────────────────┐
│ Disk Layout (agt-managed repository)                            │
├─────────────────────────────────────────────────────────────────┤
│  project.git/                    # Bare repository              │
│  ├── agt/                                                       │
│  │   ├── sessions/               # Session metadata JSON        │
│  │   └── timestamps/             # Autocommit timestamps        │
│  └── worktrees/                  # Git worktree admin dirs      │
│                                                                 │
│  project/                        # Main worktree                │
│  └── sessions/                                                  │
│      ├── agent-001/              # Session worktree             │
│      │   ├── .git                # Points to bare repo          │
│      │   └── [working files]                                    │
│      └── agent-002/              # Another session worktree     │
└─────────────────────────────────────────────────────────────────┘
```

## Database Schema

```sql
-- Projects registered with the deck
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    bare_repo_path TEXT NOT NULL,
    main_worktree_path TEXT NOT NULL,
    remote_url TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Agent sessions (synced from agt metadata)
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    branch TEXT NOT NULL,
    worktree_path TEXT NOT NULL,
    from_commit TEXT,
    user_branch TEXT,
    opencode_port INTEGER,
    opencode_pid INTEGER,
    status TEXT DEFAULT 'idle',  -- idle, running, stopped
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Snapshots (autocommit records)
CREATE TABLE snapshots (
    id SERIAL PRIMARY KEY,
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    commit_sha TEXT NOT NULL,
    files_changed INTEGER,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

## API Endpoints

### Projects

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/projects` | List all registered projects |
| POST | `/api/projects` | Register a new project |
| GET | `/api/projects/:id` | Get project details |
| DELETE | `/api/projects/:id` | Unregister project |

### Sessions

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/sessions?project_id=X` | List sessions for project |
| POST | `/api/sessions/fork` | Fork new session (`agt fork`) |
| GET | `/api/sessions/:id` | Get session details |
| POST | `/api/sessions/:id/start` | Start opencode serve |
| POST | `/api/sessions/:id/stop` | Stop opencode serve |
| POST | `/api/sessions/:id/autocommit` | Trigger autocommit |
| DELETE | `/api/sessions/:id` | Prune session (`agt prune-session`) |

### Snapshots

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/sessions/:id/snapshots` | List snapshots for session |

### OpenCode Proxy

| Method | Endpoint | Description |
|--------|----------|-------------|
| ANY | `/opencode/:session_id/*` | Proxy to session's opencode serve |
| WS | `/ws/:session_id` | WebSocket proxy for streaming |

## Session Lifecycle

```
1. Register Project
   POST /api/projects
   { "name": "foo", "bare_repo_path": "/path/to/foo.git" }
   
2. Fork Session
   POST /api/sessions/fork
   { "project_id": "proj-001", "session_id": "agent-claude", "from": "main" }
   → Runs: agt fork --session-id agent-claude --from main
   → Creates worktree at sessions/agent-claude/
   → Records in PostgreSQL

3. Start OpenCode
   POST /api/sessions/agent-claude/start
   → Spawns: opencode serve --port 4097 in worktree directory
   → Updates session.opencode_port, opencode_pid, status='running'

4. Work via UI
   - SPA loads session info from /api/sessions/agent-claude
   - iframe points to /opencode/agent-claude/ (proxied)
   - WebSocket connects to /ws/agent-claude

5. Autocommit
   POST /api/sessions/agent-claude/autocommit
   → Runs: agt autocommit -C sessions/agent-claude --session-id agent-claude
   → Records snapshot in PostgreSQL

6. Prune Session
   DELETE /api/sessions/agent-claude
   → Runs: agt prune-session --session-id agent-claude --delete-branch
   → Removes from PostgreSQL
```

## Configuration

OpenResty runs with a minimal nginx.conf:

```nginx
worker_processes 1;
error_log logs/error.log info;

events {
    worker_connections 1024;
}

http {
    lua_package_path "$prefix/lua/?.lua;;";
    
    init_by_lua_block {
        require "resty.core"
    }

    server {
        listen 8080;
        
        location / {
            root html;
            index index.html;
        }
        
        location /api {
            content_by_lua_file lua/api.lua;
        }
        
        location ~ ^/opencode/([^/]+)/(.*)$ {
            content_by_lua_file lua/proxy.lua;
        }
        
        location ~ ^/ws/([^/]+)$ {
            content_by_lua_file lua/proxy.lua;
        }
    }
}
```

## References

- agt CLI: `docs/agt.1.txt`
- OpenResty: https://openresty.org
- pgmoon: https://github.com/leafo/pgmoon
- OpenCode serve: localhost:4096/doc

---

**Status**: Implementation Phase  
**Last Updated**: 2026-01-03
