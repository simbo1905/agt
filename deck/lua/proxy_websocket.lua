local db = require "db"
local cjson = require "cjson"

local session_id = ngx.var.session_id

local sql = string.format(
    "SELECT * FROM sessions WHERE id = %s AND status = 'running'",
    db.escape_literal(session_id)
)
local res, err = db.query(sql)
if not res or #res == 0 then
    ngx.status = 404
    ngx.say(cjson.encode({error = "Session not found or not running"}))
    return
end

local session = res[1]
local port = session.opencode_port

ngx.say(cjson.encode({error = "WebSocket proxy not yet implemented", port = port}))
