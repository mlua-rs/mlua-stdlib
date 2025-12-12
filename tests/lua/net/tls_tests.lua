local task = require("@task")
local tcp = require("@net/tcp")
local tls = require("@tls")

-- Certificate paths for testing
local CERT_DIR = "tests/certs/"
local SERVER_CERT = CERT_DIR .. "server-cert.pem"
local SERVER_KEY = CERT_DIR .. "server-key.pem"
local CA_CERT = CERT_DIR .. "ca-cert.pem"
local CLIENT_CERT = CERT_DIR .. "client-cert.pem"
local CLIENT_KEY = CERT_DIR .. "client-key.pem"

testing:test("TLS basic connection to external server", function(t)
    local stream, err = tcp.connect("www.google.com", 443)
    t.assert_ne(stream, nil, err)

    -- Wrap the TCP stream with TLS
    local tls_stream, tls_err = tls.wrap(stream)
    t.assert_ne(tls_stream, nil, tls_err)

    -- Original stream should be consumed and unusable
    local ok, _err = pcall(function()
        return stream:local_addr()
    end)
    t.assert_eq(ok, false, "Original stream should be consumed")

    -- Try to send an HTTP request
    local write_ok, write_err = tls_stream:write_all("GET / HTTP/1.1\r\nHost: www.google.com\r\n\r\n")
    t.assert_ne(write_ok, nil, write_err)
    local data, read_err = tls_stream:read(1024)
    t.assert_ne(data, nil, read_err)
    t.assert_contains(data, "HTTP/1.1", "Should receive HTTP response")

    tls_stream:shutdown()
end)

testing:test("TLS ping-pong", function(t)
    -- Create server (listener) configuration
    local server_config, server_err = tls.TlsServerConfig.new({
        cert_chain = SERVER_CERT,
        private_key = SERVER_KEY,
    })
    t.assert_ne(server_config, nil, server_err)

    local listener, listen_err = tcp.listen("127.0.0.1")
    t.assert_ne(listener, nil, listen_err)
    local port = listener:local_addr():match(":(%d+)$")

    -- Spawn server task
    task.spawn(function()
        local stream, accept_err = listener:accept()
        t.assert_ne(stream, nil, accept_err)

        -- Accept TLS connection
        local tls_stream, tls_err = tls.accept(stream, server_config)
        t.assert_ne(tls_stream, nil, tls_err)

        while true do
            local data = tls_stream:read(100)
            if data == "ping" then
                tls_stream:write_all("pong")
            else
                tls_stream:write_all(string.reverse(data))
            end
        end
    end)

    -- Create client configuration with custom CA
    local client_config, client_err = tls.TlsClientConfig.new({
        ca_certs = { CA_CERT },
    })
    t.assert_ne(client_config, nil, client_err)

    local stream, connect_err = tcp.connect("127.0.0.1", port)
    t.assert_ne(stream, nil, connect_err)

    -- Wrap with TLS
    local tls_stream, tls_err = tls.wrap(stream, nil, client_config)
    t.assert_ne(tls_stream, nil, tls_err)

    -- Send message to server and read response
    local write_ok, write_err = tls_stream:write_all("Hello from client!")
    t.assert_ne(write_ok, nil, write_err)
    local data, read_err = tls_stream:read(1024)
    t.assert_ne(data, nil, read_err)
    t.assert_eq(data, "!tneilc morf olleH")
    tls_stream:write_all("ping")
    t.assert_eq(tls_stream:read(100), "pong")

    tls_stream:shutdown()
end)

testing:test("TLS mutual authentication (mTLS)", function(t)
    -- Create server configuration that requires client certificates
    local server_config, server_err = tls.TlsServerConfig.new({
        cert_chain = SERVER_CERT,
        private_key = SERVER_KEY,
        verify_client = true,
        client_ca_certs = { CA_CERT },
    })
    t.assert_ne(server_config, nil, server_err)

    local listener = tcp.listen("127.0.0.1")
    local port = listener:local_addr():match(":(%d+)$")

    -- Spawn server task
    task.spawn(function()
        local stream = listener:accept()
        local tls_stream, tls_err = tls.accept(stream, server_config)
        t.assert_ne(tls_stream, nil, tls_err)

        local data = tls_stream:read(1024)
        t.assert_eq(data, "mTLS test")
        tls_stream:write_all("mTLS success")
        tls_stream:send_close_notify()

        tls_stream:shutdown()
    end)

    -- Create client configuration with client certificate
    local client_config, client_err = tls.TlsClientConfig.new({
        ca_certs = { CA_CERT },
        client_cert = CLIENT_CERT,
        client_key = CLIENT_KEY,
    })
    t.assert_ne(client_config, nil, client_err)

    -- Connect with client certificate
    local stream = tcp.connect("localhost", port)
    local tls_stream = tls.wrap(stream, nil, client_config)

    tls_stream:write_all("mTLS test")
    local response, read_err = tls_stream:read_to_end()
    t.assert_eq(response, "mTLS success", read_err)

    tls_stream:shutdown()
end)

testing:test("TLS client without certificate verification", function(t)
    local server_config = tls.TlsServerConfig.new({
        cert_chain = SERVER_CERT,
        private_key = SERVER_KEY,
    })
    local listener = tcp.listen("127.0.0.1")
    local port = listener:local_addr():match(":(%d+)$")

    -- Spawn server task
    task.spawn(function()
        local stream = listener:accept()
        local tls_stream = tls.accept(stream, server_config)
        tls_stream:write_all("Goodbye")
        tls_stream:send_close_notify()
        tls_stream:shutdown()
    end)

    -- Create client configuration that doesn't verify certificates
    local client_config = tls.TlsClientConfig.new({
        dangerous_verify_certs = false,
    })

    local stream = tcp.connect("127.0.0.1", port)
    local tls_stream, err = tls.wrap(stream, "example.com", client_config)
    t.assert_ne(tls_stream, nil, err)

    local response = tls_stream:read_to_end()
    t.assert_eq(response, "Goodbye")

    tls_stream:shutdown()
end)

testing:test("TLS connection with invalid server name", function(t)
    local server_config = tls.TlsServerConfig.new({
        cert_chain = SERVER_CERT,
        private_key = SERVER_KEY,
    })
    local listener = tcp.listen("127.0.0.1")
    local port = listener:local_addr():match(":(%d+)$")

    -- Spawn server task
    task.spawn(function()
        local stream = listener:accept()
        -- This should fail during TLS handshake
        local _tls_stream = tls.accept(stream, server_config)
    end)

    local client_config = tls.TlsClientConfig.new({
        ca_certs = { CA_CERT },
    })

    -- Try to connect using wrong server name
    local stream = tcp.connect("127.0.0.1", port)
    local tls_stream, err = tls.wrap(stream, "wronghost.example.com", client_config)
    t.assert_eq(tls_stream, nil, "Connection should fail with wrong server name")
    t.assert_match(err, "certificate not valid for.*wronghost.example.com")
end)

testing:test("TLS ALPN negotiation", function(t)
    local server_config, server_err = tls.TlsServerConfig.new({
        cert_chain = SERVER_CERT,
        private_key = SERVER_KEY,
        alpn_protocols = { "h2", "http/1.1" },
    })
    t.assert_ne(server_config, nil, server_err)

    local listener = tcp.listen("127.0.0.1")
    local port = listener:local_addr():match(":(%d+)$")

    -- Spawn server task
    task.spawn(function()
        local stream = listener:accept()
        local tls_stream, tls_err = tls.accept(stream, server_config)
        t.assert_ne(tls_stream, nil, tls_err)

        -- Check connection info on server side
        local conn_info = tls_stream:connection_info()
        t.assert_eq(conn_info.alpn_protocol, "h2", "Should negotiate h2 protocol")
        tls_stream:write_all("ALPN test ok")
        tls_stream:send_close_notify()

        tls_stream:shutdown()
    end)

    -- Create client configuration with ALPN protocols (prefer h2)
    local client_config, client_err = tls.TlsClientConfig.new({
        ca_certs = { CA_CERT },
        alpn_protocols = { "h2", "http/1.1" },
    })
    t.assert_ne(client_config, nil, client_err)

    local stream = tcp.connect("127.0.0.1", port)
    local tls_stream, tls_err = tls.wrap(stream, "localhost", client_config)
    t.assert_ne(tls_stream, nil, tls_err)

    local conn_info = tls_stream:connection_info()
    t.assert_eq(conn_info.alpn_protocol, "h2", "Should negotiate h2 protocol")

    local response = tls_stream:read_to_end()
    t.assert_eq(response, "ALPN test ok")
    tls_stream:shutdown()
end)
