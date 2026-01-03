local db = require "db"
local cjson = require "cjson"

local method = ngx.req.get_method()
local session_id = ngx.var.session_id

if method == "GET" then
    local sql = string.format(
        "SELECT * FROM sessions WHERE id = %s",
        db.escape_literal(session_id)
    )

    local res, err = db.query(sql)
    if not res then
        ngx.status = 500
        ngx.say(cjson.encode({error = err}))
        return
    end

    if #res == 0 then
        ngx.status = 404
        ngx.say(cjson.encode({error = "Session not found"}))
        return
    end

    ngx.say(cjson.encode(res[1]))

elseif method == "DELETE" then
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
    local worktree_path = session.worktree_path

    local handle = io.popen(string.format(
        "cd %s && agt prune-session --session-id %s --delete-branch 2>&1",
        worktree_path:match("(.*/)[^/]+$") or ".",
        session_id
    ))
    local result = handle:read("*a")
    local success = handle:close()

    if not success then
        ngx.log(ngx.WARN, "agt prune-session warning: " .. result)
    end

    sql = string.format("DELETE FROM sessions WHERE id = %s", db.escape_literal(session_id))
    res, err = db.query(sql)
    if not res then
        ngx.status = 500
        ngx.say(cjson.encode({error = err}))
        return
    end

    ngx.say(cjson.encode({deleted = true, session_id = session_id}))

else
    ngx.status = 405
    ngx.say(cjson.encode({error = "Method not allowed"}))
end
