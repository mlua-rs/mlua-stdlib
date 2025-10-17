use std::path::PathBuf;
use std::result::Result as StdResult;
use std::sync::Arc;

use mlua::{ExternalResult as _, Lua, Result, Table, UserData, UserDataMethods, UserDataRegistry};

use super::UnixStream;

pub struct UnixListener {
    listener: tokio::net::UnixListener,
    unlink_on_drop: bool,
}

impl Drop for UnixListener {
    fn drop(&mut self) {
        if self.unlink_on_drop {
            if let Ok(addr) = self.listener.local_addr() {
                if let Some(path) = addr.as_pathname() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}

impl UserData for UnixListener {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("local_addr", |_, this, ()| {
            this.listener
                .local_addr()
                .map(|addr| {
                    addr.as_pathname()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|| "(unnamed)".to_string())
                })
                .into_lua_err()
        });

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
        if path2.exists() {
            if let Err(err) = std::fs::remove_file(&*path2) {
                return Err(format!("failed to remove existing socket file: {err}"));
            }
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
