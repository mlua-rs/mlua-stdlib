//! Common types and traits shared between client and server TLS implementations.

use std::any::TypeId;
use std::borrow::Cow;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{fmt, io};

use mlua::{AnyUserData, Error, FromLua, IntoLua, Lua, MaybeSend, Result, Value};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::tcp::{TcpListener, TcpStream};
#[cfg(feature = "tls")]
use super::tls::{TlsListener, TlsStream};
#[cfg(unix)]
use super::unix::{UnixListener, UnixStream};

/// Socket address that can be either TCP or Unix domain socket.
pub enum AnySocketAddr {
    IP(std::net::SocketAddr),
    #[cfg(unix)]
    Unix(tokio::net::unix::SocketAddr),
}

impl fmt::Display for AnySocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnySocketAddr::IP(addr) => write!(f, "{addr}"),
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
        Ok(AnySocketAddr::IP(self.local_addr()?))
    }

    fn peer_addr(&self) -> io::Result<AnySocketAddr> {
        Ok(AnySocketAddr::IP(self.peer_addr()?))
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

/// A stream that can be either TCP or Unix domain socket, possibly wrapped in TLS.
pub enum AnyStream {
    Tcp(TcpStream),
    #[cfg(unix)]
    Unix(UnixStream),
    #[cfg(feature = "tls")]
    TcpTls(TlsStream<TcpStream>),
    #[cfg(all(unix, feature = "tls"))]
    UnixTls(TlsStream<UnixStream>),
}

impl AsyncRead for AnyStream {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            AnyStream::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            #[cfg(unix)]
            AnyStream::Unix(s) => Pin::new(s).poll_read(cx, buf),
            #[cfg(feature = "tls")]
            AnyStream::TcpTls(s) => Pin::new(s).poll_read(cx, buf),
            #[cfg(all(unix, feature = "tls"))]
            AnyStream::UnixTls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for AnyStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            AnyStream::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            #[cfg(unix)]
            AnyStream::Unix(s) => Pin::new(s).poll_write(cx, buf),
            #[cfg(feature = "tls")]
            AnyStream::TcpTls(s) => Pin::new(s).poll_write(cx, buf),
            #[cfg(all(unix, feature = "tls"))]
            AnyStream::UnixTls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            AnyStream::Tcp(s) => Pin::new(s).poll_flush(cx),
            #[cfg(unix)]
            AnyStream::Unix(s) => Pin::new(s).poll_flush(cx),
            #[cfg(feature = "tls")]
            AnyStream::TcpTls(s) => Pin::new(s).poll_flush(cx),
            #[cfg(all(unix, feature = "tls"))]
            AnyStream::UnixTls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            AnyStream::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            #[cfg(unix)]
            AnyStream::Unix(s) => Pin::new(s).poll_shutdown(cx),
            #[cfg(feature = "tls")]
            AnyStream::TcpTls(s) => Pin::new(s).poll_shutdown(cx),
            #[cfg(all(unix, feature = "tls"))]
            AnyStream::UnixTls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

impl IntoLua for AnyStream {
    fn into_lua(self, lua: &Lua) -> Result<Value> {
        match self {
            AnyStream::Tcp(s) => lua.create_userdata(s).map(Value::UserData),
            #[cfg(unix)]
            AnyStream::Unix(s) => lua.create_userdata(s).map(Value::UserData),
            #[cfg(feature = "tls")]
            AnyStream::TcpTls(s) => lua.create_userdata(s).map(Value::UserData),
            #[cfg(all(unix, feature = "tls"))]
            AnyStream::UnixTls(s) => lua.create_userdata(s).map(Value::UserData),
        }
    }
}

impl FromLua for AnyStream {
    fn from_lua(value: Value, lua: &Lua) -> Result<Self> {
        let value = lua.unpack::<AnyUserData>(value)?;
        match value.type_id() {
            Some(id) if id == TypeId::of::<TcpStream>() => {
                let stream = value.take::<TcpStream>()?;
                Ok(AnyStream::Tcp(stream))
            }
            #[cfg(unix)]
            Some(id) if id == TypeId::of::<UnixStream>() => {
                let stream = value.take::<UnixStream>()?;
                Ok(AnyStream::Unix(stream))
            }
            #[cfg(feature = "tls")]
            Some(id) if id == TypeId::of::<TlsStream<TcpStream>>() => {
                let stream = value.take::<TlsStream<TcpStream>>()?;
                Ok(AnyStream::TcpTls(stream))
            }
            #[cfg(all(unix, feature = "tls"))]
            Some(id) if id == TypeId::of::<TlsStream<UnixStream>>() => {
                let stream = value.take::<TlsStream<UnixStream>>()?;
                Ok(AnyStream::UnixTls(stream))
            }
            _ => {
                let type_name = value.type_name().ok().flatten();
                let type_name = type_name.as_deref().unwrap_or("unknown");
                Err(Error::FromLuaConversionError {
                    from: "UserData",
                    to: "AnyStream".to_string(),
                    message: Some(format!("expected TcpStream or UnixStream, got {type_name}",)),
                })
            }
        }
    }
}

/// A listener that can be either TCP or Unix domain socket, possibly wrapped in TLS.
pub enum AnyListener {
    Tcp(TcpListener),
    #[cfg(unix)]
    Unix(UnixListener),
    #[cfg(feature = "tls")]
    TcpTls(TlsListener<TcpListener>),
    #[cfg(all(unix, feature = "tls"))]
    UnixTls(TlsListener<UnixListener>),
}

/// Trait for accepting incoming connections from various listener types.
pub trait Accept {
    type Stream: AsyncRead + AsyncWrite + Unpin + MaybeSend + 'static;

    /// Get the local address that the listener is bound to.
    fn local_addr(&self) -> io::Result<AnySocketAddr>;

    /// Accept an incoming connection.
    #[allow(async_fn_in_trait)]
    async fn accept(&self) -> io::Result<(Self::Stream, AnySocketAddr)>;
}

impl Accept for AnyListener {
    type Stream = AnyStream;

    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        match self {
            AnyListener::Tcp(l) => l.local_addr(),
            #[cfg(unix)]
            AnyListener::Unix(l) => l.local_addr(),
            #[cfg(feature = "tls")]
            AnyListener::TcpTls(l) => l.local_addr(),
            #[cfg(all(unix, feature = "tls"))]
            AnyListener::UnixTls(l) => l.local_addr(),
        }
    }

    async fn accept(&self) -> io::Result<(AnyStream, AnySocketAddr)> {
        match self {
            AnyListener::Tcp(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok((AnyStream::Tcp(stream), addr))
            }
            #[cfg(unix)]
            AnyListener::Unix(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok((AnyStream::Unix(stream), addr))
            }
            #[cfg(feature = "tls")]
            AnyListener::TcpTls(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok((AnyStream::TcpTls(stream), addr))
            }
            #[cfg(all(unix, feature = "tls"))]
            AnyListener::UnixTls(listener) => {
                let (stream, addr) = listener.accept().await?;
                Ok((AnyStream::UnixTls(stream), addr))
            }
        }
    }
}

impl FromLua for AnyListener {
    fn from_lua(value: Value, lua: &Lua) -> Result<Self> {
        let value = lua.unpack::<AnyUserData>(value)?;
        match value.type_id() {
            Some(id) if id == TypeId::of::<TcpListener>() => {
                let listener = value.take::<TcpListener>()?;
                Ok(AnyListener::Tcp(listener))
            }
            #[cfg(unix)]
            Some(id) if id == TypeId::of::<UnixListener>() => {
                let listener = value.take::<UnixListener>()?;
                Ok(AnyListener::Unix(listener))
            }
            #[cfg(feature = "tls")]
            Some(id) if id == TypeId::of::<TlsListener<TcpListener>>() => {
                let listener = value.take::<TlsListener<TcpListener>>()?;
                Ok(AnyListener::TcpTls(listener))
            }
            #[cfg(all(unix, feature = "tls"))]
            Some(id) if id == TypeId::of::<TlsListener<UnixListener>>() => {
                let listener = value.take::<TlsListener<UnixListener>>()?;
                Ok(AnyListener::UnixTls(listener))
            }
            _ => {
                let type_name = value.type_name().ok().flatten();
                let type_name = type_name.as_deref().unwrap_or("unknown");
                Err(Error::FromLuaConversionError {
                    from: "UserData",
                    to: "AnyListener".to_string(),
                    message: Some(format!("expected TcpListener or UnixListener, got {type_name}")),
                })
            }
        }
    }
}
