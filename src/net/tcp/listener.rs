use std::io;
use std::net::SocketAddr;
use std::result::Result as StdResult;

use mlua::{Lua, Result, Table, UserData, UserDataMethods, UserDataRegistry};
use tokio::net::lookup_host;

use super::{SocketOptions, TcpSocket, TcpStream};
use crate::net::common::AnySocketAddr;

pub struct TcpListener(pub(crate) tokio::net::TcpListener);

impl TcpListener {
    pub(crate) fn local_addr(&self) -> io::Result<AnySocketAddr> {
        self.0.local_addr().map(AnySocketAddr::IP)
    }
}

impl UserData for TcpListener {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("local_addr", |_, this, ()| Ok(this.local_addr()?));

        registry.add_async_function("listen", listen);

        registry.add_async_method("accept", |_, this, ()| async move {
            let (stream, _) = lua_try!(this.0.accept().await);
            Ok(Ok(TcpStream::from(stream)))
        });
    }
}

pub async fn listen(
    _: Lua,
    (addr, port, params): (String, Option<u16>, Option<Table>),
) -> Result<StdResult<TcpListener, String>> {
    let port = port.unwrap_or(0);
    let addrs = lua_try!(lookup_host((addr, port)).await);

    let sock_options = SocketOptions::from_table(&params)?;
    let backlog = opt_param!(params, "backlog")?;

    let try_listen = |addr: SocketAddr| {
        let sock = TcpSocket::new_for_addr(addr)?;
        sock.set_options(sock_options)?;
        sock.0.set_reuseaddr(true)?;
        sock.0.bind(addr)?;
        let listener = TcpListener(sock.0.listen(backlog.unwrap_or(1024))?);
        io::Result::Ok(listener)
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
