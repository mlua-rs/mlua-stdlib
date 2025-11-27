//! Generic TLS support for network streams.

use mlua::{Result, Table};

pub use server::TlsListener;
pub use stream::TlsStream;

/// Registers the `tls` module in the given Lua state.
pub fn register(lua: &mlua::Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@tls");
    let t = lua.create_table()?;
    t.set("TlsClientConfig", lua.create_proxy::<client::TlsClientConfig>()?)?;
    t.set("TlsServerConfig", lua.create_proxy::<server::TlsServerConfig>()?)?;
    t.set("wrap", lua.create_async_function(client::wrap_stream)?)?;
    t.set("accept", lua.create_async_function(server::wrap_accept_stream)?)?;
    // t.set("wrap_listener", lua.create_function(server::wrap_listener)?)?;
    lua.register_module(name, &t)?;
    Ok(t)
}

pub mod client;
pub mod server;
pub mod stream;
