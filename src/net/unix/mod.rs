mod listener;
mod stream;

pub use listener::{UnixListener, listen};
pub use stream::{UnixStream, connect};

use mlua::{Lua, Result, Table};

/// Registers the `unix` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@unix");
    let t = lua.create_table()?;
    t.set("UnixListener", lua.create_proxy::<UnixListener>()?)?;
    t.set("UnixStream", lua.create_proxy::<UnixStream>()?)?;
    t.set("listen", lua.create_async_function(listen)?)?;
    t.set("connect", lua.create_async_function(connect)?)?;
    lua.register_module(name, &t)?;
    Ok(t)
}
