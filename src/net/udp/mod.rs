use mlua::{Lua, Result, Table};

pub use socket::{UdpSocket, bind};

/// A loader for the `net/udp` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("UdpSocket", lua.create_proxy::<UdpSocket>()?)?;
    t.set("bind", lua.create_async_function(bind)?)?;
    Ok(t)
}

/// Registers the `net/udp` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@net/udp");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}

mod socket;
