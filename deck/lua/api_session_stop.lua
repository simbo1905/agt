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

if session.status ~= "running" then
    ngx.status = 400
    ngx.say(cjson.encode({error = "Session not running"}))
    return
end

if session.opencode_pid then
    os.execute(string.format("kill %d 2>/dev/null", session.opencode_pid))
end

sql = string.format(
    "UPDATE sessions SET opencode_port = NULL, opencode_pid = NULL, status = 'stopped' WHERE id = %s RETURNING *",
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
    status = "stopped"
}))
