#![cfg(test)]

use std::path::Path;

use mlua::{Lua, Result};

fn make_lua() -> Result<Lua> {
    let lua = Lua::new();

    // Preload all modules
    mlua_stdlib::assertions::register(&lua, None)?;

    Ok(lua)
}

fn run_test(modname: &str) -> Result<()> {
    let lua = make_lua()?;
    lua.load(Path::new(&format!("tests/lua/{modname}_tests.lua")))
        .exec()
}

macro_rules! include_tests {
    ($($name:ident, )*) => { $(
        #[test]
        fn $name() -> Result<()> {
            run_test(stringify!($name))
        }
    )*}
}

include_tests! {
    assertions,
}
