use mlua::{Lua, Result, Table};

pub use listener::{TcpListener, listen};
pub use stream::{TcpStream, connect};

use socket::{SocketOptions, TcpSocket};

/// A loader for the `net/tcp` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("TcpListener", lua.create_proxy::<TcpListener>()?)?;
    t.set("TcpStream", lua.create_proxy::<TcpStream>()?)?;
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
