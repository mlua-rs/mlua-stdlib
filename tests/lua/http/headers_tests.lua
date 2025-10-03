local http = require("@http")

testing:test("Headers basic operations", function(t)
    local headers = http.Headers.new()

    -- Set and get a header
    headers:set("Content-Type", "application/json")
    t.assert_eq(headers["Content-Type"], "application/json")
    t.assert_eq(headers:get("Content-Type"), "application/json")
    t.assert_eq(headers:get_count("Content-Type"), 1)
    t.assert_same(headers:get_all("Content-Type"), { "application/json" })

    -- Test case-insensitivity
    t.assert_eq(headers:get("content-type"), "application/json")
    t.assert_eq(headers:get("CONTENT-TYPE"), "application/json")

    -- Append a new value to an existing header
    headers:add("Content-Type", "text/html")
    t.assert_eq(headers["Content-Type"], "application/json")
    t.assert_eq(headers:get("Content-Type"), "application/json") -- get returns the first value
    t.assert_eq(headers:get_count("Content-Type"), 2)
    t.assert_same(headers:get_all("Content-Type"), { "application/json", "text/html" })

    -- Overwrite existing header
    headers:set("Content-Type", "text/plain")
    t.assert_same(headers:get_all("Content-Type"), { "text/plain" })
    headers["Content-Type"] = "text/markdown"
    t.assert_same(headers:get_all("Content-Type"), { "text/markdown" })

    -- Assign multiple values at once
    headers["Set-Cookie"] = { "id=123", "token=abc" }
    t.assert_same(headers:get_all("Set-Cookie"), { "id=123", "token=abc" })

    -- Append to a non-existing header
    headers:add("X-Custom-Header", "value1")
    t.assert_eq(headers:get("X-Custom-Header"), "value1")

    -- Total count of values
    t.assert_eq(headers:count(), 4)
    headers:add("Content-Type", "application/xml")
    t.assert_eq(headers:count(), 5)

    -- Names of all headers
    t.assert_same(headers:keys(), { "content-type", "set-cookie", "x-custom-header" })

    -- Remove a header
    headers:remove("Content-Type")
    headers["set-cookie"] = nil -- alternative way to remove
    t.assert_eq(headers:get("Content-Type"), nil)
    t.assert_eq(headers:get_count("Content-Type"), 0)
    t.assert_eq(headers:count(), 1)

    -- Clear all headers
    headers:clear()
    t.assert_eq(headers:count(), 0)

    -- Clone a headers object and ensure independence
    headers:set("X-Test", "a")
    local clone = headers:clone()
    clone:add("X-Test", "b")
    t.assert_same(headers:get_all("X-Test"), { "a" })
    t.assert_same(clone:get_all("X-Test"), { "a", "b" })
end)

testing:test("Headers to_table", function(t)
    local headers = http.Headers.new({
        ["Content-Type"] = "application/json",
        ["Set-Cookie"] = { "id=123", "token=abc" },
    })

    local tbl = headers:to_table()
    -- Ensure access is case-insensitive
    t.assert_eq(tbl["content-type"], "application/json")
    tbl["X-Custom"] = "value"

    t.assert_same(tbl, {
        ["content-type"] = "application/json",
        ["set-cookie"] = { "id=123", "token=abc" },
        ["x-custom"] = "value",
    })
end)

testing:test("Headers errors", function(t)
    local headers = http.Headers.new()
    headers:set("X-Test", "valid")

    local ok, err = pcall(function()
        return http.Headers.new(123)
    end)
    t.assert_eq(ok, false)
    t.assert_match(err, "error converting Lua integer to table")

    -- Invalid header name
    ok, err = pcall(function()
        headers:set("Invalid Header", "value")
    end)
    t.assert_eq(ok, false)
    t.assert_match(err, "invalid HTTP header name")

    -- Invalid header value
    ok, err = pcall(function()
        headers:set("X-Test", "invalid\x01value")
    end)
    t.assert_eq(ok, false)
    t.assert_match(err, "failed to parse header value")
end)
