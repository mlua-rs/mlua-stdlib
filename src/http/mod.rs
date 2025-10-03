use mlua::{Lua, Result, Table};

/// A loader for the `http` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("Headers", lua.create_proxy::<headers::Headers>()?)?;
    Ok(t)
}

/// Registers the `http` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@http");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}

mod headers;
