pub use listener::{LuaUnixListener, listen};
pub use stream::{LuaUnixStream, connect};

use mlua::{Lua, Result, Table};

/// Registers the `unix` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@unix");
    let t = lua.create_table()?;
    t.set("UnixListener", lua.create_proxy::<LuaUnixListener>()?)?;
    t.set("UnixStream", lua.create_proxy::<LuaUnixStream>()?)?;
    t.set("listen", lua.create_async_function(listen)?)?;
    t.set("connect", lua.create_async_function(connect)?)?;
    lua.register_module(name, &t)?;
    Ok(t)
}

mod listener;
mod stream;
