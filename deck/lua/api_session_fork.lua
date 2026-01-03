local db = require "db"
local cjson = require "cjson"

local method = ngx.req.get_method()

if method ~= "POST" then
    ngx.status = 405
    ngx.say(cjson.encode({error = "Method not allowed"}))
    return
end

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

if not data.project_id or not data.session_id then
    ngx.status = 400
    ngx.say(cjson.encode({error = "Missing required fields: project_id, session_id"}))
    return
end

local project_sql = string.format(
    "SELECT * FROM projects WHERE id = %s",
    db.escape_literal(data.project_id)
)
local project_res, err = db.query(project_sql)
if not project_res or #project_res == 0 then
    ngx.status = 404
    ngx.say(cjson.encode({error = "Project not found"}))
    return
end

local project = project_res[1]
local from_ref = data.from_ref or "HEAD"

local cmd = string.format(
    "cd %s && agt fork --session-id %s --from %s 2>&1",
    project.main_worktree_path,
    data.session_id,
    from_ref
)

local handle = io.popen(cmd)
local result = handle:read("*a")
local success = handle:close()

if not success then
    ngx.status = 500
    ngx.say(cjson.encode({error = "Failed to fork session", details = result}))
    return
end

local worktree_path = project.main_worktree_path .. "/sessions/" .. data.session_id
local branch = "agtsessions/" .. data.session_id

local sql = string.format(
    "INSERT INTO sessions (id, project_id, branch, worktree_path, from_commit, user_branch, status) VALUES (%s, %s, %s, %s, %s, %s, 'idle') RETURNING *",
    db.escape_literal(data.session_id),
    db.escape_literal(data.project_id),
    db.escape_literal(branch),
    db.escape_literal(worktree_path),
    db.escape_literal(from_ref),
    db.escape_literal(from_ref)
)

local res, db_err = db.query(sql)
if not res then
    ngx.status = 500
    ngx.say(cjson.encode({error = db_err}))
    return
end

ngx.status = 201
ngx.say(cjson.encode(res[1]))
