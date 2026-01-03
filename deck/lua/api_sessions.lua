local db = require "db"
local cjson = require "cjson"
cjson.encode_empty_table_as_object(false)

local method = ngx.req.get_method()

if method == "GET" then
    local args = ngx.req.get_uri_args()
    local project_id = args.project_id

    local sql
    if project_id then
        sql = string.format(
            "SELECT * FROM sessions WHERE project_id = %s ORDER BY created_at DESC",
            db.escape_literal(project_id)
        )
    else
        sql = "SELECT * FROM sessions ORDER BY created_at DESC"
    end

    local res, err = db.query(sql)
    if not res then
        ngx.status = 500
        ngx.say(cjson.encode({error = err}))
        return
    end
    ngx.say(cjson.encode(res))

else
    ngx.status = 405
    ngx.say(cjson.encode({error = "Method not allowed"}))
end
