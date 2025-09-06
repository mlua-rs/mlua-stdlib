use mlua::{Lua, Result, Table};

/// A loader for the `assertions` module.
pub fn loader(lua: &Lua, name: String) -> Result<Table> {
    lua.load(include_str!("../lua/assertions.lua"))
        .set_name(format!("={name}"))
        .call(())
}

/// Registers the `assertions` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<()> {
    let name = name.unwrap_or("@assertions");
    lua.register_module(name, loader(lua, name.to_string())?)
}
