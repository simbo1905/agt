# OpenCode Deck Architecture

A local development environment that orchestrates OpenCode agents across multiple projects and workspaces, with streaming portal interfaces. This is to showcase the `apt` approach to continuously snapshotting the file system. 

## Core Vision

**Hypothesis**: Multiple isolated OpenCode sessions (one per git worktree) can be coordinated through a unified web UI, with streaming portal interfaces embedded in WebView controls.

**Design Goal**: Minimise complexity locally before scaling to remote (VPS) orchestration.

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ OpenCode Deck (Fork Of OpenCode Desktop App)                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Project Dropdown Menu                                    │   │
│  │  • foo (~/projects/foo)                                  │   │
│  │  • bar (~/projects/bar)                                  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ Workspace Tabs (per project)                             │   │
│  │  [main] [feature1] [feature2] [...] [+ New Workspace]    │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │              Main Content Area                           │   │
│  ├───────────────────┬──────────────────────────────────────┤   │
│  │                   │                                      │   │
│  │  Tasks Panel      │  Portal WebView/iframe.              │   │
│  │  (Left)           │                                      │   │
│  │                   │  Shows streaming OpenCode session    │   │
│  │  • Task 1         │  via portal interface                │   │
│  │  • Task 2         │                                      │   │
│  │  • ...            │                                      │   │
│  │                   │  WebSocket ↔ OpenCode serve          │   │
│  │                   │  (localhost:4096)                    │   │
│  │                   │                                      │   │
│  └───────────────────┴──────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                            ↓
                 OpenCode Session Manager
              (localhost:4096 serve instance)
                            ↓
┌─────────────────────────────────────────────────────────────────┐
│ Disk Layout (per project)                                       │
├─────────────────────────────────────────────────────────────────┤
│  /projects/foo/                                                 │
│  ├── .git/                                                      │
│  ├── .opencode/                   (per-project session store)   │
│  │   ├── sessions/                                              │
│  │   └── config.json                                            │
│  ├── [main branch files]                                        │
│  └── .gitignore                  (excludes .opencode/)          │
│                                                                 │
│  /projects/foo-feature1/         (git worktree)                 │
│  ├── .git → ~/projects/foo/.git                                 │
│  ├── .opencode/                   (isolated for this worktree)  │
│  │   ├── sessions/                                              │
│  │   └── config.json                                            │
│  └── [feature1 branch files]                                    │
│                                                                 │
│  /projects/foo-feature2/         (git worktree)                 │
│  └── [similar structure]                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Key Concepts

### 1. Project
- A git repository (e.g., `~/projects/foo`)
- Selected via dropdown menu
- Sessions scoped to `project/.opencode/` (via `OPENCODE_DIR` env var)

### 2. Workspace
- A git worktree (branch) within a project
- Examples: `foo/` (main), `foo-feature1/` (feature branch)
- One tab per workspace in the UI
- Independent OpenCode session per worktree

### 3. Session
- An OpenCode instance (PTY-bound, file-locked)
- Created in a specific worktree's PWD
- Persisted to `.opencode/sessions/` (project-local)
- Accessed via OpenCode REST/WebSocket API
- Session IDs passed to the portal iframe for rendering

### 4. Task (Left Panel)
- Represents a command, test, or work unit
- Associated with a specific session
- Can be pending, in_progress, completed, or abandoned
- Metadata: title, session_id, created_at, status

### 5. Portal Interface (Right Panel)
- Streaming UI embedded in WebView
- Connects to OpenCode serve on `localhost:4096`
- Shows session output in real-time (stdout/stderr over WebSocket)
- Maps to your fork of `simbo1905/portal`

## Control Flow

### Opening a Project

```
1. User selects "foo" from the project dropdown
   ↓
2. Verdant Deck loads project metadata:
   - Git repo path: ~/projects/foo
   - Worktrees: [main, feature1, feature2]
   - Sessions: [list from .opencode/sessions/]
   ↓
3. Populate workspace tabs: [main] [feature1] [feature2]
4. Load tasks for active workspace (e.g., "main")
5. Initialise or connect to the OpenCode session for the "main" worktree
6. Render portal iframe with session_id
```

### Creating a New Workspace

```
1. User clicks "+ New Workspace" tab
2. Prompt for branch name (e.g., "feature3")
   ↓
3. Execute:
   cd ~/projects/foo
   git worktree add ../foo-feature3 feature3
   mkdir -p foo-feature3/.opencode
   ↓
4. Create new tab: [feature3]
5. Create a new OpenCode session in ~/projects/foo-feature3
6. Render portal iframe with new session_id
```

### Abandoning a Workspace

```
1. User closes workspace tab (e.g., feature2)
2. Verdant Deck prompts: "Delete worktree foo-feature2?"
   ↓
3. If confirmed:
   - Kill OpenCode session (if running)
   - Delete session files (.opencode/)
   - git worktree remove ../foo-feature2
   - git branch -D feature2
   ↓
4. Remove tab from UI
```

### Running a Task

```
1. User selects task from left panel (e.g., "Run tests")
2. Verdant Deck:
   - Maps task → current session_id
   - Sends task command to OpenCode via REST API (POST /tui)
   - Updates task status: pending → in_progress
   ↓
3. Portal iframe streams OpenCode output over WebSocket
4. On completion, task status → completed
5. Optionally log output to task history
```

## Technology Stack

- **UI Framework**: Flet 0.80.0 (Python)
- **Backend Orchestration**: Python (uv project)
- **Streaming Interface**: Portal (Node/TypeScript, embedded in WebView iframe)
- **Session Management**: OpenCode serve (localhost:4096)
- **Session Storage**: Per-project `.opencode/` directories
- **IPC**: REST (session management) + WebSocket (streaming)

## Data Models

### ProjectConfig
```json
{
  "name": "foo",
  "repo_path": "~/projects/foo",
  "remote": "https://github.com/user/foo.git",
  "main_branch": "main",
  "worktrees": ["main", "feature1", "feature2"],
  "sessions": {
    "main": { "id": "session_abc", "pwd": "~/projects/foo", "status": "active" },
    "feature1": { "id": "session_def", "pwd": "~/projects/foo-feature1", "status": "idle" }
  }
}
```

### Task
```json
{
  "id": "task_1",
  "title": "Run tests",
  "session_id": "session_abc",
  "status": "in_progress",
  "command": "pytest",
  "created_at": "2025-01-02T11:00:00Z",
  "output": []
}
```

### SessionMetadata (from OpenCode)
```json
{
  "id": "session_abc",
  "pwd": "~/projects/foo",
  "branch": "main",
  "model": "claude-3-5-sonnet",
  "created_at": "2025-01-02T10:00:00Z",
  "message_count": 42,
  "token_usage": { "input": 1000, "output": 500 }
}
```

## MVP Scope (Phase 1)

### Must Have
1. ✅ Project dropdown (hardcoded: foo, bar)
2. ✅ Workspace tabs (list from `.opencode/sessions/`)
3. ✅ Task list (left panel, static for MVP)
4. ✅ Portal WebView iframe (right panel, points to `localhost:3000`)
5. ✅ Basic session lifecycle (create, list, delete via OpenCode API)
6. ✅ OPENCODE_DIR environment variable handling

### Nice to Have (Phase 2)
- New workspace creation UI
- Task creation/execution
- WebSocket streaming from portal
- Session persistence/recovery
- Git worktree automation

### Out of Scope (Phase 3+)
- Remote VPS deployment
- Multi-user orchestration
- Advanced portal features (rich markdown, etc.)

## Testing Strategy

### Test Case: Parallel Subtests (from @idea.chat)

```
Test: Fork 2 parallel OpenCode sessions in different worktrees
1. User has project "foo" with main branch checked out
2. User creates workspace "feature-opus" (worktree foo-feature-opus)
3. User creates workspace "feature-codex" (worktree foo-feature-codex)
4. Both workspaces show in tabs: [main] [feature-opus] [feature-codex]
5. Click [feature-opus] tab:
   - Load opus-specific tasks
   - Connect to opus session via portal iframe
6. Click [feature-codex] tab:
   - Load codex-specific tasks
   - Connect to codex session via portal iframe
7. Both sessions run in parallel (independent processes, isolated git states)
8. User monitors both in portal, picks winner (e.g., codex finishes first)
9. User closes feature-opus tab → deletes worktree, cleans up session
10. User continues with feature-codex (or merges back to main)

✓ No disk conflicts
✓ Independent session state
✓ Clean abandonment/merge
✓ Portal shows both sessions side-by-side (via tab switching)
```

## File Layout

```
/Users/Shared/mistral-deck/
├── ARCHITECTURE.md                    (this file)
├── AGENTS.md
├── README.md
├── main.py                            (Flet app entry)
├── pyproject.toml
│
├── screens/
│   ├── provider_screen.py             (old: Provider Settings UI)
│   ├── chat_screen.py                 (old: Chat UI)
│   ├── verdant_deck.py                (NEW: Main Verdant Deck UI)
│   ├── project_selector.py            (NEW: Project dropdown)
│   ├── workspace_tabs.py              (NEW: Workspace tabs)
│   ├── task_panel.py                  (NEW: Task list left)
│   └── portal_viewer.py               (NEW: WebView iframe right)
│
├── services/
│   ├── mistral_api.py                 (old)
│   ├── provider_manager.py            (old)
│   ├── opencode_manager.py            (NEW: OpenCode orchestration)
│   ├── project_manager.py             (NEW: Project/workspace management)
│   ├── git_worktree_manager.py        (NEW: Git worktree operations)
│   └── task_manager.py                (NEW: Task lifecycle)
│
├── tests/
│   ├── test_opencode_manager.py       (NEW)
│   ├── test_project_manager.py        (NEW)
│   ├── test_git_worktree_manager.py   (NEW)
│   └── test_task_manager.py           (NEW)
│
└── data/
    └── projects.json                  (NEW: Project registry)
```

## OpenCode Serve Integration

### Assumptions
- `opencode serve` is running on `localhost:4096`
- Sessions are persisted to `project/.opencode/sessions/`
- REST API available for session management:
  - `POST /pty?directory=/path/to/worktree` → create session
  - `GET /sessions` → list sessions
  - `GET /sessions/{id}` → get session metadata
  - `DELETE /sessions/{id}` → delete session
  - `ws://localhost:4096/pty/{id}/ws` → WebSocket for streaming

### Portal Integration
- Portal (Node.js) runs on `localhost:3000`
- Connects to OpenCode WebSocket (`localhost:4096`)
- Rendered in Flet WebView:
  ```python
  ft.WebView(
      url="http://localhost:3000?session_id=session_abc",
      expand=True
  )
  ```

## Session Lifecycle (Detailed)

```
Create:
  1. Worktree created (git worktree add)
  2. OPENCODE_DIR set to worktree/.opencode
  3. REST call: POST /pty?directory=/path/to/worktree
  4. Returns: {"id": "session_xyz", "ws_url": "/pty/session_xyz/ws"}
  5. Store session_id in project metadata

Active:
  6. Portal iframe connects: WebSocket ws://localhost:4096/pty/session_xyz/ws
  7. User interacts; output streams back over WebSocket
  8. Task commands sent via REST: POST /tui (batched)

Idle:
  9. User switches workspace (tab)
  10. Session persists on disk (session files locked by OpenCode)

Resume:
  11. User clicks same tab again
  12. Verdant Deck checks: Is session still active?
  13. If yes: reconnect WebSocket, resume output streaming
  14. If no: REST call to reattach, verify PWD matches

Delete:
  15. User closes worktree (x on tab)
  16. REST call: DELETE /sessions/session_xyz
  17. Delete .opencode/ directory
  18. git worktree remove
  19. Remove tab from UI
```

## Error Handling

### Scenarios
1. OpenCode serve not running → Show error banner, option to start
2. Worktree deletion fails → Warn user, offer manual cleanup
3. Session dies unexpectedly → Auto-reconnect or prompt resume
4. Portal iframe fails to load → Show fallback (raw WebSocket output)

## Future: Remote Orchestration (Phase 3)

When scaling to VPS:
- Replace local `opencode serve` with remote SSH tunnel
- Replace local git worktrees with remote git operations
- Replace `OPENCODE_DIR` with remote session manager (e.g., tmux over SSH)
- Portal remains embedded; backend now remote

```
Verdant Deck (local Flet) 
  → SSH tunnel → VPS OpenCode serve
  → VPS git worktrees (~/projects/foo/)
  → Portal streams over secure WebSocket
```

## References

- OpenCode.ai API: localhost:4096/doc
- Portal (streaming UI): https://github.com/simbo1905/portal
- Git worktrees: `git worktree help`
- Flet 0.80.0: https://flet.dev

---

**Status**: MVP Architecture (Phase 1)  
**Last Updated**: 2025-01-02  
**Author**: Verdant Deck Team
