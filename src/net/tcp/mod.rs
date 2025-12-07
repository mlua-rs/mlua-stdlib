use mlua::{Lua, Result, Table};

pub use listener::{LuaTcpListener, listen};
pub use stream::{LuaTcpStream, connect};

use socket::{LuaTcpSocket, SocketOptions};

/// A loader for the `net/tcp` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("TcpListener", lua.create_proxy::<LuaTcpListener>()?)?;
    t.set("TcpStream", lua.create_proxy::<LuaTcpStream>()?)?;
    t.set("listen", lua.create_async_function(listen)?)?;
    t.set("connect", lua.create_async_function(connect)?)?;
    Ok(t)
}

/// Registers the `net/tcp` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@net/tcp");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}

mod listener;
mod socket;
mod stream;
