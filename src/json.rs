use std::result::Result as StdResult;
use std::sync::Arc;

use mlua::{
    AnyUserData, Error as LuaError, Function, Integer as LuaInteger, IntoLuaMulti, Lua, LuaSerdeExt,
    MetaMethod, MultiValue, Result, SerializeOptions, String as LuaString, Table, UserData, UserDataMethods,
    UserDataRefMut, Value,
};
use ouroboros::self_referencing;
use serde::{Serialize, Serializer};

use crate::bytes::StringOrBytes;

/// Represents a native Json object in Lua.
#[derive(Clone)]
pub(crate) struct JsonObject {
    root: Arc<serde_json::Value>,
    current: *const serde_json::Value,
}

impl Serialize for JsonObject {
    fn serialize<S: Serializer>(&self, serializer: S) -> StdResult<S::Ok, S::Error> {
        self.current().serialize(serializer)
    }
}

impl JsonObject {
    /// Creates a new `JsonObject` from the given JSON value.
    ///
    /// SAFETY:
    /// The caller must ensure that `current` is a value inside `root`.
    unsafe fn new(root: &Arc<serde_json::Value>, current: &serde_json::Value) -> Self {
        let root = root.clone();
        JsonObject { root, current }
    }

    /// Returns a reference to the current JSON value.
    #[inline(always)]
    fn current(&self) -> &serde_json::Value {
        unsafe { &*self.current }
    }

    /// Returns a new `JsonObject` which points to the value at the given key.
    ///
    /// This operation is cheap and does not clone the underlying data.
    fn get(&self, key: Value) -> Option<Self> {
        let value = match key {
            Value::Integer(index) if index > 0 => self.current().get(index as usize - 1),
            Value::String(key) => key.to_str().ok().and_then(|s| self.current().get(&*s)),
            _ => None,
        }?;
        unsafe { Some(Self::new(&self.root, value)) }
    }

    /// Returns a new `JsonObject` by following the given JSON Pointer path.
    fn pointer(&self, path: &str) -> Option<Self> {
        unsafe { Some(JsonObject::new(&self.root, self.root.pointer(path)?)) }
    }

    /// Converts this `JsonObject` into a Lua `Value`.
    fn into_lua(self, lua: &Lua) -> Result<Value> {
        match self.current() {
            serde_json::Value::Null => Ok(Value::NULL),
            serde_json::Value::Bool(b) => Ok(Value::Boolean(*b)),
            serde_json::Value::Number(n) => {
                if let Some(n) = n.as_i64() {
                    Ok(Value::Integer(n as _))
                } else if let Some(n) = n.as_f64() {
                    Ok(Value::Number(n))
                } else {
                    Err(LuaError::ToLuaConversionError {
                        from: "number".to_string(),
                        to: "integer or float",
                        message: Some("number is too big to fit in a Lua integer".to_owned()),
                    })
                }
            }
            serde_json::Value::String(s) => Ok(Value::String(lua.create_string(s)?)),
            value @ serde_json::Value::Array(_) | value @ serde_json::Value::Object(_) => {
                let obj_ud = lua.create_ser_userdata(unsafe { JsonObject::new(&self.root, value) })?;
                Ok(Value::UserData(obj_ud))
            }
        }
    }

    fn lua_iterator(&self, lua: &Lua) -> Result<MultiValue> {
        match self.current() {
            serde_json::Value::Array(_) => {
                let next = Self::lua_array_iterator(lua)?;
                let iter_ud = AnyUserData::wrap(LuaJsonArrayIter {
                    value: self.clone(),
                    next: 1, // index starts at 1
                });
                (next, iter_ud).into_lua_multi(lua)
            }
            serde_json::Value::Object(_) => {
                let next = Self::lua_map_iterator(lua)?;
                let iter_builder = LuaJsonMapIterBuilder {
                    value: self.clone(),
                    iter_builder: |value| value.current().as_object().unwrap().iter(),
                };
                let iter_ud = AnyUserData::wrap(iter_builder.build());
                (next, iter_ud).into_lua_multi(lua)
            }
            _ => ().into_lua_multi(lua),
        }
    }

    /// Returns an iterator function for arrays.
    fn lua_array_iterator(lua: &Lua) -> Result<Function> {
        if let Ok(Some(f)) = lua.named_registry_value("__json_array_iterator") {
            return Ok(f);
        }

        let f = lua.create_function(|lua, mut it: UserDataRefMut<LuaJsonArrayIter>| {
            it.next += 1;
            match it.value.get(Value::Integer(it.next - 1)) {
                Some(next_value) => (it.next - 1, next_value.into_lua(lua)?).into_lua_multi(lua),
                None => ().into_lua_multi(lua),
            }
        })?;
        lua.set_named_registry_value("__json_array_iterator", &f)?;
        Ok(f)
    }

    /// Returns an iterator function for objects.
    fn lua_map_iterator(lua: &Lua) -> Result<Function> {
        if let Ok(Some(f)) = lua.named_registry_value("__json_map_iterator") {
            return Ok(f);
        }

        let f = lua.create_function(|lua, mut it: UserDataRefMut<LuaJsonMapIter>| {
            let root = it.borrow_value().root.clone();
            it.with_iter_mut(move |iter| match iter.next() {
                Some((key, value)) => {
                    let key = lua.create_string(key)?;
                    let value = unsafe { JsonObject::new(&root, value) }.into_lua(lua)?;
                    (key, value).into_lua_multi(lua)
                }
                None => ().into_lua_multi(lua),
            })
        })?;
        lua.set_named_registry_value("__json_map_iterator", &f)?;
        Ok(f)
    }
}

impl From<serde_json::Value> for JsonObject {
    fn from(value: serde_json::Value) -> Self {
        let root = Arc::new(value);
        unsafe { Self::new(&root, &root) }
    }
}

impl UserData for JsonObject {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_method("pointer", |lua, this, path: LuaString| {
            this.pointer(&path.to_str()?)
                .map(|obj| obj.into_lua(lua))
                .unwrap_or(Ok(Value::Nil))
        });

        registry.add_method("dump", |lua, this, ()| lua.to_value(this));

        registry.add_method("iter", |lua, this, ()| this.lua_iterator(lua));

        registry.add_meta_method(MetaMethod::Index, |lua, this, key: Value| {
            this.get(key)
                .map(|obj| obj.into_lua(lua))
                .unwrap_or(Ok(Value::Nil))
        });

        registry.add_meta_method(crate::METAMETHOD_ITER, |lua, this, ()| this.lua_iterator(lua));
    }
}

struct LuaJsonArrayIter {
    value: JsonObject,
    next: LuaInteger,
}

#[self_referencing]
struct LuaJsonMapIter {
    value: JsonObject,

    #[borrows(value)]
    #[covariant]
    iter: serde_json::map::Iter<'this>,
}

fn decode(lua: &Lua, (data, opts): (StringOrBytes, Option<Table>)) -> Result<StdResult<Value, String>> {
    let opts = opts.as_ref();
    let mut options = SerializeOptions::new();
    if let Some(enabled) = opts.and_then(|t| t.get::<bool>("set_array_metatable").ok()) {
        options = options.set_array_metatable(enabled);
    }

    let json: serde_json::Value = lua_try!(serde_json::from_slice(&data.as_bytes_deref()));
    Ok(Ok(lua.to_value_with(&json, options)?))
}

fn decode_native(lua: &Lua, data: StringOrBytes) -> Result<StdResult<Value, String>> {
    let json: serde_json::Value = lua_try!(serde_json::from_slice(&data.as_bytes_deref()));
    Ok(Ok(lua_try!(JsonObject::from(json).into_lua(lua))))
}

fn encode(value: Value, options: Option<Table>) -> StdResult<String, String> {
    let mut value = value.to_serializable();
    let options = options.as_ref();

    if options.and_then(|t| t.get::<bool>("relaxed").ok()) == Some(true) {
        value = value.deny_recursive_tables(false).deny_unsupported_types(false);
    }

    if options.and_then(|t| t.get::<bool>("pretty").ok()) == Some(true) {
        value = value.sort_keys(true);
        return serde_json::to_string_pretty(&value).map_err(|e| e.to_string());
    }

    serde_json::to_string(&value).map_err(|e| e.to_string())
}

/// A loader for the `json` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("decode", lua.create_function(decode)?)?;
    t.set("decode_native", lua.create_function(decode_native)?)?;
    t.set("encode", Function::wrap_raw(encode))?;
    Ok(t)
}

/// Registers the `json` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@json");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
