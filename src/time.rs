use std::time::{Duration as StdDuration, Instant as StdInstant};

use mlua::{
    Either, Error, FromLua, Lua, MetaMethod, Result, Table, UserData, UserDataMethods, UserDataRef,
    UserDataRegistry, Value,
};

pub(crate) struct Instant(StdInstant);

impl UserData for Instant {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("elapsed", |_, this, ()| Ok(Duration(this.0.elapsed())));

        registry.add_meta_method(
            MetaMethod::Sub,
            |_, this, other: Either<UserDataRef<Self>, Duration>| match other {
                Either::Left(other) => Ok(Either::Left(Duration(this.0.duration_since(other.0)))),
                Either::Right(other) => Ok(Either::Right(Instant(this.0 - other.0))),
            },
        );

        registry.add_meta_method(MetaMethod::Add, |_, this, dur: Duration| {
            Ok(Instant(this.0 + dur.0))
        });
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Duration(pub(crate) StdDuration);

impl UserData for Duration {
    fn register(registry: &mut UserDataRegistry<Self>) {
        registry.add_method("as_secs", |_, this, ()| Ok(this.0.as_secs_f64()));
        registry.add_method("as_millis", |_, this, ()| Ok(this.0.as_millis() as u64));

        registry.add_meta_method(MetaMethod::ToString, |_, this, ()| Ok(format!("{:?}", this.0)));
    }
}

impl FromLua for Duration {
    fn from_lua(value: Value, _: &Lua) -> Result<Self> {
        match value {
            Value::Integer(i) if i >= 0 => Ok(Duration(StdDuration::from_secs(i as u64))),
            Value::Number(n) if n >= 0. => Ok(Duration(StdDuration::from_secs_f64(n))),
            value => {
                match value.as_string().and_then(|s| s.to_str().ok()) {
                    Some(s) if s.ends_with("us") => {
                        let s = &s[..s.len() - 2];
                        if let Ok(micros) = s.parse::<u64>() {
                            return Ok(Duration(StdDuration::from_micros(micros)));
                        }
                    }
                    Some(s) if s.ends_with("ms") => {
                        let s = &s[..s.len() - 2];
                        if let Ok(millis) = s.parse::<u64>() {
                            return Ok(Duration(StdDuration::from_millis(millis)));
                        }
                    }
                    Some(s) if s.ends_with('s') => {
                        let s = &s[..s.len() - 1];
                        if let Ok(secs) = s.parse::<u64>() {
                            return Ok(Duration(StdDuration::from_secs(secs)));
                        }
                    }
                    _ => {}
                }

                Err(Error::FromLuaConversionError {
                    from: value.type_name(),
                    to: "Duration".to_string(),
                    message: Some("expected non-negative number".to_string()),
                })
            }
        }
    }
}

pub(crate) fn instant(_: &Lua, _: ()) -> Result<Instant> {
    Ok(Instant(StdInstant::now()))
}

/// A loader for the `time` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("instant", lua.create_function(instant)?)?;
    Ok(t)
}

/// Registers the `time` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@time");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
