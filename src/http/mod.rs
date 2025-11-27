use mlua::{Lua, Result, Table};

pub use body::LuaBody;
pub use headers::LuaHeaders;
pub use method::LuaMethod;
pub use request::LuaRequest;
pub use response::LuaResponse;

#[allow(unused_imports)]
pub(crate) use headers::LuaHeaderMapExt;

/// A loader for the `http` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("Headers", lua.create_proxy::<LuaHeaders>()?)?;
    t.set("HttpServer", lua.create_proxy::<server::HttpServer>()?)?;
    t.set("Request", lua.create_proxy::<LuaRequest>()?)?;
    t.set("Response", lua.create_proxy::<LuaResponse>()?)?;
    Ok(t)
}

/// Registers the `http` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@http");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}

mod body;
mod headers;
mod method;
mod request;
mod response;
mod server;
