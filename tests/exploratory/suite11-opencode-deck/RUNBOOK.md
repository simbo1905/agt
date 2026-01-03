# Suite 11: OpenCode Deck - Manual Validation Runbook

## Objective

Validate the OpenCode Deck SPA served by OpenResty, demonstrating agt's ability to manage agent sessions with full filesystem autocommit capabilities.

## Working Directory

```
.tmp/suite11
```

## Prerequisites

- macOS with Homebrew
- OpenResty installed: `brew install openresty/brew/openresty`
- PostgreSQL 14 installed: `brew install postgresql@14`
- agt binary built: `make build`
- opencode binary available in PATH (for session proxying)

## Reference

- `DECK.md` - Architecture documentation
- `docs/agt.1.txt` - agt CLI reference
- `deck/` - Implementation files

## Setup

```bash
# 1. Start PostgreSQL
brew services start postgresql@14

# 2. Create database and apply schema
/opt/homebrew/opt/postgresql@14/bin/createdb agt_deck 2>/dev/null || true
/opt/homebrew/opt/postgresql@14/bin/psql agt_deck < deck/sql/schema.sql

# 3. Install pgmoon (if not already)
/opt/homebrew/opt/openresty/bin/opm get leafo/pgmoon

# 4. Build agt
make build

# 5. Start OpenResty
cd /Users/Shared/agt
/opt/homebrew/opt/openresty/bin/openresty -p $(pwd)/deck -c conf/nginx.conf

# 6. Open browser
open http://localhost:8080
```

## Scenarios

### Scenario 1: Verify OpenResty Serves SPA

**Steps:**
1. Navigate to http://localhost:8080
2. Verify the React SPA loads
3. Check browser console for JavaScript errors

**Expected:**
- Page title: "OpenCode Deck - Agent Session Manager"
- Dark theme UI with header, sidebar, and main content area
- No JavaScript console errors

**Pass/Fail:** [ ]

---

### Scenario 2: Register a Project

**Steps:**
1. Create a test agt repository:
   ```bash
   mkdir -p .tmp/suite11
   cd .tmp/suite11
   agt init https://github.com/octocat/Hello-World.git
   cd Hello-World
   ```
2. In the SPA, click "+ Add" next to Projects
3. Fill in:
   - Project ID: `test-project`
   - Name: `Hello World Test`
   - Bare Repo Path: `/Users/Shared/agt/.tmp/suite11/Hello-World.git`
   - Main Worktree Path: `/Users/Shared/agt/.tmp/suite11/Hello-World`
4. Click "Register"

**Expected:**
- Project appears in the dropdown
- No error messages
- Database contains the project:
  ```bash
  /opt/homebrew/opt/postgresql@14/bin/psql agt_deck -c "SELECT * FROM projects"
  ```

**Pass/Fail:** [ ]

---

### Scenario 3: Fork a Session

**Steps:**
1. Select "Hello World Test" from the project dropdown
2. Click "+ Fork" next to Sessions
3. Fill in:
   - Session ID: `agent-001`
   - From: `HEAD`
4. Click "Fork Session"

**Expected:**
- Session card appears with status "idle"
- Worktree exists:
  ```bash
  ls -la .tmp/suite11/Hello-World/sessions/agent-001/
  ```
- Session in database:
  ```bash
  /opt/homebrew/opt/postgresql@14/bin/psql agt_deck -c "SELECT * FROM sessions"
  ```
- agt shows the session:
  ```bash
  cd .tmp/suite11/Hello-World && agt list-sessions
  ```

**Pass/Fail:** [ ]

---

### Scenario 4: Create Snapshot (Autocommit)

**Steps:**
1. Make a change in the session worktree:
   ```bash
   echo "Test change $(date)" >> .tmp/suite11/Hello-World/sessions/agent-001/README
   ```
2. Click "Snapshot" button on the agent-001 session card

**Expected:**
- Snapshot recorded in database:
  ```bash
  /opt/homebrew/opt/postgresql@14/bin/psql agt_deck -c "SELECT * FROM snapshots"
  ```
- Snapshot visible in session detail (if session is selected)
- Git history shows the autocommit:
  ```bash
  cd .tmp/suite11/Hello-World && agt log --oneline agtsessions/agent-001
  ```

**Pass/Fail:** [ ]

---

### Scenario 5: Start OpenCode Session

**Prerequisites:** opencode binary must be installed and in PATH

**Steps:**
1. Click "Start" button on agent-001 session card

**Expected:**
- Status changes to "running" with green badge
- Port number displayed (e.g., 4097)
- Session detail shows iframe (if opencode is running)
- Process is running:
  ```bash
  ps aux | grep opencode
  ```

**Pass/Fail:** [ ]

---

### Scenario 6: Stop OpenCode Session

**Steps:**
1. Click "Stop" button on the running agent-001 session

**Expected:**
- Status changes to "stopped" with red badge
- Port number cleared
- Process is terminated:
  ```bash
  ps aux | grep opencode
  ```

**Pass/Fail:** [ ]

---

### Scenario 7: Delete Session

**Steps:**
1. Click "Delete" button on agent-001 session card
2. Confirm the deletion

**Expected:**
- Session card disappears
- Worktree removed:
  ```bash
  ls .tmp/suite11/Hello-World/sessions/
  ```
- Session removed from database:
  ```bash
  /opt/homebrew/opt/postgresql@14/bin/psql agt_deck -c "SELECT * FROM sessions"
  ```

**Pass/Fail:** [ ]

---

### Scenario 8: Multiple Parallel Sessions

**Steps:**
1. Fork session `agent-claude` from HEAD
2. Fork session `agent-opus` from HEAD
3. Make different changes in each worktree:
   ```bash
   echo "Claude change" >> .tmp/suite11/Hello-World/sessions/agent-claude/README
   echo "Opus change" >> .tmp/suite11/Hello-World/sessions/agent-opus/README
   ```
4. Snapshot both sessions
5. Start both sessions (if opencode available)

**Expected:**
- Both sessions visible in sidebar
- Separate worktrees exist
- Separate branches in git:
  ```bash
  cd .tmp/suite11/Hello-World && agt branch
  ```
- Each has independent snapshots

**Pass/Fail:** [ ]

---

### Scenario 9: API Direct Testing

**Steps:**
```bash
# List projects
curl -s http://localhost:8080/api/projects | jq

# List sessions
curl -s http://localhost:8080/api/sessions | jq

# Fork a session
curl -s -X POST http://localhost:8080/api/sessions/fork \
  -H "Content-Type: application/json" \
  -d '{"project_id":"test-project","session_id":"api-test","from_ref":"HEAD"}' | jq

# Trigger autocommit
curl -s -X POST http://localhost:8080/api/sessions/api-test/autocommit | jq

# Get session details
curl -s http://localhost:8080/api/sessions/api-test | jq

# Delete session
curl -s -X DELETE http://localhost:8080/api/sessions/api-test | jq
```

**Expected:**
- All API calls return valid JSON
- Operations reflect in database and filesystem

**Pass/Fail:** [ ]

---

### Scenario 10: Error Handling

**Steps:**
1. Try to fork a session with duplicate ID
2. Try to start a non-existent session
3. Try to autocommit on a session with no changes

**Expected:**
- Appropriate error messages displayed
- No server crashes
- Logs show errors:
  ```bash
  tail -f deck/logs/error.log
  ```

**Pass/Fail:** [ ]

---

## Cleanup

```bash
# Stop OpenResty
/opt/homebrew/opt/openresty/bin/openresty -p $(pwd)/deck -s stop

# Clean database
/opt/homebrew/opt/postgresql@14/bin/psql agt_deck -c "DELETE FROM projects"

# Remove test files
rm -rf .tmp/suite11
```

## Success Criteria

- All 10 scenarios pass
- No JavaScript errors in browser console
- No Lua errors in deck/logs/error.log
- Database state consistent with UI
- Filesystem state consistent with agt metadata

## Failure Modes

| Category | Symptoms | Likely Cause |
|----------|----------|--------------|
| Database | "Failed to connect" | PostgreSQL not running |
| Lua | 500 errors | Syntax error in Lua files |
| SPA | Blank page | JavaScript error, check console |
| agt | Fork fails | agt binary not in PATH |
| Proxy | 502 errors | opencode not running/installed |

---

**Last Updated:** 2026-01-03
**Author:** OpenCode Deck Team
