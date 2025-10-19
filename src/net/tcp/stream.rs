use std::io;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::result::Result as StdResult;

use mlua::{Lua, Result, String as LuaString, Table, UserData, UserDataMethods, UserDataRegistry};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::lookup_host;

use super::{SocketOptions, TcpSocket};
use crate::net::{AddressProvider, AnySocketAddr};
use crate::time::Duration;

pub struct TcpStream {
    pub(crate) stream: tokio::net::TcpStream,
    pub(crate) host: Option<String>,
    pub(crate) read_timeout: Option<Duration>,
    pub(crate) write_timeout: Option<Duration>,
}

impl Deref for TcpStream {
    type Target = tokio::net::TcpStream;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl DerefMut for TcpStream {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl From<tokio::net::TcpStream> for TcpStream {
    fn from(stream: tokio::net::TcpStream) -> Self {
        TcpStream {
            stream,
            host: None,
            read_timeout: None,
            write_timeout: None,
        }
    }
}

impl AddressProvider for TcpStream {
    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        self.stream.local_addr().map(AnySocketAddr::Tcp)
    }

    fn peer_addr(&self) -> io::Result<AnySocketAddr> {
        self.stream.peer_addr().map(AnySocketAddr::Tcp)
    }
}

impl UserData for TcpStream {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_async_function("connect", connect);

        registry.add_method("local_addr", |_, this, ()| Ok(this.local_addr()?));
        registry.add_method("peer_addr", |_, this, ()| Ok(this.peer_addr()?));

        registry.add_method_mut("set_read_timeout", |_, this, dur: Option<Duration>| {
            this.read_timeout = dur;
            Ok(())
        });

        registry.add_method_mut("set_write_timeout", |_, this, dur: Option<Duration>| {
            this.write_timeout = dur;
            Ok(())
        });

        registry.add_async_method_mut("read", |lua, mut this, size: usize| async move {
            let mut buf = vec![0; size];
            let n = with_io_timeout!(this.read_timeout, this.read(&mut buf));
            let n = lua_try!(n);
            buf.truncate(n);
            Ok(Ok(lua.create_string(buf)?))
        });

        registry.add_async_method_mut("read_to_end", |lua, mut this, ()| async move {
            let mut buf = Vec::new();
            let n = with_io_timeout!(this.read_timeout, this.read_to_end(&mut buf));
            let _n = lua_try!(n);
            Ok(Ok(lua.create_string(buf)?))
        });

        registry.add_async_method_mut("write", |_, mut this, data: LuaString| async move {
            let n = with_io_timeout!(this.write_timeout, this.write(&data.as_bytes()));
            let n = lua_try!(n);
            Ok(Ok(n))
        });

        registry.add_async_method_mut("write_all", |_, mut this, data: LuaString| async move {
            let r = with_io_timeout!(this.write_timeout, this.write_all(&data.as_bytes()));
            lua_try!(r);
            Ok(Ok(true))
        });

        registry.add_async_method_mut("flush", |_, mut this, ()| async move {
            let r = with_io_timeout!(this.write_timeout, this.flush());
            lua_try!(r);
            Ok(Ok(true))
        });

        registry.add_async_method_mut("shutdown", |_, mut this, ()| async move {
            lua_try!(this.shutdown().await);
            Ok(Ok(true))
        });
    }
}

pub async fn connect(
    _: Lua,
    (host, port, params): (String, u16, Option<Table>),
) -> Result<StdResult<TcpStream, String>> {
    let addrs = lua_try!(lookup_host((&*host, port)).await);
    let options = SocketOptions::from_table(&params)?;

    let timeout = opt_param!(Duration, params, "timeout")?; // A single timeout for any operation
    let connect_timeout = opt_param!(Duration, params, "connect_timeout")?.or(timeout);
    let read_timeout = opt_param!(Duration, params, "read_timeout")?.or(timeout);
    let write_timeout = opt_param!(Duration, params, "write_timeout")?.or(timeout);

    let try_connect = |addr: SocketAddr| async move {
        let sock = TcpSocket::new_for_addr(addr)?;
        sock.set_options(options)?;
        with_io_timeout!(connect_timeout, sock.0.connect(addr))
    };

    let mut last_err = None;
    for addr in addrs {
        match try_connect(addr).await {
            Ok(stream) => {
                return Ok(Ok(TcpStream {
                    stream,
                    host: Some(host.clone()),
                    read_timeout,
                    write_timeout,
                }));
            }
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        }
    }

    Ok(Err(last_err.map(|err| err.to_string()).unwrap_or_else(|| {
        "could not resolve to any address".to_string()
    })))
}
