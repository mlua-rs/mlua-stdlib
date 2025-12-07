use std::mem;

use bytes::Bytes;
use http::StatusCode;
use http::response::{Parts, Response};
use http_body_util::{BodyExt, Either as EitherBody};
use hyper_util::client::legacy::connect::HttpInfo;
use mlua::{
    AnyUserData, ExternalResult, FromLua, Lua, MetaMethod, Result, String as LuaString, UserData,
    UserDataMethods, Value,
};

use super::headers::LuaHeaderMapExt;
use crate::http::{LuaBody, LuaHeaders};

/// A Lua wrapper around [`http::Response`].
pub struct LuaResponse {
    pub(crate) head: Parts,
    pub(crate) body: EitherBody<LuaBody, AnyUserData>,
}

impl Default for LuaResponse {
    fn default() -> Self {
        LuaResponse::new(LuaBody::new())
    }
}

impl LuaResponse {
    /// Create a new Response with the given body.
    pub fn new(body: LuaBody) -> Self {
        let head = Response::new(()).into_parts().0;
        let body = EitherBody::Left(body);
        LuaResponse { head, body }
    }
}

impl UserData for LuaResponse {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_method("status", |_, this, ()| Ok(this.head.status.as_u16()));
        registry.add_method_mut("set_status", |_, this, status: u16| {
            this.head.status = StatusCode::from_u16(status).into_lua_err()?;
            Ok(())
        });

        registry.add_method("version", |_, this, ()| Ok(format!("{:?}", this.head.version)));

        registry.add_method("clone_headers", |_, this, ()| {
            Ok(LuaHeaders(this.head.headers.clone()))
        });

        registry.add_method("header", |lua, this, name: LuaString| {
            LuaHeaderMapExt::get(&this.head.headers, lua, &name)
        });

        registry.add_method("header_all", |lua, this, name: LuaString| {
            LuaHeaderMapExt::get_all(&this.head.headers, lua, &name)
        });

        registry.add_method("header_count", |_, this, name: LuaString| {
            LuaHeaderMapExt::get_count(&this.head.headers, &name)
        });

        registry.add_method_mut("set_header", |_, this, (name, value): (LuaString, LuaString)| {
            LuaHeaderMapExt::set(&mut this.head.headers, &name, &value)
        });

        registry.add_method_mut("add_header", |_, this, (name, value): (LuaString, LuaString)| {
            LuaHeaderMapExt::add(&mut this.head.headers, &name, &value)
        });

        registry.add_method_mut("remove_header", |_, this, name: LuaString| {
            LuaHeaderMapExt::remove(&mut this.head.headers, &name)
        });

        registry.add_method("remote_addr", |_, this, ()| {
            Ok((this.head.extensions)
                .get::<HttpInfo>()
                .map(|info| info.remote_addr().to_string()))
        });

        registry.add_method_mut("body", |lua, this, ()| {
            match &mut this.body {
                EitherBody::Left(body) => {
                    // Move the body to Lua
                    let ud_body = lua.create_userdata(mem::take(body))?;
                    this.body = EitherBody::Right(ud_body.clone());
                    Ok(ud_body)
                }
                EitherBody::Right(userdata) => Ok(userdata.clone()),
            }
        });

        registry.add_method_mut("set_body", |_, this, body: LuaBody| {
            this.body = EitherBody::Left(body);
            Ok(())
        });

        registry.add_async_method_mut("text", |lua, mut this, ()| async move {
            let body = match &mut this.body {
                EitherBody::Left(b) => b,
                EitherBody::Right(ud) => &mut *ud.borrow_mut::<LuaBody>()?,
            };
            let body = lua_try!(body.collect().await);
            Ok(Ok(lua.create_string(body.to_bytes())?))
        });

        registry.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            let mut buf = String::with_capacity(1024);
            let (version, status) = (this.head.version, this.head.status);
            let reason = status.canonical_reason().unwrap_or("");
            buf.push_str(&format!("{version:?} {status} {reason}\n"));
            // Iterate headers
            for (name, value) in &this.head.headers {
                let value = String::from_utf8_lossy(value.as_bytes());
                buf.push_str(&format!("{name}: {value}\n"));
            }
            Ok(buf)
        });
    }
}

impl From<LuaResponse> for Response<LuaBody> {
    fn from(lua_response: LuaResponse) -> Self {
        let LuaResponse { head, body } = lua_response;
        let body = match body {
            EitherBody::Left(b) => b,
            EitherBody::Right(ud) => ud.take::<LuaBody>().expect("Body userdata has wrong type"),
        };
        Response::from_parts(head, body)
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Response> for LuaResponse {
    fn from(resp: reqwest::Response) -> Self {
        let hyper_resp: hyper::Response<_> = resp.into();
        let (head, body) = hyper_resp.into_parts();
        LuaResponse {
            head,
            body: EitherBody::Left(LuaBody::from(body)),
        }
    }
}

impl FromLua for LuaResponse {
    fn from_lua(value: Value, _lua: &Lua) -> Result<Self> {
        match value {
            // Value::Table(t) => {
            //     let status: u16 = t.get::<u16>("status").unwrap_or(200);
            //     let body: Option<LuaString> = t.get("body")?;
            //     let headers: Option<Table> = t.get("headers")?;

            //     let mut builder = http::Response::builder().status(status);

            //     // Add headers if provided
            //     if let Some(headers_table) = headers {
            //         for pair in headers_table.pairs::<String, LuaString>() {
            //             let (name, value) = pair?;
            //             let header_name = HeaderName::from_bytes(name.as_bytes()).into_lua_err()?;
            //             let header_value =
            //                 HeaderValue::from_bytes(value.as_bytes().as_ref()).into_lua_err()?;
            //             builder = builder.header(header_name, header_value);
            //         }
            //     }

            //     // Set body
            //     let response_body = if let Some(body_str) = body {
            //         Body::Bytes(bytes::Bytes::copy_from_slice(body_str.as_bytes().as_ref()))
            //     } else {
            //         Body::Bytes(bytes::Bytes::new())
            //     };

            //     let response = builder.body(response_body).into_lua_err()?;
            //     Ok(Response(response))
            // }
            Value::String(s) => {
                let bytes = Bytes::copy_from_slice(&s.as_bytes());
                Ok(LuaResponse::new(LuaBody::from(bytes)))
            }
            // Value::UserData(ud) => {
            //     if let Ok(response) = ud.borrow::<Response>() {
            //         let status = response.0.status().as_u16();
            //         let mut builder = http::Response::builder().status(status);

            //         // Copy headers
            //         for (name, value) in response.0.headers() {
            //             builder = builder.header(name.clone(), value.clone());
            //         }

            //         let response = builder.body(Body::Bytes(bytes::Bytes::new())).into_lua_err()?;
            //         Ok(Response(response))
            //     } else {
            //         Err(mlua::Error::FromLuaConversionError {
            //             from: "UserData",
            //             to: "Response".to_string(),
            //             message: Some("expected Response userdata".to_string()),
            //         })
            //     }
            // }
            _ => Err(mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Response".to_string(),
                message: Some("expected table or Response userdata".to_string()),
            }),
        }
    }
}
