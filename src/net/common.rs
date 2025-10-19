//! Common types and traits shared between client and server TLS implementations.

use std::borrow::Cow;
use std::{fmt, io};

use mlua::{IntoLua, Lua, Result, Value};

/// Socket address that can be either TCP or Unix domain socket.
pub enum AnySocketAddr {
    Tcp(std::net::SocketAddr),
    #[cfg(unix)]
    Unix(tokio::net::unix::SocketAddr),
}

impl fmt::Display for AnySocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnySocketAddr::Tcp(addr) => write!(f, "{addr}"),
            #[cfg(unix)]
            AnySocketAddr::Unix(addr) => {
                let path = addr
                    .as_pathname()
                    .map(|p| p.to_string_lossy())
                    .unwrap_or_else(|| Cow::Borrowed("(unnamed)"));
                write!(f, "{path}")
            }
        }
    }
}

impl IntoLua for AnySocketAddr {
    fn into_lua(self, lua: &Lua) -> Result<Value> {
        lua.create_string(self.to_string()).map(Value::String)
    }
}

/// Trait for getting local and peer addresses from various stream types.
pub trait AddressProvider {
    fn local_addr(&self) -> io::Result<AnySocketAddr>;
    fn peer_addr(&self) -> io::Result<AnySocketAddr>;
}

impl AddressProvider for tokio::net::TcpStream {
    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        Ok(AnySocketAddr::Tcp(self.local_addr()?))
    }

    fn peer_addr(&self) -> io::Result<AnySocketAddr> {
        Ok(AnySocketAddr::Tcp(self.peer_addr()?))
    }
}

#[cfg(unix)]
impl AddressProvider for tokio::net::UnixStream {
    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        Ok(AnySocketAddr::Unix(self.local_addr()?))
    }

    fn peer_addr(&self) -> io::Result<AnySocketAddr> {
        Ok(AnySocketAddr::Unix(self.peer_addr()?))
    }
}
