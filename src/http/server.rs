use std::future::Future;
use std::mem;
use std::sync::Arc;

use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::conn::auto::Builder as ConnBuilder;
use hyper_util::server::graceful::GracefulShutdown;
use mlua::{
    Error as LuaError, ExternalError, Function, Lua, Result, Table, UserData, UserDataMethods,
    UserDataRegistry,
};
use parking_lot::Mutex;
use tokio::io::{AsyncRead, AsyncWrite};

use super::{LuaRequest, LuaResponse};
use crate::net::common::Accept;
use crate::net::{AnyListener, AnyStream};
use crate::time::Duration;

/// Local executor that can spawn `!Send` futures
#[derive(Clone, Copy)]
struct LocalExec;

impl<F> hyper::rt::Executor<F> for LocalExec
where
    F: Future + 'static,
    F::Output: 'static,
{
    fn execute(&self, fut: F) {
        tokio::task::spawn_local(fut);
    }
}

/// HTTP server that can handle HTTP/1 and HTTP/2 connections
pub struct HttpServer {
    conn: ConnBuilder<LocalExec>,
    graceful: Arc<Mutex<GracefulShutdown>>,
    shutdown_notify: Arc<Mutex<tokio::sync::watch::Sender<()>>>,
}

impl HttpServer {
    pub fn new(params: Option<Table>) -> Result<Self> {
        let mut conn = ConnBuilder::new(LocalExec);

        // http1 params
        let http1 = opt_param!(Table, params, "http1")?;
        if let Some(keep_alive) = opt_param!(http1, "keep_alive")? {
            conn.http1().keep_alive(keep_alive);
        }
        if let Some(preserve_header_case) = opt_param!(http1, "preserve_header_case")? {
            conn.http1().preserve_header_case(preserve_header_case);
        }
        if let Some(max_headers) = opt_param!(http1, "max_headers")? {
            conn.http1().max_headers(max_headers);
        }
        if let Some(header_read_timeout) = opt_param!(Duration, http1, "header_read_timeout")? {
            conn.http1().header_read_timeout(header_read_timeout.0);
        }
        if let Some(max_buf_size) = opt_param!(http1, "max_buf_size")? {
            conn.http1().max_buf_size(max_buf_size);
        }

        // http2 params
        let http2 = opt_param!(Table, params, "http2")?;
        if let Some(initial_stream_window_size) = opt_param!(u32, http2, "initial_stream_window_size")? {
            conn.http2()
                .initial_stream_window_size(initial_stream_window_size);
        }
        if let Some(initial_connection_window_size) =
            opt_param!(u32, params, "initial_connection_window_size")?
        {
            conn.http2()
                .initial_connection_window_size(initial_connection_window_size);
        }
        if let Some(adaptive_window) = opt_param!(http2, "adaptive_window")? {
            conn.http2().adaptive_window(adaptive_window);
        }
        if let Some(max_frame_size) = opt_param!(u32, http2, "max_frame_size")? {
            conn.http2().max_frame_size(max_frame_size);
        }
        if let Some(max_concurrent_streams) = opt_param!(u32, http2, "max_concurrent_streams")? {
            conn.http2().max_concurrent_streams(max_concurrent_streams);
        }
        if let Some(keep_alive_interval) = opt_param!(Duration, http2, "keep_alive_interval")? {
            conn.http2().keep_alive_interval(keep_alive_interval.0);
        }
        if let Some(keep_alive_timeout) = opt_param!(Duration, http2, "keep_alive_timeout")? {
            conn.http2().keep_alive_timeout(keep_alive_timeout.0);
        }
        if let Some(max_header_list_size) = opt_param!(u32, http2, "max_header_list_size")? {
            conn.http2().max_header_list_size(max_header_list_size);
        }
        if let Some(max_send_buf_size) = opt_param!(http2, "max_send_buf_size")? {
            conn.http2().max_send_buf_size(max_send_buf_size);
        }

        conn.http1().timer(TokioTimer::new());
        conn.http2().timer(TokioTimer::new());

        Ok(HttpServer {
            conn,
            graceful: Default::default(),
            shutdown_notify: Default::default(),
        })
    }

    /// Serve a single connection with the given IO stream and handler function
    pub fn serve_connection<I>(
        &self,
        stream: I,
        handler: Function,
    ) -> impl Future<Output = Result<()>> + 'static
    where
        I: AsyncRead + AsyncWrite + Unpin + 'static,
    {
        let service = service_fn(move |req: hyper::Request<Incoming>| {
            let handler = handler.clone();
            async move {
                let req = LuaRequest::from(req);
                let res = handler.call_async::<LuaResponse>(req).await;
                res.map(|resp| resp.into())
            }
        });

        let io = TokioIo::new(stream);
        let conn = self.conn.serve_connection(io, service).into_owned();
        let graceful_conn = self.graceful.lock().watch(conn);
        async move {
            match graceful_conn.await {
                Ok(_) => Ok(()),
                Err(err) => match err.downcast::<LuaError>() {
                    Ok(e) => Err(*e),
                    Err(e) => Err(e.into_lua_err()),
                },
            }
        }
    }

    pub async fn start<L>(&self, listener: L, handler: Function) -> Result<()>
    where
        L: Accept,
    {
        let mut shutdown_rx = self.shutdown_notify.lock().subscribe();
        loop {
            tokio::select! {
                conn = listener.accept() => {
                    let (stream, _remote_addr) = match conn {
                        Ok(conn) => conn,
                        Err(e) => {
                            eprintln!("Failed to accept connection: {}", e);
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            continue;
                        }
                    };

                    let serve_fut = self.serve_connection(stream, handler.clone());
                    tokio::task::spawn_local(serve_fut);
                }

                _ = shutdown_rx.changed() => {
                    break Ok(())
                }
            }
        }
    }

    /// Shutdown the server gracefully, waiting for existing connections to finish
    pub async fn graceful_shutdown(&self, wait: Duration) {
        let shutdown_tx = mem::take(&mut *self.shutdown_notify.lock());
        let _ = shutdown_tx.send(());
        let graceful = mem::take(&mut *self.graceful.lock());
        tokio::select! {
            biased;
            _ = graceful.shutdown() => {},
            _ = tokio::time::sleep(wait.0) => {},
        }
    }
}

impl UserData for HttpServer {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_function("new", |_: &Lua, params| HttpServer::new(params));

        registry.add_async_method(
            "serve_connection",
            |_lua, this, (stream, handler): (AnyStream, Function)| async move {
                this.serve_connection(stream, handler).await
            },
        );

        registry.add_async_method(
            "start",
            |_, this, (listener, handler): (AnyListener, Function)| async move {
                this.start(listener, handler).await
            },
        );

        registry.add_async_method(
            "graceful_shutdown",
            |_lua, this, wait: Option<Duration>| async move {
                let wait = wait.unwrap_or(Duration(std::time::Duration::from_secs(60)));
                this.graceful_shutdown(wait).await;
                Ok(())
            },
        );
    }
}

// /// Create and bind a new HTTP server
// async fn listen(
//     _: Lua,
//     (addr, port, params): (String, Option<u16>, Option<Table>),
// ) -> Result<StdResult<HttpServer, String>> {
//     let port = port.unwrap_or(8080);

//     let addrs = match lookup_host((addr.clone(), port)).await {
//         Ok(addrs) => addrs,
//         Err(e) => return Ok(Err(format!("Failed to resolve address: {}", e))),
//     };

//     let mut last_err = None;
//     for sock_addr in addrs {
//         match TcpListener::bind(sock_addr).await {
//             Ok(listener) => {
//                 let local_addr = match listener.local_addr() {
//                     Ok(addr) => addr,
//                     Err(e) => return Ok(Err(format!("Failed to get local address: {}", e))),
//                 };
//                 return Ok(Ok(HttpServer { listener, local_addr }));
//             }
//             Err(e) => {
//                 last_err = Some(e);
//                 continue;
//             }
//         }
//     }

//     Ok(Err(last_err
//         .map(|err| format!("Failed to bind: {}", err))
//         .unwrap_or_else(|| {
//             "Could not resolve to any address".to_string()
//         })))
// }

// /// Serve HTTP requests with the given handler function using LocalSet for non-Send futures
// async fn serve_impl(lua: &Lua, server: &HttpServer, handler: Function) -> Result<()> {
//     let handler = Rc::new(RefCell::new(handler));
//     let local = tokio::task::LocalSet::new();

//     local
//         .run_until(async {
//             loop {
//                 let (stream, _remote_addr) = match server.listener.accept().await {
//                     Ok(conn) => conn,
//                     Err(e) => {
//                         eprintln!("Failed to accept connection: {}", e);
//                         continue;
//                     }
//                 };

//                 let io = TokioIo::new(stream);
//                 let handler = handler.clone();
//                 let lua = lua.clone();

//                 tokio::task::spawn_local(async move {
//                     let service = service_fn(move |req: HyperRequest<Incoming>| {
//                         let handler = handler.clone();
//                         let lua = lua.clone();
//                         async move { handle_request(&lua, req, &handler).await }
//                     });

//                     let conn_builder = ConnBuilder::new(LocalExec);

//                     if let Err(err) = conn_builder.serve_connection(io, service).await {
//                         eprintln!("Error serving connection: {:?}", err);
//                     }
//                 });
//             }
//         })
//         .await;

//     Ok(())
// }
