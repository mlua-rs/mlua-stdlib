use std::io;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::result::Result as StdResult;

use mlua::{Lua, Result, Table, UserData, UserDataMethods, UserDataRegistry};
use tokio::net::{TcpListener, lookup_host};

use super::{LuaTcpSocket, LuaTcpStream, SocketOptions};
use crate::net::common::{Accept, AnySocketAddr};

/// Lua wrapper around tokio [`TcpListener`].
#[derive(Debug)]
pub struct LuaTcpListener(pub(crate) TcpListener);

impl Deref for LuaTcpListener {
    type Target = TcpListener;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LuaTcpListener {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<TcpListener> for LuaTcpListener {
    #[inline]
    fn from(listener: TcpListener) -> Self {
        LuaTcpListener(listener)
    }
}

impl Accept for LuaTcpListener {
    type Stream = LuaTcpStream;

    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        self.0.local_addr().map(AnySocketAddr::IP)
    }

    async fn accept(&self) -> io::Result<(Self::Stream, AnySocketAddr)> {
        let (stream, addr) = self.0.accept().await?;
        Ok((stream.into(), AnySocketAddr::IP(addr)))
    }
}

impl UserData for LuaTcpListener {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("local_addr", |_, this, ()| Ok(this.local_addr()?));

        registry.add_async_function("listen", listen);

        registry.add_async_method("accept", |_, this, ()| async move {
            let (stream, _) = lua_try!(this.0.accept().await);
            Ok(Ok(LuaTcpStream::from(stream)))
        });
    }
}

/// Creates a TCP listener bound to the specified address.
///
/// # Arguments
/// * `addr`: The address to bind to.
/// * `port`: The port to bind to. If `None`, a random available port will be used.
/// * `params` (optional): A table of socket options.
///
/// The following options can be specified:
/// * `backlog`: The maximum number of pending connections. Default is 1024.
pub async fn listen(
    _: Lua,
    (addr, port, params): (String, Option<u16>, Option<Table>),
) -> Result<StdResult<LuaTcpListener, String>> {
    let port = port.unwrap_or(0);
    let addrs = lua_try!(lookup_host((addr, port)).await);

    let sock_options = SocketOptions::from_table(&params)?;
    let backlog = opt_param!(params, "backlog")?;

    let try_listen = |addr: SocketAddr| {
        let sock = LuaTcpSocket::new_for_addr(addr)?;
        sock.set_options(sock_options)?;
        sock.0.set_reuseaddr(true)?;
        sock.0.bind(addr)?;
        let backlog = backlog.unwrap_or(1024);
        let listener = sock.0.listen(backlog)?;
        io::Result::Ok(listener.into())
    };

    let mut last_err = None;
    for addr in addrs {
        match try_listen(addr) {
            Ok(sock) => return Ok(Ok(sock)),
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
