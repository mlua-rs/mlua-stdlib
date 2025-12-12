local task = require("@task")
local time = require("@time")
local udp = require("@net/udp")

testing:test("UDP ping-pong", function(t)
    local server, server_err = udp.bind("127.0.0.1")
    t.assert_ne(server, nil, server_err)
    local server_port = server:local_addr():match(":(%d+)$")

    local client, client_err = udp.bind("127.0.0.1")
    t.assert_ne(client, nil, client_err)

    -- Server task
    task.spawn(function()
        local data, addr_or_err = server:recv_from(100)
        t.assert_ne(data, nil, addr_or_err)
        local addr = addr_or_err
        t.assert_eq(data, "ping")

        -- Extract port from address
        local client_host, client_port = addr:match("([^:]+):(%d+)")
        local sent, send_err = server:send_to("pong", client_host, client_port)
        t.assert_ne(sent, nil, send_err)
    end)

    -- Client send_to/receive_from
    local sent, send_err = client:send_to("ping", "127.0.0.1", server_port)
    t.assert_ne(sent, nil, send_err)
    t.assert_eq(sent, 4)
    local data, recv_err = client:recv_from(100)
    t.assert_ne(data, nil, recv_err)
    t.assert_eq(data, "pong")
end)

testing:test("UDP connected socket", function(t)
    local server, server_err = udp.bind("127.0.0.1")
    t.assert_ne(server, nil, server_err)
    local server_port = server:local_addr():match(":(%d+)$")

    local client, client_err = udp.bind("127.0.0.1")
    t.assert_ne(client, nil, client_err)

    -- Connect client to server
    local ok, connect_err = client:connect("127.0.0.1", server_port)
    t.assert_ne(ok, nil, connect_err)

    -- Server task
    task.spawn(function()
        local data, addr, recv_err = server:recv_from(100)
        t.assert_ne(data, nil, recv_err)
        t.assert_eq(data, "hello")

        local client_host, client_port = addr:match("([^:]+):(%d+)")
        server:send_to("world", client_host, client_port)
    end)

    -- Client uses send/recv (after connect)
    local sent, send_err = client:send("hello")
    t.assert_ne(sent, nil, send_err)
    t.assert_eq(sent, 5)
    local data, recv_err = client:recv(100)
    t.assert_ne(data, nil, recv_err)
    t.assert_eq(data, "world")
end)

testing:test("UDP timeout", function(t)
    local socket, err = udp.bind("127.0.0.1", nil, { recv_timeout = "100ms" })
    t.assert_ne(socket, nil, err)

    local start = time.Instant.now()
    local data, recv_err = socket:recv(100)
    local elapsed = start:elapsed():as_secs()

    t.assert_eq(data, nil)
    t.assert_match(recv_err, "deadline has elapsed")
    t.assert(elapsed >= 0.1, "elapsed time should be at least 100ms, got " .. tostring(elapsed))
end)

testing:test("UDP broadcast", function(t)
    local socket, err = udp.bind("0.0.0.0")
    t.assert_ne(socket, nil, err)

    t.assert_eq(socket:broadcast(), false)
    local bc_ok, bc_err = socket:set_broadcast(true)
    t.assert_eq(bc_ok, true, bc_err)
    t.assert_eq(socket:broadcast(), true)
end)
