use std::any::TypeId;
use std::mem;
use std::pin::{Pin, pin};
use std::task::{Context, Poll, ready};

use bytes::{Buf, Bytes};
use futures_util::stream::StreamExt;
use http_body_util::BodyExt as _;
use hyper::body::{Body as HttpBody, Frame, Incoming, SizeHint};
use mlua::{
    Error, ExternalError, FromLua, Lua, Result as LuaResult, UserData, UserDataMethods, UserDataRegistry,
    Value,
};

/// A Lua-accessible HTTP body
///
/// This can wrap various body types, including raw bytes, Hyper incoming bodies,
/// and Reqwest bodies (if the `reqwest` feature is enabled).
pub struct LuaBody(Inner);

enum Inner {
    Bytes(Bytes),
    Incoming {
        incoming: Incoming,
        // If Some, the maximum number of bytes allowed to be read from the body
        remaining: Option<usize>,
    },
    #[cfg(feature = "reqwest")]
    Reqwest {
        body: reqwest::Body,
        // If Some, the maximum number of bytes allowed to be read from the body
        remaining: Option<usize>,
    },
}

impl Default for LuaBody {
    #[inline]
    fn default() -> Self {
        LuaBody::new()
    }
}

impl LuaBody {
    pub const fn new() -> Self {
        LuaBody(Inner::Bytes(Bytes::new()))
    }

    async fn buffer(&mut self) -> Result<(), Error> {
        match self {
            LuaBody(Inner::Bytes(_)) => Ok(()),
            _ => {
                let collect = self.collect().await?;
                *self = LuaBody::from(collect.to_bytes());
                Ok(())
            }
        }
    }

    fn consume_if_unbuffered(&mut self) -> Self {
        match self {
            LuaBody(Inner::Bytes(bytes)) => LuaBody(Inner::Bytes(bytes.clone())),
            _ => mem::take(self),
        }
    }
}

impl From<Bytes> for LuaBody {
    fn from(bytes: Bytes) -> Self {
        LuaBody(Inner::Bytes(bytes))
    }
}

impl From<Incoming> for LuaBody {
    fn from(incoming: Incoming) -> Self {
        LuaBody(Inner::Incoming {
            incoming,
            remaining: None,
        })
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Body> for LuaBody {
    fn from(body: reqwest::Body) -> Self {
        LuaBody(Inner::Reqwest {
            body,
            remaining: None,
        })
    }
}

#[cfg(feature = "reqwest")]
impl From<LuaBody> for reqwest::Body {
    fn from(body: LuaBody) -> Self {
        match body {
            LuaBody(Inner::Bytes(bytes)) => reqwest::Body::from(bytes),
            LuaBody(Inner::Incoming { incoming, .. }) => reqwest::Body::wrap(incoming),
            #[cfg(feature = "reqwest")]
            LuaBody(Inner::Reqwest { body, .. }) => body,
        }
    }
}

impl FromLua for LuaBody {
    fn from_lua(value: Value, _: &Lua) -> LuaResult<Self> {
        match value {
            Value::String(s) => Ok(LuaBody::from(Bytes::copy_from_slice(&s.as_bytes()))),
            Value::UserData(ud) => match ud.type_id() {
                Some(id) if id == TypeId::of::<Bytes>() => Ok(LuaBody::from(ud.borrow::<Bytes>()?.clone())),
                Some(id) if id == TypeId::of::<LuaBody>() => ud.take::<LuaBody>(),
                _ => Err(mlua::Error::FromLuaConversionError {
                    from: "UserData",
                    to: "Body".to_string(),
                    message: Some("expected Bytes or Body userdata".to_string()),
                }),
            },
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Body".to_string(),
                message: Some("expected String".to_string()),
            }),
        }
    }
}

impl HttpBody for LuaBody {
    type Data = Bytes;
    type Error = Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        fn process_frame(frame: Frame<Bytes>, remaining: &mut Option<usize>) -> Result<Frame<Bytes>, Error> {
            if let (Some(data), Some(remaining)) = (frame.data_ref(), remaining.as_mut()) {
                if data.remaining() > *remaining {
                    *remaining = 0;
                    Err("body size limit exceeded".into_lua_err())
                } else {
                    *remaining -= data.remaining();
                    Ok(frame)
                }
            } else {
                Ok(frame)
            }
        }

        let this = self.get_mut();
        match &mut this.0 {
            Inner::Bytes(bytes) if bytes.is_empty() => Poll::Ready(None),
            Inner::Bytes(bytes) => {
                let chunk = mem::take(bytes);
                Poll::Ready(Some(Ok(Frame::data(chunk))))
            }
            Inner::Incoming { incoming, remaining } => match ready!(pin!(incoming).poll_frame(cx)) {
                Some(Ok(frame)) => Poll::Ready(Some(process_frame(frame, remaining))),
                Some(Err(e)) => Poll::Ready(Some(Err(e.into_lua_err()))),
                None => Poll::Ready(None),
            },
            #[cfg(feature = "reqwest")]
            Inner::Reqwest { body, remaining } => match ready!(pin!(body).poll_frame(cx)) {
                Some(Ok(frame)) => Poll::Ready(Some(process_frame(frame, remaining))),
                Some(Err(e)) => Poll::Ready(Some(Err(e.into_lua_err()))),
                None => Poll::Ready(None),
            },
        }
    }

    fn is_end_stream(&self) -> bool {
        match &self.0 {
            Inner::Bytes(bytes) => bytes.is_empty(),
            Inner::Incoming { incoming, .. } => incoming.is_end_stream(),
            #[cfg(feature = "reqwest")]
            Inner::Reqwest { body, .. } => body.is_end_stream(),
        }
    }

    fn size_hint(&self) -> SizeHint {
        match &self.0 {
            Inner::Bytes(bytes) => SizeHint::with_exact(bytes.len() as u64),
            Inner::Incoming { incoming, .. } => incoming.size_hint(),
            #[cfg(feature = "reqwest")]
            Inner::Reqwest { body, .. } => body.size_hint(),
        }
    }
}

impl UserData for LuaBody {
    fn register(registry: &mut UserDataRegistry<Self>) {
        // Get the (upper) size hint for the body
        registry.add_method("size_hint", |_, this, ()| {
            let hint = this.size_hint();
            match hint.upper() {
                Some(upper) => Ok(Some(upper)),
                None => Ok(None),
            }
        });

        // Set a size limit for reading the body from an incoming stream
        registry.add_method_mut("set_size_limit", |_, this, limit| match &mut this.0 {
            Inner::Bytes(_) => Ok(()),
            Inner::Incoming { remaining, .. } => {
                *remaining = Some(limit);
                Ok(())
            }
            #[cfg(feature = "reqwest")]
            Inner::Reqwest { remaining, .. } => {
                *remaining = Some(limit);
                Ok(())
            }
        });

        // Buffer the body fully into memory
        registry.add_async_method_mut("buffer", |_, mut this, ()| async move {
            lua_try!(this.buffer().await);
            Ok(Ok(()))
        });

        // Discard the body without reading it
        registry.add_method_mut("discard", |_, this, ()| {
            *this = LuaBody(Inner::Bytes(Bytes::new()));
            Ok(())
        });

        // Read the full body as bytes
        //
        // Consumes the body if it is not buffered
        registry.add_async_method_mut("read", |lua, mut this, ()| async move {
            let body = this.consume_if_unbuffered();
            let body = lua_try!(body.collect().await);
            let bytes = body.to_bytes();
            Ok(Ok(lua.create_any_userdata(bytes)?))
        });

        // Get an async reader for the body
        //
        // Consumes the body if it is not buffered, returns a function that can be
        // called to get the next chunk of data
        registry.add_method_mut("reader", |lua, this, ()| {
            use std::cell::RefCell;
            use std::rc::Rc;

            let body_stream = this.consume_if_unbuffered().into_data_stream();
            let body_stream = Rc::new(RefCell::new(body_stream));
            lua.create_async_function(move |lua, ()| {
                let body_stream = body_stream.clone();
                #[allow(clippy::await_holding_refcell_ref)]
                async move {
                    let mut body_stream = lua_try!(body_stream.try_borrow_mut());
                    match body_stream.next().await {
                        Some(Ok(data)) => {
                            let data = lua.create_any_userdata(data)?;
                            Ok(Ok(Value::UserData(data)))
                        }
                        Some(Err(e)) => Ok(Err(e.to_string())),
                        None => Ok(Ok(Value::Nil)),
                    }
                }
            })
        });

        // Read the full body as text
        //
        // Consumes the body if it is not buffered
        registry.add_async_method_mut("text", |lua, mut this, ()| async move {
            let body = this.consume_if_unbuffered();
            let body = lua_try!(body.collect().await);
            let text = lua.create_string(body.to_bytes())?;
            Ok(Ok(text))
        });

        // Read the full body as JSON
        //
        // Consumes the body if it is not buffered
        #[cfg(feature = "json")]
        registry.add_async_method_mut("json", |_, mut this, ()| async move {
            let body = this.consume_if_unbuffered();
            let body = lua_try!(body.collect().await);
            let body_reader = body.aggregate().reader();
            let json = lua_try!(serde_json::from_reader::<_, serde_json::Value>(body_reader));
            Ok(Ok(crate::json::JsonObject::from(json)))
        });
    }
}
