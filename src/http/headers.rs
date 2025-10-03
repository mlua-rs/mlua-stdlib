use http::header::{HeaderMap, HeaderName, HeaderValue};
use mlua::{
    Either, ExternalError, ExternalResult, FromLua, Lua, MetaMethod, Result, String as LuaString, Table,
    UserData, UserDataMethods, Value,
};

#[derive(Clone)]
pub(crate) struct Headers(HeaderMap);

impl UserData for Headers {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_function("new", |lua, arg: Option<Table>| match arg {
            None => Ok(Headers(HeaderMap::new())),
            Some(t) => Headers::from_lua(Value::Table(t), lua),
        });

        registry.add_method("get", |lua, this, name: LuaString| {
            let name = name.to_str()?;
            (this.0.get(&*name))
                .map(|v| lua.create_string(v.as_ref()))
                .transpose()
        });

        registry.add_method("get_all", |lua, this, name: LuaString| {
            let name = name.to_str()?;
            (this.0.get_all(&*name))
                .iter()
                .map(|v| lua.create_string(v.as_ref()))
                .collect::<Result<Vec<_>>>()
        });

        registry.add_method("get_count", |_, this, name: LuaString| {
            let name = name.to_str()?;
            Ok(this.0.get_all(&*name).iter().count())
        });

        registry.add_method_mut("set", |_, this, (name, value): (LuaString, LuaString)| {
            let name = HeaderName::from_lua(name)?;
            let value = HeaderValue::from_lua(value)?;
            this.0.insert(name, value);
            Ok(())
        });

        registry.add_method_mut("add", |_, this, (name, value): (LuaString, LuaString)| {
            let name = HeaderName::from_lua(name)?;
            let value = HeaderValue::from_lua(value)?;
            Ok(this.0.append(name, value))
        });

        registry.add_method_mut("remove", |_, this, name: LuaString| {
            let name = HeaderName::from_lua(name)?;
            this.0.remove(&name);
            Ok(())
        });

        registry.add_method("count", |_, this, ()| Ok(this.0.len()));

        registry.add_method_mut("clear", |_, this, ()| {
            this.0.clear();
            Ok(())
        });

        registry.add_method("keys", |lua, this, ()| {
            (this.0.keys())
                .map(|n| lua.create_string(n.as_str()))
                .collect::<Result<Vec<_>>>()
        });

        registry.add_method("clone", |_, this, ()| Ok(Headers(this.0.clone())));

        // Convert headers map to a Lua table
        registry.add_method("to_table", |lua, this, ()| {
            let table = lua.create_table_with_capacity(0, this.0.keys_len())?;
            for key in this.0.keys() {
                let name = lua.create_string(&key)?;
                let mut iter = this.0.get_all(key).iter().enumerate().peekable();
                let mut values = None;
                while let Some((i, value)) = iter.next() {
                    let value = lua.create_string(value.as_ref())?;
                    if i == 0 && iter.peek().is_none() {
                        table.raw_set(name, value)?;
                        break;
                    }
                    // There are multiple values for this header, store them in a sub-table
                    if i == 0 {
                        values = Some(lua.create_table()?);
                        values.as_ref().unwrap().raw_set(i + 1, value)?;
                        table.raw_set(name.clone(), values.as_ref().unwrap())?;
                    } else {
                        values.as_ref().unwrap().raw_set(i + 1, value)?;
                    }
                }
            }
            set_headers_metatable(lua, &table)?;
            Ok(table)
        });

        // Index
        registry.add_meta_method(MetaMethod::Index, |lua, this, key: LuaString| {
            let key = key.to_str()?;
            match this.0.get(&*key) {
                Some(value) => Ok(Some(lua.create_string(value.as_ref())?)),
                None => Ok(None),
            }
        });

        // NewIndex
        registry.add_meta_method_mut(
            MetaMethod::NewIndex,
            |_, this, (key, value): (LuaString, Either<Option<LuaString>, Table>)| {
                let key = HeaderName::from_lua(key)?;
                match value {
                    Either::Left(None) => {
                        this.0.remove(&key);
                    }
                    Either::Left(Some(v)) => {
                        let value = HeaderValue::from_lua(v)?;
                        this.0.insert(key, value);
                    }
                    Either::Right(t) => {
                        this.0.remove(&key);
                        for (i, v) in t.sequence_values::<LuaString>().enumerate() {
                            let value = HeaderValue::from_lua(v?)?;
                            if i == 0 {
                                this.0.insert(key.clone(), value);
                                continue;
                            }
                            this.0.append(key.clone(), value);
                        }
                    }
                }
                Ok(())
            },
        );
    }
}

impl FromLua for Headers {
    fn from_lua(value: Value, lua: &Lua) -> Result<Self> {
        match value {
            Value::Table(table) => {
                let mut headers = HeaderMap::new();
                table.for_each::<LuaString, Value>(|key, value| {
                    let name = HeaderName::from_lua(key)?;
                    // Maybe `value` is a list of values
                    if let Value::Table(values) = value {
                        for value in values.sequence_values::<LuaString>() {
                            headers.append(name.clone(), HeaderValue::from_lua(value?)?);
                        }
                    } else {
                        let value = lua.unpack::<LuaString>(value)?;
                        headers.append(name, HeaderValue::from_lua(value)?);
                    }
                    Ok(())
                })?;
                Ok(Headers(headers))
            }
            Value::UserData(ud) if ud.is::<Self>() => ud.borrow::<Self>().map(|hdrs| hdrs.clone()),
            val => {
                let type_name = val.type_name();
                let msg = format!("cannot make headers from {type_name}");
                Err(msg.into_lua_err())
            }
        }
    }
}

/// Sets a metatable for the given headers table to handle case-insensitive keys
/// and normalize underscores to dashes.
fn set_headers_metatable(lua: &Lua, headers: &Table) -> Result<()> {
    if let Ok(Some(mt)) = lua.named_registry_value::<Option<Table>>("__headers_metatable") {
        return headers.set_metatable(Some(mt));
    }

    // Create a new metatable
    let metatable = lua
        .load(
            r#"
            return {
                __index = function(self, key)
                    local normalized_key = string.lower(key)
                    return rawget(self, normalized_key)
                end,

                __newindex = function(self, key, value)
                    local normalized_key = string.lower(key)
                    rawset(self, normalized_key, value)
                end
            }
        "#,
        )
        .eval::<Table>()?;

    // Cache it in the Lua registry
    lua.set_named_registry_value("__headers_metatable", &metatable)?;
    headers.set_metatable(Some(metatable))
}

pub(crate) trait LuaHeaderExt {
    fn from_lua(value: LuaString) -> Result<Self>
    where
        Self: Sized;
}

impl LuaHeaderExt for HeaderName {
    #[inline]
    fn from_lua(value: LuaString) -> Result<Self> {
        HeaderName::from_bytes(&value.as_bytes()).into_lua_err()
    }
}

impl LuaHeaderExt for HeaderValue {
    #[inline]
    fn from_lua(value: LuaString) -> Result<Self> {
        HeaderValue::from_bytes(&value.as_bytes()).into_lua_err()
    }
}
