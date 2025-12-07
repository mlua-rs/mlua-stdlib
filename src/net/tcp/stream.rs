use std::io;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::result::Result as StdResult;
use std::task::{Context, Poll};

use mlua::{Lua, Result, String as LuaString, Table, UserData, UserDataMethods, UserDataRegistry};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, ReadBuf};
use tokio::net::{TcpStream, lookup_host};

use super::{LuaTcpSocket, SocketOptions};
use crate::net::{AddressProvider, AnySocketAddr};
use crate::time::Duration;

/// Lua wrapper around tokio [`TcpStream`].
pub struct LuaTcpStream {
    pub(crate) stream: TcpStream,
    pub(crate) host: Option<String>,
    pub(crate) read_timeout: Option<Duration>,
    pub(crate) write_timeout: Option<Duration>,
}

impl Deref for LuaTcpStream {
    type Target = TcpStream;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}

impl DerefMut for LuaTcpStream {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}

impl From<TcpStream> for LuaTcpStream {
    #[inline]
    fn from(stream: TcpStream) -> Self {
        Self {
            stream,
            host: None,
            read_timeout: None,
            write_timeout: None,
        }
    }
}

impl AddressProvider for LuaTcpStream {
    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        self.stream.local_addr().map(AnySocketAddr::IP)
    }

    fn peer_addr(&self) -> io::Result<AnySocketAddr> {
        self.stream.peer_addr().map(AnySocketAddr::IP)
    }
}

impl AsyncRead for LuaTcpStream {
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
    }
}

impl AsyncWrite for LuaTcpStream {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
    }
}

impl UserData for LuaTcpStream {
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

/// Connects to a TCP server at the given host and port.
///
/// # Arguments
/// * `host` - The hostname or IP address of the server.
/// * `port` - The port number of the server.
/// * `params` - Optional table of socket options.
///
/// The following options can be specified:
/// * `timeout`: A general timeout applied to all operations.
/// * `connect_timeout`: Timeout for the connect operation.
/// * `read_timeout`: Timeout for read operations.
/// * `write_timeout`: Timeout for write operations.
pub async fn connect(
    _: Lua,
    (host, port, params): (String, u16, Option<Table>),
) -> Result<StdResult<LuaTcpStream, String>> {
    let addrs = lua_try!(lookup_host((&*host, port)).await);
    let options = SocketOptions::from_table(&params)?;

    let timeout = opt_param!(Duration, params, "timeout")?; // A single timeout for any operation
    let connect_timeout = opt_param!(Duration, params, "connect_timeout")?.or(timeout);
    let read_timeout = opt_param!(Duration, params, "read_timeout")?.or(timeout);
    let write_timeout = opt_param!(Duration, params, "write_timeout")?.or(timeout);

    let try_connect = |addr: SocketAddr| async move {
        let sock = LuaTcpSocket::new_for_addr(addr)?;
        sock.set_options(options)?;
        with_io_timeout!(connect_timeout, sock.0.connect(addr))
    };

    let mut last_err = None;
    for addr in addrs {
        match try_connect(addr).await {
            Ok(stream) => {
                return Ok(Ok(LuaTcpStream {
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
