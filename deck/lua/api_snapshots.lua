local db = require "db"
local cjson = require "cjson"

local method = ngx.req.get_method()
local session_id = ngx.var.session_id

if method ~= "GET" then
    ngx.status = 405
    ngx.say(cjson.encode({error = "Method not allowed"}))
    return
end

local sql = string.format(
    "SELECT * FROM snapshots WHERE session_id = %s ORDER BY created_at DESC LIMIT 100",
    db.escape_literal(session_id)
)

local res, err = db.query(sql)
if not res then
    ngx.status = 500
    ngx.say(cjson.encode({error = err}))
    return
end

ngx.say(cjson.encode(res))
