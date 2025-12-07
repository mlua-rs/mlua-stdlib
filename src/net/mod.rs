use mlua::{Lua, Result, Table};

pub use common::{AddressProvider, AnyListener, AnySocketAddr, AnyStream};
pub use tcp::{LuaTcpListener, LuaTcpStream};

/// A loader for the `net` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    Ok(t)
}

/// Registers the `net` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@net");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}

macro_rules! with_io_timeout {
    ($timeout:expr, $fut:expr) => {
        match $timeout {
            Some(dur) => (tokio::time::timeout(dur.0, $fut).await)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::TimedOut, e))
                .flatten(),
            None => $fut.await,
        }
    };
}

pub(crate) mod common;

pub mod tcp;
#[cfg(feature = "tls")]
pub mod tls;
pub mod udp;
#[cfg(unix)]
pub mod unix;
