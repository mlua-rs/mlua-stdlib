local task = require("@task")
local time = require("@time")
local unix = require("@unix")

testing:test("Unix ping-pong", function(t)
    local socket_path = "/tmp/mlua_test_" .. tostring(os.time()) .. ".sock"

    local listener, listen_err = unix.listen(socket_path, { unlink_on_drop = true })
    t.assert_ne(listener, nil, listen_err)

    -- Accept only one connection
    task.spawn(function()
        local stream, accept_err = listener:accept()
        t.assert_ne(stream, nil, accept_err)

        while true do
            local data, read_err = stream:read(100)
            t.assert_ne(data, nil, read_err)
            if data == "ping" then
                stream:write_all("pong")
            else
                stream:write_all(string.reverse(data))
            end
        end
    end)

    local stream, connect_err = unix.connect(socket_path)
    t.assert_ne(stream, nil, connect_err)

    stream:write_all("ping")
    local response = stream:read(100)
    t.assert_eq(response, "pong")
    stream:write_all("hello")
    local response2 = stream:read(100)
    t.assert_eq(response2, "olleh")

    stream:shutdown()
end)

testing:test("Unix stream addresses", function(t)
    local socket_path = "/tmp/mlua_test2_" .. tostring(os.time()) .. ".sock"

    local listener = unix.listen(socket_path, { unlink_on_drop = true })
    local addr = listener:local_addr()
    t.assert_eq(addr, socket_path)
end)

testing:test("Unix stream with timeouts", function(t)
    local socket_path = "/tmp/mlua_test3_" .. tostring(os.time()) .. ".sock"
    local listener = unix.listen(socket_path, { unlink_on_drop = true })

    task.spawn(function()
        local stream = listener:accept()
        -- Don't send anything, let client timeout
        task.sleep("1s")
        stream:shutdown()
    end)

    local client = unix.connect(socket_path, { read_timeout = "100ms" })

    local start = time.Instant.now()
    local ok, err = client:read_to_end()
    local elapsed = start:elapsed():as_secs()
    t.assert_eq(ok, nil)
    t.assert_match(err, "deadline has elapsed")
    t.assert(elapsed >= 0.1, "elapsed time should be at least 100ms, got " .. tostring(elapsed))

    client:shutdown()
end)
