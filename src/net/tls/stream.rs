use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use mlua::{MaybeSend, String as LuaString, UserData, UserDataMethods, UserDataRegistry};
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};

use super::client::TlsClientConfig;
use super::server::TlsServerConfig;
use crate::net::AddressProvider;
use crate::time::LuaDuration;

/// A TLS stream wrapper for Lua.
///
/// This type wraps a TLS stream and provides read/write methods.
/// It consumes the underlying stream to prevent further use of the plain stream.
pub struct LuaTlsStream<S> {
    inner: TlsStream<S>,
    read_timeout: Option<LuaDuration>,
    write_timeout: Option<LuaDuration>,
}

impl<S> LuaTlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    /// Create a new TLS client stream from the plain stream.
    pub async fn new_client(
        stream: S,
        domain: ServerName<'static>,
        config: TlsClientConfig,
    ) -> io::Result<Self> {
        let tls_stream = TlsConnector::from(config.0)
            .connect(domain, stream)
            .await
            .map(TlsStream::Client)?;

        Ok(LuaTlsStream {
            inner: tls_stream,
            read_timeout: None,
            write_timeout: None,
        })
    }

    /// Create a new TLS server stream from the plain stream.
    pub async fn new_server(stream: S, config: TlsServerConfig) -> io::Result<Self> {
        let tls_stream = TlsAcceptor::from(config.0)
            .accept(stream)
            .await
            .map(TlsStream::Server)?;

        Ok(LuaTlsStream {
            inner: tls_stream,
            read_timeout: None,
            write_timeout: None,
        })
    }

    pub(crate) fn set_read_timeout(&mut self, dur: Option<LuaDuration>) {
        self.read_timeout = dur;
    }

    pub(crate) fn set_write_timeout(&mut self, dur: Option<LuaDuration>) {
        self.write_timeout = dur;
    }

    /// Get a reference to the underlying stream
    pub fn get_ref(&self) -> &S {
        self.inner.get_ref().0
    }

    /// Get a mutable reference to the underlying stream
    pub fn get_mut(&mut self) -> &mut S {
        self.inner.get_mut().0
    }
}

impl<S> AsyncRead for LuaTlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for LuaTlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, buf)
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

impl<S> AddressProvider for LuaTlsStream<S>
where
    S: AsyncRead + AsyncWrite + AddressProvider + Unpin,
{
    fn local_addr(&self) -> io::Result<crate::net::AnySocketAddr> {
        self.get_ref().local_addr()
    }

    fn peer_addr(&self) -> io::Result<crate::net::AnySocketAddr> {
        self.get_ref().peer_addr()
    }
}

impl<S> UserData for LuaTlsStream<S>
where
    S: AsyncRead + AsyncWrite + AddressProvider + Unpin + MaybeSend + 'static,
{
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("local_addr", |_, this, ()| Ok(this.local_addr()?));
        registry.add_method("peer_addr", |_, this, ()| Ok(this.peer_addr()?));

        registry.add_method_mut("set_read_timeout", |_, this, dur: Option<LuaDuration>| {
            this.read_timeout = dur;
            Ok(())
        });

        registry.add_method_mut("set_write_timeout", |_, this, dur: Option<LuaDuration>| {
            this.write_timeout = dur;
            Ok(())
        });

        registry.add_async_method_mut("read", |lua, mut this, size: usize| async move {
            let mut buf = vec![0; size];
            let read_timeout = this.read_timeout;
            let n = with_io_timeout!(read_timeout, this.inner.read(&mut buf));
            let n = lua_try!(n);
            buf.truncate(n);
            Ok(Ok(lua.create_string(buf)?))
        });

        registry.add_async_method_mut("read_to_end", |lua, mut this, ()| async move {
            let mut buf = Vec::new();
            let read_timeout = this.read_timeout;
            let n = with_io_timeout!(read_timeout, this.inner.read_to_end(&mut buf));
            let _n = lua_try!(n);
            Ok(Ok(lua.create_string(buf)?))
        });

        registry.add_async_method_mut("write", |_, mut this, data: LuaString| async move {
            let write_timeout = this.write_timeout;
            let n = with_io_timeout!(write_timeout, this.inner.write(&data.as_bytes()));
            let n = lua_try!(n);
            Ok(Ok(n))
        });

        registry.add_async_method_mut("write_all", |_, mut this, data: LuaString| async move {
            let write_timeout = this.write_timeout;
            let r = with_io_timeout!(write_timeout, this.inner.write_all(&data.as_bytes()));
            lua_try!(r);
            Ok(Ok(true))
        });

        registry.add_async_method_mut("flush", |_, mut this, ()| async move {
            let write_timeout = this.write_timeout;
            let r = with_io_timeout!(write_timeout, this.inner.flush());
            lua_try!(r);
            Ok(Ok(true))
        });

        registry.add_async_method_mut("send_close_notify", |_, mut this, ()| async move {
            this.inner.get_mut().1.send_close_notify();
            _ = this.inner.flush().await;
            Ok(())
        });

        registry.add_async_method_mut("shutdown", |_, mut this, ()| async move {
            lua_try!(this.get_mut().shutdown().await);
            Ok(Ok(true))
        });

        registry.add_method("connection_info", |lua, this, ()| {
            let conn_data = this.inner.get_ref().1;
            let info = lua.create_table()?;

            // Protocol version
            if let Some(version) = conn_data.protocol_version() {
                info.set("version", version.as_str())?;
            }
            // Negotiated cipher suite
            if let Some(cipher_suite) = conn_data.negotiated_cipher_suite() {
                info.set("cipher_suite", cipher_suite.suite().as_str())?;
            }
            // ALPN protocol
            if let Some(alpn) = conn_data.alpn_protocol() {
                info.set("alpn_protocol", lua.create_string(alpn)?)?;
            }

            Ok(info)
        });
    }
}
