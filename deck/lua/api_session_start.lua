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
    "SELECT * FROM sessions WHERE id = %s",
    db.escape_literal(session_id)
)
local res, err = db.query(sql)
if not res or #res == 0 then
    ngx.status = 404
    ngx.say(cjson.encode({error = "Session not found"}))
    return
end

local session = res[1]

if session.status == "running" then
    ngx.status = 400
    ngx.say(cjson.encode({error = "Session already running", port = session.opencode_port}))
    return
end

local base_port = 4097
local port_sql = "SELECT COALESCE(MAX(opencode_port), 4096) + 1 as next_port FROM sessions WHERE opencode_port IS NOT NULL"
local port_res = db.query(port_sql)
local port = port_res and port_res[1] and port_res[1].next_port or base_port

local cmd = string.format(
    "cd %s && nohup opencode serve --port %d > /tmp/opencode_%s.log 2>&1 & echo $!",
    session.worktree_path,
    port,
    session_id
)

local handle = io.popen(cmd)
local pid = handle:read("*a"):gsub("%s+", "")
handle:close()

if not pid or pid == "" then
    ngx.status = 500
    ngx.say(cjson.encode({error = "Failed to start opencode serve"}))
    return
end

sql = string.format(
    "UPDATE sessions SET opencode_port = %d, opencode_pid = %s, status = 'running' WHERE id = %s RETURNING *",
    port,
    pid,
    db.escape_literal(session_id)
)
res, err = db.query(sql)
if not res then
    ngx.status = 500
    ngx.say(cjson.encode({error = err}))
    return
end

ngx.say(cjson.encode({
    session_id = session_id,
    status = "running",
    port = port,
    pid = tonumber(pid)
}))
