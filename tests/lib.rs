#![cfg(test)]

use std::path::Path;

use mlua::{Error, Lua, ObjectLike, Result, Table};

async fn run_file(modname: &str) -> Result<()> {
    let lua = Lua::new();

    // Preload all modules
    mlua_stdlib::assertions::register(&lua, None)?;
    mlua_stdlib::bytes::register(&lua, None)?;
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
    {
        mlua_stdlib::http::register(&lua, None)?;
        mlua_stdlib::reqwest::register(&lua, None)?;
    }
    #[cfg(feature = "net")]
    {
        mlua_stdlib::net::register(&lua, None)?;
        mlua_stdlib::net::tcp::register(&lua, None)?;
        mlua_stdlib::net::udp::register(&lua, None)?;
        #[cfg(unix)]
        mlua_stdlib::net::unix::register(&lua, None)?;
    }
    #[cfg(feature = "tls")]
    mlua_stdlib::net::tls::register(&lua, None)?;
    #[cfg(feature = "task")]
    mlua_stdlib::task::register(&lua, None)?;

    // Add `testing` global variable (an instance of the testing framework)
    let testing = testing.call_function::<Table>("new", modname)?;
    lua.globals().set("testing", &testing)?;

    let path = format!("tests/lua/{modname}_tests.lua");
    lua.load(Path::new(&path)).exec()?;

    #[cfg(feature = "async")]
    let (ok, _results) = {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(testing.call_async_method::<(bool, Table)>("run", ()))
            .await?
    };
    #[cfg(not(feature = "async"))]
    let (ok, _results) = testing.call_method::<(bool, Table)>("run", ())?;
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
    ($(#[$meta:meta])* $group:ident { $($(#[$item_meta:meta])* $item:ident),* $(,)? }, $($rest:tt)*) => {
        $(#[$meta])*
        mod $group {
            use super::*;
            $(
                $(#[$item_meta])*
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
    bytes,
    env,
    time,
    #[cfg(feature = "json")] json,
    #[cfg(feature = "regex")] regex,
    #[cfg(feature = "yaml")] yaml,

    #[cfg(feature = "http")]
    http {
        headers,
        server,
    },

    #[cfg(feature = "net")]
    net {
        tcp,
        udp,
        #[cfg(feature = "tls")]
        tls,
        #[cfg(unix)]
        unix,
    },

    #[cfg(feature = "task")]
    task,
}
