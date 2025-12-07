use std::mem;

use http::request::{Parts, Request};
use http_body_util::Either as EitherBody;
use hyper::body::Incoming;
use mlua::{
    AnyUserData, Error, FromLua, Lua, MetaMethod, Result, String as LuaString, Table, UserData,
    UserDataMethods, Value,
};

use crate::http::{LuaBody, LuaHeaderMapExt, LuaHeaders, LuaMethod};
use crate::time::Duration;

/// A Lua wrapper around [`http::Request`].
pub struct LuaRequest {
    pub(crate) head: Parts,
    pub(crate) body: EitherBody<LuaBody, AnyUserData>,
}

impl Default for LuaRequest {
    fn default() -> Self {
        let req = Request::new(());
        let (head, _) = req.into_parts();
        LuaRequest {
            head,
            body: EitherBody::Left(LuaBody::new()),
        }
    }
}

impl LuaRequest {
    /// Consumes the LuaRequest and returns its parts.
    pub fn into_parts(self) -> (Parts, LuaBody) {
        let LuaRequest { head, body, .. } = self;
        let body = match body {
            EitherBody::Left(body) => body,
            EitherBody::Right(ud) => ud.take::<LuaBody>().expect("Body userdata has wrong type"),
        };
        (head, body)
    }

    #[allow(unused)]
    pub(crate) fn params(&self) -> RequestParams {
        (self.head.extensions)
            .get::<RequestParams>()
            .cloned()
            .unwrap_or_default()
    }
}

impl UserData for LuaRequest {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_method("method", |lua, this, ()| {
            lua.create_string(this.head.method.as_str())
        });

        registry.add_method("uri", |_, this, ()| Ok(this.head.uri.to_string()));

        registry.add_method("version", |_, this, ()| Ok(format!("{:?}", this.head.version)));

        registry.add_method("path_and_query", |lua, this, ()| {
            (this.head.uri.path_and_query())
                .map(|pq| lua.create_string(pq.as_str()))
                .transpose()
        });

        registry.add_method("path", |lua, this, ()| lua.create_string(this.head.uri.path()));

        registry.add_method("query", |lua, this, ()| {
            this.head.uri.query().map(|q| lua.create_string(q)).transpose()
        });

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

        registry.add_method_mut("body", |lua, this, ()| {
            match &mut this.body {
                EitherBody::Left(body) => {
                    // Move the body into Lua
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

        registry.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            let mut buf = String::with_capacity(1024);
            let (method, uri, version) = (&this.head.method, &this.head.uri, this.head.version);
            buf.push_str(&format!("{method} {uri} {version:?}\n"));
            // Iterate headers
            for (name, value) in &this.head.headers {
                let value = String::from_utf8_lossy(value.as_bytes());
                buf.push_str(&format!("{name}: {value}\n"));
            }
            Ok(buf)
        });
    }
}

impl From<Request<Incoming>> for LuaRequest {
    fn from(req: Request<Incoming>) -> Self {
        let (head, body) = req.into_parts();
        LuaRequest {
            head,
            body: EitherBody::Left(LuaBody::from(body)),
        }
    }
}

impl FromLua for LuaRequest {
    fn from_lua(value: Value, _lua: &Lua) -> Result<Self> {
        match value {
            Value::Table(params) => {
                let req = Request::new(());
                let mut head = req.into_parts().0;

                if let Some(method) = opt_param!(LuaMethod, Some(&params), "method")? {
                    head.method = method.0;
                }
                if let Some(headers) = opt_param!(LuaHeaders, Some(&params), "headers")? {
                    head.headers = headers.0;
                }

                // Various types of body
                let mut body = LuaBody::default();
                if let Some(body2) = opt_param!(LuaBody, Some(&params), "body")? {
                    body = body2;
                }
                // TODO: json, form, etc

                // Additional custom parameters
                head.extensions.insert(RequestParams::from_table(&params)?);

                Ok(Self {
                    head,
                    body: EitherBody::Left(body),
                })
            }
            Value::UserData(ud) if ud.is::<Self>() => ud.take::<Self>(),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Request".to_string(),
                message: Some("expected Table or Request userdata".to_string()),
            }),
        }
    }
}

/// Additional custom request parameters
#[derive(Clone, Debug, Default)]
pub(crate) struct RequestParams {
    pub(crate) timeout: Option<Duration>,
}

impl RequestParams {
    pub(crate) fn from_table(table: &Table) -> Result<Self> {
        let mut params = RequestParams::default();
        if let Some(timeout) = opt_param!(Duration, Some(table), "timeout")? {
            params.timeout = Some(timeout);
        }
        Ok(params)
    }
}
