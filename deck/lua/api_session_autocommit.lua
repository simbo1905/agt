local db = require "db"
local cjson = require "cjson"

local method = ngx.req.get_method()
local session_id = ngx.var.session_id

if method ~= "POST" then
    ngx.status = 405
    ngx.say(cjson.encode({error = "Method not allowed"}))
    return
end

local sql = string.format(
    "SELECT s.*, p.main_worktree_path FROM sessions s JOIN projects p ON s.project_id = p.id WHERE s.id = %s",
    db.escape_literal(session_id)
)
local res, err = db.query(sql)
if not res or #res == 0 then
    ngx.status = 404
    ngx.say(cjson.encode({error = "Session not found"}))
    return
end

local session = res[1]

local cmd = string.format(
    "cd %s && agt autocommit -C %s --session-id %s 2>&1",
    session.main_worktree_path,
    session.worktree_path,
    session_id
)

local handle = io.popen(cmd)
local result = handle:read("*a")
local success = handle:close()

local commit_sha = result:match("commit ([a-f0-9]+)")
local files_changed = result:match("(%d+) files? changed") or result:match("changed (%d+)")

if commit_sha then
    local snapshot_sql = string.format(
        "INSERT INTO snapshots (session_id, commit_sha, files_changed) VALUES (%s, %s, %s) RETURNING *",
        db.escape_literal(session_id),
        db.escape_literal(commit_sha),
        files_changed and tonumber(files_changed) or "NULL"
    )
    db.query(snapshot_sql)
end

ngx.say(cjson.encode({
    session_id = session_id,
    success = success ~= nil,
    output = result,
    commit_sha = commit_sha,
    files_changed = files_changed and tonumber(files_changed) or nil
}))
