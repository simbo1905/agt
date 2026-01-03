local pgmoon = require("pgmoon")

local _M = {}

local db_config = {
    host = "127.0.0.1",
    port = "5432",
    database = "agt_deck",
    user = "consensussolutions"
}

function _M.connect()
    local pg = pgmoon.new(db_config)
    pg:settimeout(5000)
    local ok, err = pg:connect()
    if not ok then
        return nil, "Failed to connect to database: " .. (err or "unknown error")
    end
    return pg
end

function _M.query(sql, ...)
    local pg, err = _M.connect()
    if not pg then
        return nil, err
    end

    local res, query_err, partial, num_queries
    if select("#", ...) > 0 then
        res, query_err, partial, num_queries = pg:query(sql, ...)
    else
        res, query_err, partial, num_queries = pg:query(sql)
    end

    pg:keepalive()

    if not res then
        return nil, "Query failed: " .. (query_err or "unknown error")
    end

    return res
end

function _M.escape_literal(value)
    local pg, err = _M.connect()
    if not pg then
        return "'" .. tostring(value):gsub("'", "''") .. "'"
    end
    local escaped = pg:escape_literal(value)
    pg:keepalive()
    return escaped
end

function _M.escape_identifier(value)
    local pg, err = _M.connect()
    if not pg then
        return '"' .. tostring(value):gsub('"', '""') .. '"'
    end
    local escaped = pg:escape_identifier(value)
    pg:keepalive()
    return escaped
end

return _M
