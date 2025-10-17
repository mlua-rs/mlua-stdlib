use std::io::Result as IoResult;
use std::net::SocketAddr;
use std::ops::Deref;

use mlua::{Result, Table};

pub(crate) struct TcpSocket(pub(crate) tokio::net::TcpSocket);

impl Deref for TcpSocket {
    type Target = tokio::net::TcpSocket;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub(super) struct SocketOptions {
    keepalive: Option<bool>,
    nodelay: Option<bool>,
    recv_buffer_size: Option<u32>,
    send_buffer_size: Option<u32>,
    reuseraddr: Option<bool>,
    reuseport: Option<bool>,
}

impl TcpSocket {
    pub(crate) fn new_for_addr(addr: SocketAddr) -> IoResult<Self> {
        let sock = match addr {
            SocketAddr::V4(_) => tokio::net::TcpSocket::new_v4()?,
            SocketAddr::V6(_) => tokio::net::TcpSocket::new_v6()?,
        };
        Ok(TcpSocket(sock))
    }

    pub(crate) fn set_options(&self, options: SocketOptions) -> IoResult<()> {
        if let Some(keepalive) = options.keepalive {
            self.set_keepalive(keepalive)?;
        }
        if let Some(nodelay) = options.nodelay {
            self.set_nodelay(nodelay)?;
        }
        if let Some(reuseraddr) = options.reuseraddr {
            self.set_reuseaddr(reuseraddr)?;
        }
        if let Some(reuseport) = options.reuseport {
            self.set_reuseport(reuseport)?;
        }
        if let Some(recv_buffer_size) = options.recv_buffer_size {
            self.set_recv_buffer_size(recv_buffer_size)?;
        }
        if let Some(send_buffer_size) = options.send_buffer_size {
            self.set_send_buffer_size(send_buffer_size)?;
        }
        Ok(())
    }
}

impl SocketOptions {
    pub(crate) fn from_table(params: &Option<Table>) -> Result<Self> {
        Ok(SocketOptions {
            keepalive: opt_param!(params, "keepalive")?,
            nodelay: opt_param!(params, "nodelay")?,
            recv_buffer_size: opt_param!(params, "recv_buffer_size")?,
            send_buffer_size: opt_param!(params, "send_buffer_size")?,
            reuseraddr: opt_param!(params, "reuseaddr")?,
            reuseport: opt_param!(params, "reuseport")?,
        })
    }
}
