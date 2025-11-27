use std::io;
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::sync::Arc;

use mlua::{Lua, Result, Table, UserData, UserDataMethods, UserDataRegistry};

use super::UnixStream;
use crate::net::common::{Accept, AnySocketAddr};

pub struct UnixListener {
    pub(crate) listener: tokio::net::UnixListener,
    pub(crate) unlink_on_drop: bool,
}

impl Drop for UnixListener {
    fn drop(&mut self) {
        if self.unlink_on_drop
            && let Ok(addr) = self.listener.local_addr()
            && let Some(path) = addr.as_pathname()
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl Accept for UnixListener {
    type Stream = UnixStream;

    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        self.listener.local_addr().map(AnySocketAddr::Unix)
    }

    async fn accept(&self) -> io::Result<(Self::Stream, AnySocketAddr)> {
        let (stream, addr) = self.listener.accept().await?;
        let io = UnixStream::from(stream);
        let addr = AnySocketAddr::Unix(addr);
        Ok((io, addr))
    }
}

impl UserData for UnixListener {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("local_addr", |_, this, ()| Ok(this.local_addr()?));

        registry.add_async_function("listen", listen);

        registry.add_async_method("accept", |_, this, ()| async move {
            let (stream, _) = lua_try!(this.listener.accept().await);
            Ok(Ok(UnixStream::from(stream)))
        });
    }
}

pub async fn listen(
    _: Lua,
    (path, params): (String, Option<Table>),
) -> Result<StdResult<UnixListener, String>> {
    let path = Arc::new(PathBuf::from(path));

    let path2 = path.clone();
    let res = tokio::task::spawn_blocking(move || {
        // Remove the socket file if it already exists
        if path2.exists()
            && let Err(err) = std::fs::remove_file(&*path2)
        {
            return Err(format!("failed to remove existing socket file: {err}"));
        }
        Ok(())
    })
    .await;
    lua_try!(lua_try!(res));

    // Control whether to remove the socket file on drop or not
    let unlink_on_drop = opt_param!(params, "unlink_on_drop")?.unwrap_or(false);

    let listener = lua_try!(tokio::net::UnixListener::bind(&*path));
    Ok(Ok(UnixListener {
        listener,
        unlink_on_drop,
    }))
}
