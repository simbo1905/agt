local db = require "db"
local cjson = require "cjson"
local http = require "resty.http"

local session_id = ngx.var.session_id
local opencode_path = ngx.var.opencode_path or ""

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

local httpc = http.new()
httpc:set_timeout(30000)

local upstream_uri = "http://127.0.0.1:" .. port .. "/" .. opencode_path
local args = ngx.req.get_uri_args()
if next(args) then
    upstream_uri = upstream_uri .. "?" .. ngx.encode_args(args)
end

ngx.req.read_body()
local body = ngx.req.get_body_data()

local res, err = httpc:request_uri(upstream_uri, {
    method = ngx.req.get_method(),
    body = body,
    headers = ngx.req.get_headers(),
    keepalive_timeout = 60000,
    keepalive_pool = 10
})

if not res then
    ngx.status = 502
    ngx.say(cjson.encode({error = "Failed to proxy request: " .. (err or "unknown")}))
    return
end

ngx.status = res.status
for k, v in pairs(res.headers) do
    if k:lower() ~= "transfer-encoding" and k:lower() ~= "connection" then
        ngx.header[k] = v
    end
end
ngx.print(res.body)
