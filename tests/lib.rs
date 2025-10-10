#![cfg(test)]

use std::path::Path;

use mlua::{Error, Lua, ObjectLike, Result, Table};

async fn run_file(modname: &str) -> Result<()> {
    let lua = Lua::new();

    // Preload all modules
    mlua_stdlib::assertions::register(&lua, None)?;
    mlua_stdlib::env::register(&lua, None)?;
    let testing = mlua_stdlib::testing::register(&lua, None)?;
    mlua_stdlib::time::register(&lua, None)?;

    #[cfg(feature = "json")]
    mlua_stdlib::json::register(&lua, None)?;
    #[cfg(feature = "yaml")]
    mlua_stdlib::yaml::register(&lua, None)?;
    #[cfg(feature = "regex")]
    mlua_stdlib::regex::register(&lua, None)?;
    #[cfg(feature = "http")]
    mlua_stdlib::http::register(&lua, None)?;
    #[cfg(feature = "task")]
    mlua_stdlib::task::register(&lua, None)?;

    // Add `testing` global variable (an instance of the testing framework)
    let testing = testing.call_function::<Table>("new", modname)?;
    lua.globals().set("testing", &testing)?;

    let path = format!("tests/lua/{modname}_tests.lua");
    lua.load(Path::new(&path)).exec()?;

    let local = tokio::task::LocalSet::new();
    let (ok, _results) = local
        .run_until(testing.call_async_method::<(bool, Table)>("run", ()))
        .await?;
    if ok {
        return Ok(());
    }

    let msg = format!("Tests failed for {modname}");
    Err(Error::runtime(msg))
}

// Helper macro to generate Rust test functions for Lua test modules.
macro_rules! include_tests {
    () => {};

    // Grouped tests
    ($(#[$meta:meta])? $group:ident { $($item:ident),* $(,)? }, $($rest:tt)*) => {
        $(#[$meta])*
        mod $group {
            use super::*;
            $(
                #[tokio::test]
                async fn $item() -> Result<()> {
                    run_file(&format!("{}/{}", stringify!($group), stringify!($item))).await
                }
            )*
        }

        include_tests!( $($rest)* );
    };

    ($(#[$meta:meta])? $name:ident, $($rest:tt)*) => {
        $(#[$meta])*
        #[tokio::test]
        async fn $name() -> Result<()> {
            run_file(stringify!($name)).await
        }

        include_tests!( $($rest)* );
    };
}

include_tests! {
    assertions,
    env,
    #[cfg(feature = "json")] json,
    #[cfg(feature = "regex")] regex,
    #[cfg(feature = "yaml")] yaml,

    #[cfg(feature = "http")]
    http {
        headers,
    },

    #[cfg(feature = "task")]
    task,
}
