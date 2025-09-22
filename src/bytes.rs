use std::ops::Deref;

use mlua::{BorrowedBytes, Error, FromLua, Lua, Result, String as LuaString, UserData, UserDataRef, Value};

use crate::types::MaybeSend;

/// A wrapper around a byte slice that can be passed to Lua as userdata.
#[cfg(not(feature = "send"))]
pub struct BytesBox(Box<dyn AsRef<[u8]>>);

/// A wrapper around a byte slice that can be passed to Lua as userdata.
#[cfg(feature = "send")]
pub struct BytesBox(Box<dyn AsRef<[u8]> + Send>);

impl<T: AsRef<[u8]> + MaybeSend + 'static> From<T> for BytesBox {
    #[inline(always)]
    fn from(value: T) -> Self {
        Self(Box::new(value))
    }
}

impl UserData for BytesBox {}

/// A type that can represent either a Lua string or a `BytesBox` userdata.
pub enum StringOrBytes {
    String(LuaString),
    Bytes(UserDataRef<BytesBox>),
}

impl FromLua for StringOrBytes {
    fn from_lua(value: Value, _lua: &Lua) -> Result<Self> {
        match value {
            Value::String(s) => Ok(Self::String(s)),
            Value::UserData(ud) => Ok(Self::Bytes(ud.borrow::<BytesBox>()?)),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "string or bytes".into(),
                message: None,
            }),
        }
    }
}

impl StringOrBytes {
    #[inline]
    pub(crate) fn as_bytes_deref(&self) -> impl Deref<Target = [u8]> {
        match self {
            StringOrBytes::String(s) => AsBytesRefImpl::Lua(s.as_bytes()),
            StringOrBytes::Bytes(b) => AsBytesRefImpl::Ref((*b.0).as_ref()),
        }
    }
}

enum AsBytesRefImpl<'a> {
    Ref(&'a [u8]),
    Lua(BorrowedBytes<'a>),
}

impl Deref for AsBytesRefImpl<'_> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Ref(b) => b,
            Self::Lua(s) => s.as_ref(),
        }
    }
}
