use mlua::{ExternalResult, FromLua, Lua, Result, String as LuaString, Value};

/// A Lua userdata wrapper around [`http::Method`].
#[derive(Clone, Default, Debug)]
pub struct LuaMethod(pub http::Method);

impl FromLua for LuaMethod {
    fn from_lua(value: Value, lua: &Lua) -> Result<Self> {
        let s = LuaString::from_lua(value, lua)?;
        http::Method::from_bytes(&s.as_bytes()).map(Self).into_lua_err()
    }
}
