use mlua::{Lua, Result, Table};

/// A loader for the `assertions` module.
fn loader(lua: &Lua) -> Result<Table> {
    lua.load(include_str!("../lua/assertions.lua"))
        .set_name("@mlua-stdlib/assertions.lua")
        .call(())
}

/// Registers the `assertions` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@assertions");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
