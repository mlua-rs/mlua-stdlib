use std::fmt;
use std::time::{Duration, Instant};

use mlua::{
    Either, Error, FromLua, Lua, MetaMethod, Result, Table, UserData, UserDataMethods, UserDataRef,
    UserDataRegistry, Value,
};

/// A Lua wrapper around [`Instant`].
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LuaInstant(pub Instant);

impl fmt::Debug for LuaInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl UserData for LuaInstant {
    fn register(registry: &mut UserDataRegistry<Self>) {
        // Static method to get the current instant
        registry.add_function("now", |_, ()| Ok(LuaInstant(Instant::now())));

        registry.add_method("elapsed", |_, this, ()| Ok(LuaDuration(this.0.elapsed())));

        registry.add_meta_method(
            MetaMethod::Sub,
            |_, this, other: Either<UserDataRef<Self>, LuaDuration>| match other {
                Either::Left(other) => Ok(Either::Left(LuaDuration(this.0.duration_since(other.0)))),
                Either::Right(other) => Ok(Either::Right(LuaInstant(this.0 - other.0))),
            },
        );

        registry.add_meta_method(MetaMethod::Add, |_, this, dur: LuaDuration| {
            Ok(LuaInstant(this.0 + dur.0))
        });

        registry.add_meta_method(MetaMethod::Eq, |_, this, other: UserDataRef<Self>| {
            Ok(this.0 == other.0)
        });

        registry.add_meta_method(MetaMethod::Lt, |_, this, other: UserDataRef<Self>| {
            Ok(this.0 < other.0)
        });

        registry.add_meta_method(MetaMethod::Le, |_, this, other: UserDataRef<Self>| {
            Ok(this.0 <= other.0)
        });
    }
}

/// A Lua wrapper around [`Duration`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LuaDuration(pub Duration);

impl fmt::Debug for LuaDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl UserData for LuaDuration {
    fn register(registry: &mut UserDataRegistry<Self>) {
        // Static method to parse a duration from a string
        registry.add_function("parse", |_, s: String| {
            let dur = humantime::parse_duration(&s)
                .map_err(|e| Error::RuntimeError(format!("failed to parse duration: {e}")))?;
            Ok(LuaDuration(dur))
        });

        registry.add_method("as_secs", |_, this, ()| Ok(this.0.as_secs_f64()));
        registry.add_method("as_millis", |_, this, ()| Ok(this.0.as_millis() as u64));
        registry.add_method("as_micros", |_, this, ()| Ok(this.0.as_micros() as u64));

        registry.add_meta_method(MetaMethod::Sub, |_, this, other: LuaDuration| {
            Ok(LuaDuration(this.0 - other.0))
        });

        registry.add_meta_method(MetaMethod::Add, |_, this, dur: LuaDuration| {
            Ok(LuaDuration(this.0 + dur.0))
        });

        registry.add_meta_method(MetaMethod::Eq, |_, this, other: UserDataRef<Self>| {
            Ok(this.0 == other.0)
        });

        registry.add_meta_method(MetaMethod::Lt, |_, this, other: UserDataRef<Self>| {
            Ok(this.0 < other.0)
        });

        registry.add_meta_method(MetaMethod::Le, |_, this, other: UserDataRef<Self>| {
            Ok(this.0 <= other.0)
        });

        registry.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(humantime::format_duration(this.0).to_string())
        });
    }
}

impl FromLua for LuaDuration {
    fn from_lua(value: Value, _: &Lua) -> Result<Self> {
        match value {
            Value::Integer(i) if i >= 0 => return Ok(LuaDuration(Duration::from_secs(i as u64))),
            Value::Number(n) if n >= 0. => return Ok(LuaDuration(Duration::from_secs_f64(n))),
            Value::String(s) => {
                let s = s.to_str()?;
                let dur = humantime::parse_duration(&s).map_err(|e| Error::FromLuaConversionError {
                    from: "string",
                    to: "LuaDuration".to_string(),
                    message: Some(format!("failed to parse duration: {e}")),
                })?;
                return Ok(LuaDuration(dur));
            }
            Value::UserData(ud) if ud.is::<Self>() => {
                return Ok(*ud.borrow()?);
            }
            _ => {}
        }
        Err(Error::FromLuaConversionError {
            from: value.type_name(),
            to: "LuaDuration".to_string(),
            message: Some("expected a valid string or number".to_string()),
        })
    }
}

/// A loader for the `time` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("Instant", lua.create_proxy::<LuaInstant>()?)?;
    t.set("Duration", lua.create_proxy::<LuaDuration>()?)?;
    Ok(t)
}

/// Registers the `time` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@time");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
