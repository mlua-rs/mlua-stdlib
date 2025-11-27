local http = require("@http")
local tcp = require("@net/tcp")
local task = require("@task")
local reqwest = require("@reqwest")

testing:test("Simple http server", function(t)
    local listener = tcp.listen("127.0.0.1")
    local port = listener:local_addr():match(":(%d+)$")

    local server = http.HttpServer.new()
    local server_h = task.spawn(function()
        server:start(listener, function(_req)
            return "Hello, World!"
        end)
    end)

    local client = reqwest.Client.new()
    local res = client:request(string.format("http://127.0.0.1:%d", port))
    t.assert_eq(res:status(), 200)
    t.assert_eq(res:text(), "Hello, World!")

    server:graceful_shutdown()
    server_h:join()
end)
