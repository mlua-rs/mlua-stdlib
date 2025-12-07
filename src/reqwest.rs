use mlua::{
    ErrorContext as _, ExternalResult as _, Lua, Result, Table, UserData, UserDataMethods, UserDataRegistry,
};

use crate::http::{LuaHeaders, LuaRequest, LuaResponse};
use crate::time::Duration;

/// A Lua wrapper around [`reqwest::Client`].
#[derive(Clone, Debug)]
pub struct Client(pub reqwest::Client);

impl UserData for Client {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_function("new", |_lua, params: Option<Table>| {
            let mut builder = reqwest::Client::builder();

            // Timeouts and connection pool
            if let Some(timeout) = opt_param!(Duration, params, "timeout")? {
                builder = builder.timeout(timeout.0);
            }
            if let Some(connect_timeout) = opt_param!(Duration, params, "connect_timeout")? {
                builder = builder.connect_timeout(connect_timeout.0);
            }
            if let Some(read_timeout) = opt_param!(Duration, params, "read_timeout")? {
                builder = builder.read_timeout(read_timeout.0);
            }
            if let Some(pool_idle_timeout) = opt_param!(Duration, params, "pool_idle_timeout")? {
                builder = builder.pool_idle_timeout(pool_idle_timeout.0);
            }
            if let Some(pool_max_idle_per_host) = opt_param!(params, "pool_max_idle_per_host")? {
                builder = builder.pool_max_idle_per_host(pool_max_idle_per_host);
            }

            // Cookies
            if let Some(cookie_store) = opt_param!(params, "cookie_store")? {
                builder = builder.cookie_store(cookie_store);
            }

            // TLS
            if let Some(accept_invalid_certs) = opt_param!(params, "accept_invalid_certs")? {
                builder = builder.danger_accept_invalid_certs(accept_invalid_certs);
            }
            if let Some(accept_invalid_hostnames) = opt_param!(params, "accept_invalid_hostnames")? {
                builder = builder.danger_accept_invalid_hostnames(accept_invalid_hostnames);
            }
            if let Some(https_only) = opt_param!(params, "https_only")? {
                builder = builder.https_only(https_only);
            }
            // TODO: max/min TLS versions, root certs, client certs

            // Headers
            if let Some(default_headers) = opt_param!(LuaHeaders, params, "default_headers")? {
                builder = builder.default_headers(default_headers.0);
            }

            // Protocol
            if let Some(true) = opt_param!(params, "http1_only")? {
                builder = builder.http1_only();
            }
            if let Some(true) = opt_param!(params, "http2_only")? {
                builder = builder.http2_prior_knowledge();
            }

            (builder.build().map(Client))
                .into_lua_err()
                .context("failed to build client")
        });

        registry.add_async_method(
            "request",
            |_, this, (url, req): (String, Option<LuaRequest>)| async move {
                let req = req.unwrap_or_default();
                let params = req.params();
                let (head, body) = req.into_parts();

                let mut req_builder = this.0.request(head.method, url);
                req_builder = req_builder.version(head.version);
                req_builder = req_builder.headers(head.headers);
                req_builder = req_builder.body(body);
                if let Some(timeout) = params.timeout {
                    req_builder = req_builder.timeout(timeout.0);
                }

                let req = req_builder.build().into_lua_err()?;
                let resp = lua_try!(this.0.execute(req).await);

                Ok(Ok(LuaResponse::from(resp)))
            },
        );
    }
}

/// A loader for the `reqwest` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("Client", lua.create_proxy::<Client>()?)?;
    Ok(t)
}

/// Registers the `reqwest` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@reqwest");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
