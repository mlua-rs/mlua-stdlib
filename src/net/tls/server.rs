//! TLS server functionality - accepting TLS connections.

use std::any::TypeId;
use std::io;
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::sync::Arc;

use mlua::{AnyUserData, Lua, Result as LuaResult, Table, UserData, UserDataMethods, UserDataRegistry};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{RootCertStore, ServerConfig};

use super::stream::LuaTlsStream;
use crate::net::common::{Accept, AddressProvider, AnySocketAddr};
use crate::net::tcp::{LuaTcpListener, LuaTcpStream};
#[cfg(unix)]
use crate::net::unix::{LuaUnixListener, LuaUnixStream};

/// TLS configuration options for server connections.
#[derive(Debug, Clone)]
pub struct TlsServerOptions {
    /// Server certificate chain
    pub cert_chain: PathBuf,
    /// Server private key
    pub private_key: PathBuf,
    /// Client certificate verification
    pub verify_client: bool,
    /// CA certificates for client verification
    pub client_ca_certs: Vec<PathBuf>,
    /// ALPN protocols to advertise (e.g., ["h2", "http/1.1"])
    pub alpn_protocols: Vec<String>,
}

/// Common configuration for a set of server sessions.
#[derive(Debug, Clone, mlua::FromLua)]
pub struct TlsServerConfig(pub Arc<ServerConfig>);

impl TlsServerOptions {
    /// Create a new TLS server configuration from a Lua table.
    pub fn from_table(params: &Table) -> LuaResult<Self> {
        Ok(Self {
            cert_chain: param!(params, "cert_chain")?,
            private_key: param!(params, "private_key")?,
            verify_client: opt_param!(Some(params), "verify_client")?.unwrap_or(false),
            client_ca_certs: opt_param!(Some(params), "client_ca_certs")?.unwrap_or_default(),
            alpn_protocols: opt_param!(Some(params), "alpn_protocols")?.unwrap_or_default(),
        })
    }

    /// Build a `TlsServerConfig` from this configuration options.
    ///
    /// This operation may be expensive due to file I/O.
    /// Errors if the certificate or key files cannot be read or parsed.
    pub fn build(self) -> io::Result<TlsServerConfig> {
        let cert_data = std::fs::read(self.cert_chain)?;
        let certs: Vec<CertificateDer> = rustls_pemfile::certs(&mut cert_data.as_slice())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let key_data = std::fs::read(self.private_key)?;
        let key: PrivateKeyDer = rustls_pemfile::private_key(&mut key_data.as_slice())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no private key found"))?;

        let mut config = if self.verify_client {
            let mut root_store = RootCertStore::empty();
            for ca_path in self.client_ca_certs {
                let ca_data = std::fs::read(ca_path)?;
                let ca_certs = rustls_pemfile::certs(&mut ca_data.as_slice())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                for cert in ca_certs {
                    root_store.add(cert).map_err(|e| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("invalid cert: {e}"))
                    })?;
                }
            }

            let client_verifier = rustls::server::WebPkiClientVerifier::builder(root_store.into())
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            ServerConfig::builder()
                .with_client_cert_verifier(client_verifier)
                .with_single_cert(certs, key)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        } else {
            ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        };

        // Configure ALPN protocols if provided
        if !self.alpn_protocols.is_empty() {
            config.alpn_protocols = self.alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
        }

        Ok(TlsServerConfig(Arc::new(config)))
    }
}

impl UserData for TlsServerConfig {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_function("new", |_, params: Table| {
            let options = TlsServerOptions::from_table(&params)?;
            Ok(Ok(lua_try!(options.build())))
        });
    }
}

/// Wrap a stream with TLS as a server (accepts incoming TLS connection).
pub async fn wrap_accept_stream(
    lua: Lua,
    (stream, config): (AnyUserData, TlsServerConfig),
) -> LuaResult<StdResult<AnyUserData, String>> {
    match stream.type_id() {
        Some(type_id) if type_id == TypeId::of::<LuaTcpStream>() => {
            #[rustfmt::skip]
            let LuaTcpStream { stream, read_timeout, write_timeout, .. } = stream.take()?;
            match LuaTlsStream::new_server(stream, config).await {
                Ok(mut tls_stream) => {
                    tls_stream.set_read_timeout(read_timeout);
                    tls_stream.set_write_timeout(write_timeout);
                    Ok(Ok(lua.create_userdata(tls_stream)?))
                }
                Err(e) => Ok(Err(e.to_string())),
            }
        }
        #[cfg(unix)]
        Some(type_id) if type_id == TypeId::of::<LuaUnixStream>() => {
            #[rustfmt::skip]
            let LuaUnixStream { stream, read_timeout, write_timeout } = stream.take()?;
            match LuaTlsStream::new_server(stream, config).await {
                Ok(mut tls_stream) => {
                    tls_stream.set_read_timeout(read_timeout);
                    tls_stream.set_write_timeout(write_timeout);
                    Ok(Ok(lua.create_userdata(tls_stream)?))
                }
                Err(e) => Ok(Err(e.to_string())),
            }
        }
        _ => Ok(Err("unsupported stream type".to_string())),
    }
}

/// Generic TLS listener wrapper.
///
/// Wraps any listener type and automatically upgrades accepted connections to TLS.
pub struct LuaTlsListener<L> {
    inner: L,
    config: TlsServerConfig,
}

impl<L> LuaTlsListener<L> {
    /// Create a new TLS listener wrapping the given listener with the specified TLS configuration.
    pub fn new(inner: L, config: TlsServerConfig) -> Self {
        Self { inner, config }
    }

    /// Get a reference to the underlying listener
    pub fn get_ref(&self) -> &L {
        &self.inner
    }

    /// Get a mutable reference to the underlying listener
    pub fn get_mut(&mut self) -> &mut L {
        &mut self.inner
    }
}

impl<L> Accept for LuaTlsListener<L>
where
    L: Accept + 'static,
    L::Stream: AddressProvider,
{
    type Stream = LuaTlsStream<L::Stream>;

    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        self.inner.local_addr()
    }

    async fn accept(&self) -> io::Result<(Self::Stream, AnySocketAddr)> {
        let (stream, addr) = self.inner.accept().await?;
        let tls_stream = LuaTlsStream::new_server(stream, self.config.clone()).await?;
        Ok((tls_stream, addr))
    }
}

impl<L> UserData for LuaTlsListener<L>
where
    L: Accept + 'static,
    L::Stream: AddressProvider,
{
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("local_addr", |_, this, ()| Ok(this.inner.local_addr()?));

        registry.add_async_method("accept", |lua, this, ()| async move {
            // Accept the plain TCP stream from the inner tokio listener
            let (stream, _) = lua_try!(this.inner.accept().await);
            // Upgrade to TLS
            let tls_stream = lua_try!(LuaTlsStream::new_server(stream, this.config.clone()).await);
            Ok(Ok(lua.create_userdata(tls_stream)?))
        });
    }
}

/// Wrap a listener with TLS (server-side).
pub fn wrap_listener(
    lua: &Lua,
    (listener, config): (AnyUserData, TlsServerConfig),
) -> LuaResult<StdResult<AnyUserData, String>> {
    match listener.type_id() {
        Some(type_id) if type_id == TypeId::of::<LuaTcpListener>() => {
            let tcp_listener = listener.take::<LuaTcpListener>()?;
            let tls_listener = LuaTlsListener::new(tcp_listener, config);
            Ok(Ok(lua.create_userdata(tls_listener)?))
        }
        #[cfg(unix)]
        Some(type_id) if type_id == TypeId::of::<LuaUnixListener>() => {
            let unix_listener = listener.take::<LuaUnixListener>()?;
            let tls_listener = LuaTlsListener::new(unix_listener, config);
            Ok(Ok(lua.create_userdata(tls_listener)?))
        }
        _ => Ok(Err("unsupported listener type".to_string())),
    }
}
