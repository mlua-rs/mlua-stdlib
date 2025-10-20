use std::io;
use std::ops::Deref;
use std::result::Result as StdResult;

use mlua::{Lua, Result, String as LuaString, Table, UserData, UserDataMethods, UserDataRegistry, Value};

use crate::net::{AddressProvider, AnySocketAddr};
use crate::time::Duration;

/// UDP socket wrapper.
pub struct UdpSocket {
    pub(crate) socket: tokio::net::UdpSocket,
    pub(crate) recv_timeout: Option<Duration>,
}

impl Deref for UdpSocket {
    type Target = tokio::net::UdpSocket;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.socket
    }
}

impl From<tokio::net::UdpSocket> for UdpSocket {
    fn from(socket: tokio::net::UdpSocket) -> Self {
        UdpSocket {
            socket,
            recv_timeout: None,
        }
    }
}

impl AddressProvider for UdpSocket {
    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        self.socket.local_addr().map(AnySocketAddr::IP)
    }

    fn peer_addr(&self) -> io::Result<AnySocketAddr> {
        self.socket.peer_addr().map(AnySocketAddr::IP)
    }
}

impl UserData for UdpSocket {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_async_function("bind", bind);

        registry.add_method("local_addr", |_, this, ()| Ok(this.local_addr()?));
        registry.add_method("peer_addr", |_, this, ()| Ok(this.peer_addr()?));

        registry.add_method_mut("set_recv_timeout", |_, this, dur: Option<Duration>| {
            this.recv_timeout = dur;
            Ok(())
        });

        registry.add_method("set_ttl", |_, this, ttl: u32| {
            lua_try!(this.socket.set_ttl(ttl));
            Ok(Ok(()))
        });

        registry.add_method("ttl", |_, this, ()| {
            let ttl = lua_try!(this.socket.ttl());
            Ok(Ok(ttl))
        });

        registry.add_async_method("connect", |_, this, (host, port): (String, u16)| async move {
            lua_try!(this.socket.connect((host, port)).await);
            Ok(Ok(true))
        });

        registry.add_async_method_mut("send", |_, this, data: LuaString| async move {
            let n = lua_try!(this.socket.send(&data.as_bytes()).await);
            Ok(Ok(n))
        });

        registry.add_async_method_mut("recv", |lua, this, size: Option<usize>| async move {
            let size = size.unwrap_or(1472); // Default MTU size minus UDP header
            let mut buf = vec![0; size]; // TODO: reuse buffer?
            let n = with_io_timeout!(this.recv_timeout, this.socket.recv(&mut buf));
            let n = lua_try!(n);
            buf.truncate(n);
            Ok(Ok(lua.create_string(buf)?))
        });

        registry.add_async_method_mut(
            "send_to",
            |_, this, (data, host, port): (LuaString, String, u16)| async move {
                let n = lua_try!(this.socket.send_to(&data.as_bytes(), (host, port)).await);
                Ok(Ok(n))
            },
        );

        registry.add_async_method_mut("recv_from", |lua, this, size: Option<usize>| async move {
            let size = size.unwrap_or(1472); // Default MTU size minus UDP header
            let mut buf = vec![0; size]; // TODO: reuse buffer?
            match with_io_timeout!(this.recv_timeout, this.socket.recv_from(&mut buf)) {
                Ok((n, addr)) => {
                    buf.truncate(n);
                    let data = lua.create_string(buf)?;
                    Ok((Value::String(data), addr.to_string()))
                }
                Err(e) => Ok((Value::Nil, e.to_string())),
            }
        });

        registry.add_method("set_broadcast", |_, this, enable: bool| {
            lua_try!(this.socket.set_broadcast(enable));
            Ok(Ok(true))
        });

        registry.add_method("broadcast", |_, this, ()| {
            let enabled = lua_try!(this.socket.broadcast());
            Ok(Ok(enabled))
        });
    }
}

/// Binds a UDP socket to the given host and port with optional parameters.
pub async fn bind(
    _: Lua,
    (host, port, params): (String, Option<u16>, Option<Table>),
) -> Result<StdResult<UdpSocket, String>> {
    let port = port.unwrap_or(0);
    let recv_timeout = opt_param!(Duration, params, "recv_timeout")?;

    let socket = lua_try!(tokio::net::UdpSocket::bind((host, port)).await);

    Ok(Ok(UdpSocket { socket, recv_timeout }))
}
