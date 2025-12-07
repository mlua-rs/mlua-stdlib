use std::io;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::result::Result as StdResult;
use std::sync::Arc;

use mlua::{Lua, Result, Table, UserData, UserDataMethods, UserDataRegistry};
use tokio::net::UnixListener;

use super::LuaUnixStream;
use crate::net::common::{Accept, AnySocketAddr};

/// Lua wrapper around tokio [`UnixListener`].
pub struct LuaUnixListener {
    pub(crate) listener: UnixListener,
    pub(crate) unlink_on_drop: bool,
}

impl Deref for LuaUnixListener {
    type Target = UnixListener;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.listener
    }
}

impl DerefMut for LuaUnixListener {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.listener
    }
}

impl Drop for LuaUnixListener {
    fn drop(&mut self) {
        if self.unlink_on_drop
            && let Ok(addr) = self.listener.local_addr()
            && let Some(path) = addr.as_pathname()
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl Accept for LuaUnixListener {
    type Stream = LuaUnixStream;

    fn local_addr(&self) -> io::Result<AnySocketAddr> {
        self.listener.local_addr().map(AnySocketAddr::Unix)
    }

    async fn accept(&self) -> io::Result<(Self::Stream, AnySocketAddr)> {
        let (stream, addr) = self.listener.accept().await?;
        Ok((stream.into(), AnySocketAddr::Unix(addr)))
    }
}

impl UserData for LuaUnixListener {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("local_addr", |_, this, ()| Ok(this.local_addr()?));

        registry.add_async_function("listen", listen);

        registry.add_async_method("accept", |_, this, ()| async move {
            let (stream, _) = lua_try!(this.listener.accept().await);
            Ok(Ok(LuaUnixStream::from(stream)))
        });
    }
}

/// Binds a Unix domain socket listener to the specified path.
///
/// # Arguments
/// * `path`: The file system path to bind the Unix domain socket listener to.
/// * `params` (optional): A table of listener options.
///
/// The following options can be specified:
/// * `unlink_on_drop` (default `false`): Remove the socket file when the listener is dropped.
pub async fn listen(
    _: Lua,
    (path, params): (String, Option<Table>),
) -> Result<StdResult<LuaUnixListener, String>> {
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

    let listener = lua_try!(UnixListener::bind(&*path));
    Ok(Ok(LuaUnixListener {
        listener,
        unlink_on_drop,
    }))
}
