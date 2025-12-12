local task = require("@task")
local tcp = require("@net/tcp")
local time = require("@time")

testing:test("TCP ping-pong", function(t)
    local listener, listen_err = tcp.listen("127.0.0.1")
    t.assert_ne(listener, nil, listen_err)
    local port = listener:local_addr():match(":(%d+)$")

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

    local stream, connect_err = tcp.connect("127.0.0.1", port)
    t.assert_ne(stream, nil, connect_err)

    stream:write_all("ping")
    local response = stream:read(100)
    t.assert_eq(response, "pong")
    stream:write_all("hello")
    local response2 = stream:read(100)
    t.assert_eq(response2, "olleh")

    stream:shutdown()
end)

testing:test("TCP connect timeout", function(t)
    -- The IP address is reserved for documentation and should be non-routable
    local stream, err = tcp.connect("203.0.113.95", 1234, { timeout = "100ms" })
    t.assert_eq(stream, nil)
    t.assert_match(err, "deadline has elapsed")
end)

testing:test("TCP read/write timeout", function(t)
    local listener = tcp.listen("127.0.0.1")
    local port = listener:local_addr():match(":(%d+)$")

    task.spawn(function()
        local stream = listener:accept()
        task.sleep("1s")
        stream:shutdown()
    end)

    local stream =
        tcp.connect("127.0.0.1", port, { read_timeout = "100ms", write_timeout = "20ms", send_buffer_size = 1024 })
    local start = time.Instant.now()
    local data, read_err = stream:read(10)
    local elapsed = start:elapsed():as_secs()
    t.assert_eq(data, nil)
    t.assert_match(read_err, "deadline has elapsed")
    t.assert(elapsed >= 0.1, "elapsed time should be at least 100ms, got " .. tostring(elapsed))

    start = time.Instant.now()
    local ok, write_err = stream:write_all(string.rep("abcdef", 100000))
    elapsed = start:elapsed():as_secs()
    t.assert_eq(ok, nil)
    t.assert_match(write_err, "deadline has elapsed")
    t.assert(elapsed >= 0.02 and elapsed < 0.03, "elapsed time should be at least 20ms, got " .. tostring(elapsed))

    stream:shutdown()
end)
