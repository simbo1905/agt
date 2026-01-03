local db = require "db"
local cjson = require "cjson"
cjson.encode_empty_table_as_object(false)

local method = ngx.req.get_method()

if method == "GET" then
    local res, err = db.query("SELECT * FROM projects ORDER BY created_at DESC")
    if not res then
        ngx.status = 500
        ngx.say(cjson.encode({error = err}))
        return
    end
    ngx.say(cjson.encode(res))

elseif method == "POST" then
    ngx.req.read_body()
    local body = ngx.req.get_body_data()
    if not body then
        ngx.status = 400
        ngx.say(cjson.encode({error = "Missing request body"}))
        return
    end

    local ok, data = pcall(cjson.decode, body)
    if not ok then
        ngx.status = 400
        ngx.say(cjson.encode({error = "Invalid JSON"}))
        return
    end

    if not data.id or not data.name or not data.bare_repo_path or not data.main_worktree_path then
        ngx.status = 400
        ngx.say(cjson.encode({error = "Missing required fields: id, name, bare_repo_path, main_worktree_path"}))
        return
    end

    local sql = string.format(
        "INSERT INTO projects (id, name, bare_repo_path, main_worktree_path, remote_url) VALUES (%s, %s, %s, %s, %s) RETURNING *",
        db.escape_literal(data.id),
        db.escape_literal(data.name),
        db.escape_literal(data.bare_repo_path),
        db.escape_literal(data.main_worktree_path),
        db.escape_literal(data.remote_url or "")
    )

    local res, err = db.query(sql)
    if not res then
        ngx.status = 500
        ngx.say(cjson.encode({error = err}))
        return
    end

    ngx.status = 201
    ngx.say(cjson.encode(res[1]))

elseif method == "DELETE" then
    local args = ngx.req.get_uri_args()
    local project_id = args.id
    if not project_id then
        ngx.status = 400
        ngx.say(cjson.encode({error = "Missing project id"}))
        return
    end

    local sql = string.format("DELETE FROM projects WHERE id = %s", db.escape_literal(project_id))
    local res, err = db.query(sql)
    if not res then
        ngx.status = 500
        ngx.say(cjson.encode({error = err}))
        return
    end

    ngx.say(cjson.encode({deleted = true}))

else
    ngx.status = 405
    ngx.say(cjson.encode({error = "Method not allowed"}))
end
