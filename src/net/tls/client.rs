//! TLS client functionality - initiating TLS connections.

use std::any::TypeId;
use std::io;
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::sync::{Arc, LazyLock};

use mlua::{AnyUserData, Lua, Result as LuaResult, Table, UserData, UserDataMethods, UserDataRegistry};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, DnsName, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};

use super::stream::TlsStream;
#[cfg(unix)]
use crate::net::unix::UnixStream;
use crate::net::{AnyStream, TcpStream};

/// TLS configuration options for client connections
#[derive(Debug, Clone)]
pub struct TlsClientOptions {
    /// Use native certificate store
    pub use_native_certs: bool,
    /// Use webpki roots (Mozilla's root certificates)
    pub use_webpki_roots: bool,
    /// Additional CA certificates to trust
    pub ca_certs: Vec<PathBuf>,
    /// Client certificate for mutual TLS
    pub client_cert: Option<PathBuf>,
    /// Client private key for mutual TLS
    pub client_key: Option<PathBuf>,
    /// Whether to verify the server certificate
    pub dangerous_verify_certs: bool,
    /// ALPN protocols to advertise (e.g., ["h2", "http/1.1"])
    pub alpn_protocols: Vec<String>,
}

fn default_server_name() -> ServerName<'static> {
    ServerName::DnsName(const { DnsName::try_from_str("localhost") }.unwrap())
}

/// A certificate verifier that accepts all certificates without verification.
/// This is insecure and should only be used for testing/development.
#[derive(Debug)]
struct NoVerifier;

impl ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

/// Common configuration for (typically) all connections made by a program.
#[derive(Debug, Clone, mlua::FromLua)]
pub struct TlsClientConfig(pub Arc<ClientConfig>);

impl Default for TlsClientOptions {
    fn default() -> Self {
        Self {
            use_native_certs: true,
            use_webpki_roots: true,
            ca_certs: Vec::new(),
            client_cert: None,
            client_key: None,
            dangerous_verify_certs: true,
            alpn_protocols: Vec::new(),
        }
    }
}

impl TlsClientOptions {
    /// Create a new TLS client configuration from a Lua table.
    pub fn from_table(params: &Option<Table>) -> LuaResult<Self> {
        let mut config = Self::default();

        if let Some(use_native_certs) = opt_param!(params, "use_native_certs")? {
            config.use_native_certs = use_native_certs;
        }
        if let Some(use_webpki_roots) = opt_param!(params, "use_webpki_roots")? {
            config.use_webpki_roots = use_webpki_roots;
        }
        config.ca_certs = opt_param!(params, "ca_certs")?.unwrap_or_default();
        config.client_cert = opt_param!(params, "client_cert")?;
        config.client_key = opt_param!(params, "client_key")?;
        if let Some(verify_certs) = opt_param!(params, "dangerous_verify_certs")? {
            config.dangerous_verify_certs = verify_certs;
        }
        config.alpn_protocols = opt_param!(params, "alpn_protocols")?.unwrap_or_default();

        Ok(config)
    }

    /// Build a TlsClientConfig from this configuration options.
    ///
    /// This operation may be expensive due to file I/O.
    /// Errors if any of the certificate files cannot be read or parsed.
    pub fn build(self) -> io::Result<TlsClientConfig> {
        // If certificate verification is disabled, use a dangerous no-verification config
        if !self.dangerous_verify_certs {
            let config = ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(NoVerifier))
                .with_no_client_auth();
            return Ok(TlsClientConfig(Arc::new(config)));
        }

        let mut root_store = RootCertStore::empty();

        // Add native certificates
        if self.use_native_certs {
            let certs = rustls_native_certs::load_native_certs();
            if let Some(err) = certs.errors.first() {
                return Err(io::Error::other(err.to_string()));
            }
            for cert in certs.certs {
                root_store
                    .add(cert)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("invalid cert: {e}")))?;
            }
        }

        // Add webpki roots
        if self.use_webpki_roots {
            root_store.extend(webpki_roots::TLS_SERVER_ROOTS.to_vec());
        }

        // Add custom CA certificates
        for ca_path in self.ca_certs {
            let ca_data = std::fs::read(ca_path)?;
            let certs = rustls_pemfile::certs(&mut ca_data.as_slice())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            for cert in certs {
                root_store.add(cert).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("invalid cert: {}", e))
                })?;
            }
        }

        let config_builder = ClientConfig::builder().with_root_certificates(root_store);

        // Configure client certificate if provided
        let mut config = if let (Some(cert_path), Some(key_path)) = (self.client_cert, self.client_key) {
            let cert_data = std::fs::read(cert_path)?;
            let certs = rustls_pemfile::certs(&mut cert_data.as_slice())
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            let key_data = std::fs::read(key_path)?;
            let key = rustls_pemfile::private_key(&mut key_data.as_slice())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no private key found"))?;

            config_builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        } else {
            config_builder.with_no_client_auth()
        };

        // Configure ALPN protocols if provided
        if !self.alpn_protocols.is_empty() {
            config.alpn_protocols = self.alpn_protocols.into_iter().map(|s| s.into_bytes()).collect();
        }

        Ok(TlsClientConfig(Arc::new(config)))
    }
}

impl UserData for TlsClientConfig {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_function("new", |_, params: Option<Table>| {
            let options = TlsClientOptions::from_table(&params)?;
            Ok(Ok(lua_try!(options.build())))
        });
    }
}

static DEFAULT_TLS_CLIENT_CONFIG: LazyLock<TlsClientConfig> =
    LazyLock::new(|| TlsClientOptions::default().build().unwrap());

/// Wrap a stream with TLS (client-side).
pub async fn wrap_stream(
    _lua: Lua,
    (stream, server_name, config): (AnyUserData, Option<String>, Option<TlsClientConfig>),
) -> LuaResult<StdResult<AnyStream, String>> {
    let server_name = server_name.and_then(|name| ServerName::try_from(name).ok());
    let config = config.unwrap_or_else(|| DEFAULT_TLS_CLIENT_CONFIG.clone());

    match stream.type_id() {
        Some(type_id) if type_id == TypeId::of::<TcpStream>() => {
            #[rustfmt::skip]
            let TcpStream { stream, host, read_timeout, write_timeout } = stream.take::<TcpStream>()?;
            let server_name = server_name
                .or_else(|| host.and_then(|host| ServerName::try_from(host).ok()))
                .or_else(|| stream.peer_addr().map(|addr| ServerName::from(addr.ip())).ok())
                .unwrap_or_else(default_server_name);
            match TlsStream::new_client(stream.into(), server_name, config).await {
                Ok(mut tls_stream) => {
                    tls_stream.set_read_timeout(read_timeout);
                    tls_stream.set_write_timeout(write_timeout);
                    Ok(Ok(AnyStream::TcpTls(tls_stream)))
                }
                Err(e) => Ok(Err(e.to_string())),
            }
        }
        #[cfg(unix)]
        Some(type_id) if type_id == TypeId::of::<UnixStream>() => {
            #[rustfmt::skip]
            let UnixStream { stream, read_timeout, write_timeout } = stream.take::<UnixStream>()?;
            let server_name = server_name.unwrap_or_else(default_server_name);
            match TlsStream::new_client(stream.into(), server_name, config).await {
                Ok(mut tls_stream) => {
                    tls_stream.set_read_timeout(read_timeout);
                    tls_stream.set_write_timeout(write_timeout);
                    Ok(Ok(AnyStream::UnixTls(tls_stream)))
                }
                Err(e) => Ok(Err(e.to_string())),
            }
        }
        _ => Ok(Err("unsupported stream type".to_string())),
    }
}
