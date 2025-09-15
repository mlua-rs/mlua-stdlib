#![cfg(test)]

use std::path::Path;

use mlua::{Error, Lua, ObjectLike, Result, Table};

fn run_file(modname: &str) -> Result<()> {
    let lua = Lua::new();

    // Preload all modules
    mlua_stdlib::assertions::register(&lua, None)?;
    let testing = mlua_stdlib::testing::register(&lua, None)?;

    #[cfg(feature = "json")]
    mlua_stdlib::json::register(&lua, None)?;
    #[cfg(feature = "yaml")]
    mlua_stdlib::yaml::register(&lua, None)?;
    #[cfg(feature = "regex")]
    mlua_stdlib::regex::register(&lua, None)?;

    // Add `testing` global variable (an instance of the testing framework)
    let testing = testing.call_function::<Table>("new", modname)?;
    lua.globals().set("testing", &testing)?;

    let path = format!("tests/lua/{modname}_tests.lua");
    lua.load(Path::new(&path)).exec()?;

    // Run the tests and check results
    let (ok, _results) = testing.call_method::<(bool, Table)>("run", ())?;
    if ok {
        return Ok(());
    }

    let msg = format!("Tests failed for {modname}");
    return Err(Error::runtime(msg));
}

// Helper macro to generate Rust test functions for Lua test modules.
macro_rules! include_tests {
    ($( $(#[$meta:meta])? $name:ident $(,)? )*) => {
        $(
            $(#[$meta])*
            #[test]
            fn $name() -> Result<()> {
                run_file(stringify!($name))
            }
        )*
    };
}

include_tests! {
    assertions,
    #[cfg(feature = "json")] json,
    #[cfg(feature = "regex")] regex,
    #[cfg(feature = "yaml")] yaml,
}
