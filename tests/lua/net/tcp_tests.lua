local task = require("@task")
local tcp = require("@net/tcp")
local time = require("@time")

testing:test("Tcp ping-pong", function(t)
    local listener, err = tcp.listen("127.0.0.1:0")
    t.assert_ne(listener, nil, err)
    local port = listener:local_addr():match(":(%d+)$")

    -- Accept only one connection
    task.spawn(function()
        local stream, err2 = listener:accept()
        t.assert_ne(stream, nil, err2)

        while true do
            local data = stream:read(100)
            if data == "ping" then
                stream:write_all("pong")
            else
                stream:write_all(string.rep(data, 2))
            end
        end
    end)

    local stream, err3 = tcp.connect(string.format("127.0.0.1:%d", port))
    t.assert_ne(stream, nil, err3)
    stream:write_all("ping")
    local response = stream:read(100)
    t.assert_eq(response, "pong")
    stream:write_all("hello")
    local response2 = stream:read(100)
    t.assert_eq(response2, "hellohello")
    stream:shutdown()
end)

testing:test("Tcp connect timeout", function(t)
    -- The IP address is reserved for documentation and should be non-routable
    local stream, err = tcp.connect("203.0.113.95:1234", { timeout = "100ms" })
    t.assert_eq(stream, nil)
    t.assert_match(err, "deadline has elapsed")
end)

testing:test("Tcp read/write timeout", function(t)
    local listener = tcp.listen("127.0.0.1:0")
    local port = listener:local_addr():match(":(%d+)$")

    task.spawn(function()
        local stream = listener:accept()
        task.sleep("1s")
        stream:shutdown()
    end)

    local stream = tcp.connect(
        string.format("127.0.0.1:%d", port),
        { read_timeout = "100ms", write_timeout = "20ms", send_buffer_size = 1024 }
    )
    local start = time.instant()
    local data, err = stream:read(10)
    local elapsed = start:elapsed():as_secs()
    t.assert_eq(data, nil)
    t.assert_match(err, "deadline has elapsed")
    t.assert(elapsed >= 0.1, "elapsed time should be at least 100ms, got " .. tostring(elapsed))

    start = time.instant()
    local ok, err2 = stream:write_all(string.rep("abcdef", 100000))
    elapsed = start:elapsed():as_secs()
    t.assert_eq(ok, nil)
    t.assert_match(err2, "deadline has elapsed")
    t.assert(elapsed >= 0.02 and elapsed < 0.03, "elapsed time should be at least 20ms, got " .. tostring(elapsed))

    stream:shutdown()
end)
