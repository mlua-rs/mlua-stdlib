use std::ops::{Deref, DerefMut};

use bytes::Bytes;
use mlua::{
    BorrowedBytes, Error, FromLua, Lua, MetaMethod, Result, String as LuaString, Table, UserData,
    UserDataMethods, UserDataRegistry, Value,
};

/// A Lua userdata wrapper around [`Bytes`].
#[derive(Clone, Default, Debug, FromLua, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct LuaBytes(pub Bytes);

impl Deref for LuaBytes {
    type Target = Bytes;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LuaBytes {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl UserData for LuaBytes {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_function("new", |_, data: LuaString| {
            Ok(Self(Bytes::copy_from_slice(&data.as_bytes())))
        });

        registry.add_method("len", |_, this, ()| Ok(this.len()));

        registry.add_method("is_empty", |_, this, ()| Ok(this.is_empty()));

        registry.add_method_mut("split_off", |_, this, n| Ok(Self(this.split_off(n))));
        registry.add_method_mut("split_to", |_, this, n| Ok(Self(this.split_to(n))));

        registry.add_method_mut("clear", |_, this, ()| {
            this.clear();
            Ok(())
        });
        registry.add_method_mut("truncate", |_, this, len| {
            this.truncate(len);
            Ok(())
        });

        registry.add_meta_method(MetaMethod::ToString, |lua, this, ()| lua.create_string(&this.0));
    }
}

/// A type that can represent either a Lua string or a [`LuaBytes`] userdata.
pub enum StringOrBytes {
    String(LuaString),
    Bytes(LuaBytes),
}

impl FromLua for StringOrBytes {
    fn from_lua(value: Value, _lua: &Lua) -> Result<Self> {
        match value {
            Value::String(s) => Ok(Self::String(s)),
            Value::UserData(ud) => Ok(Self::Bytes(ud.borrow::<LuaBytes>()?.clone())),
            _ => Err(Error::FromLuaConversionError {
                from: value.type_name(),
                to: "String or Bytes".into(),
                message: None,
            }),
        }
    }
}

impl StringOrBytes {
    /// Get a type that dereferences to a underlying byte slice.
    #[inline]
    pub fn as_bytes_deref(&self) -> impl Deref<Target = [u8]> {
        match self {
            StringOrBytes::String(s) => AsBytesRefImpl::Lua(s.as_bytes()),
            StringOrBytes::Bytes(b) => AsBytesRefImpl::Ref(b.as_ref()),
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

/// A loader for the `bytes` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("Bytes", lua.create_proxy::<LuaBytes>()?)?;
    Ok(t)
}

/// Registers the `bytes` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@bytes");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
