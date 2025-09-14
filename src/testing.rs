use mlua::{Lua, Result, Table, Value};

/// A loader for the `testing` module.
pub(crate) fn loader(lua: &Lua) -> Result<Table> {
    // This module has several dependencies, so we pass them as a table
    let deps = lua.create_table()?;

    let assertions = {
        let opts = lua.create_table()?;
        opts.set("level", 3)?;
        lua.load(include_str!("../lua/assertions.lua"))
            .set_name(format!("@mlua-stdlib/assertions.lua"))
            .call::<Value>(opts)?
    };
    deps.set("assertions", assertions)?;
    deps.set("print", lua.create_function(crate::terminal::print)?)?;
    deps.set("println", lua.create_function(crate::terminal::println)?)?;
    deps.set("style", lua.create_function(crate::terminal::style)?)?;
    deps.set("instant", lua.create_function(crate::time::instant)?)?;

    lua.load(include_str!("../lua/testing.lua"))
        .set_name(format!("@mlua-stdlib/testing.lua"))
        .call(deps)
}

/// Registers the `testing` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@testing");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
