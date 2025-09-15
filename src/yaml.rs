use std::result::Result as StdResult;
use std::sync::Arc;

use mlua::{
    AnyUserData, Error, Function, Integer as LuaInteger, IntoLuaMulti, Lua, LuaSerdeExt, MetaMethod,
    MultiValue, Result, SerializeOptions, Table, UserData, UserDataMethods, UserDataRefMut, Value,
};
use ouroboros::self_referencing;
use serde::{Serialize, Serializer};

use crate::bytes::StringOrBytes;

/// Represents a native YAML object in Lua.
#[derive(Clone)]
pub(crate) struct YamlObject {
    root: Arc<serde_yaml::Value>,
    current: *const serde_yaml::Value,
}

impl Serialize for YamlObject {
    fn serialize<S: Serializer>(&self, serializer: S) -> StdResult<S::Ok, S::Error> {
        self.current().serialize(serializer)
    }
}

impl YamlObject {
    /// Creates a new `YamlObject` from the given YAML value.
    ///
    /// SAFETY:
    /// The caller must ensure that `current` is a value inside `root`.
    unsafe fn new(root: &Arc<serde_yaml::Value>, current: &serde_yaml::Value) -> Self {
        let root = root.clone();
        YamlObject { root, current }
    }

    /// Returns a reference to the current YAML value.
    #[inline(always)]
    fn current(&self) -> &serde_yaml::Value {
        unsafe { &*self.current }
    }

    /// Returns a new `YamlObject` which points to the value at the given key.
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

    /// Converts this `YamlObject` into a Lua `Value`.
    fn into_lua(self, lua: &Lua) -> Result<Value> {
        match self.current() {
            serde_yaml::Value::Null => Ok(Value::NULL),
            serde_yaml::Value::Bool(b) => Ok(Value::Boolean(*b)),
            serde_yaml::Value::Number(n) => {
                if let Some(n) = n.as_i64() {
                    Ok(Value::Integer(n as _))
                } else if let Some(n) = n.as_f64() {
                    Ok(Value::Number(n))
                } else {
                    Err(Error::ToLuaConversionError {
                        from: "number".to_string(),
                        to: "integer or float",
                        message: Some("number is too big to fit in a Lua integer".to_owned()),
                    })
                }
            }
            serde_yaml::Value::String(s) => Ok(Value::String(lua.create_string(s)?)),
            value @ serde_yaml::Value::Sequence(_) | value @ serde_yaml::Value::Mapping(_) => {
                let obj_ud = lua.create_ser_userdata(unsafe { YamlObject::new(&self.root, value) })?;
                Ok(Value::UserData(obj_ud))
            }
            serde_yaml::Value::Tagged(tagged) => {
                // For tagged values, we'll return the value part and ignore the tag for simplicity
                let obj = unsafe { YamlObject::new(&self.root, &tagged.value) };
                obj.into_lua(lua)
            }
        }
    }

    fn lua_iterator(&self, lua: &Lua) -> Result<MultiValue> {
        match self.current() {
            serde_yaml::Value::Sequence(_) => {
                let next = Self::lua_array_iterator(lua)?;
                let iter_ud = AnyUserData::wrap(LuaYamlArrayIter {
                    value: self.clone(),
                    next: 1, // index starts at 1
                });
                (next, iter_ud).into_lua_multi(lua)
            }
            serde_yaml::Value::Mapping(_) => {
                let next = Self::lua_map_iterator(lua)?;
                let iter_builder = LuaYamlMapIterBuilder {
                    value: self.clone(),
                    iter_builder: |value| value.current().as_mapping().unwrap().iter(),
                };
                let iter_ud = AnyUserData::wrap(iter_builder.build());
                (next, iter_ud).into_lua_multi(lua)
            }
            _ => ().into_lua_multi(lua),
        }
    }

    /// Returns an iterator function for arrays.
    fn lua_array_iterator(lua: &Lua) -> Result<Function> {
        if let Ok(Some(f)) = lua.named_registry_value("__yaml_array_iterator") {
            return Ok(f);
        }

        let f = lua.create_function(|lua, mut it: UserDataRefMut<LuaYamlArrayIter>| {
            it.next += 1;
            match it.value.get(Value::Integer(it.next - 1)) {
                Some(next_value) => (it.next - 1, next_value.into_lua(lua)?).into_lua_multi(lua),
                None => ().into_lua_multi(lua),
            }
        })?;
        lua.set_named_registry_value("__yaml_array_iterator", &f)?;
        Ok(f)
    }

    /// Returns an iterator function for objects.
    fn lua_map_iterator(lua: &Lua) -> Result<Function> {
        if let Ok(Some(f)) = lua.named_registry_value("__yaml_map_iterator") {
            return Ok(f);
        }

        let f = lua.create_function(|lua, mut it: UserDataRefMut<LuaYamlMapIter>| {
            let root = it.borrow_value().root.clone();
            it.with_iter_mut(move |iter| match iter.next() {
                Some((key, value)) => {
                    // Convert YAML key to Lua value
                    let key = match key {
                        serde_yaml::Value::Null
                        | serde_yaml::Value::Bool(..)
                        | serde_yaml::Value::String(..)
                        | serde_yaml::Value::Number(..) => unsafe {
                            YamlObject::new(&root, key).into_lua(lua)?
                        },
                        _ => {
                            let err =
                                Error::runtime("only string/number/boolean keys are supported in YAML maps");
                            return Err(err);
                        }
                    };
                    let value = unsafe { YamlObject::new(&root, value) }.into_lua(lua)?;
                    (key, value).into_lua_multi(lua)
                }
                None => ().into_lua_multi(lua),
            })
        })?;
        lua.set_named_registry_value("__yaml_map_iterator", &f)?;
        Ok(f)
    }
}

impl From<serde_yaml::Value> for YamlObject {
    fn from(value: serde_yaml::Value) -> Self {
        let root = Arc::new(value);
        unsafe { Self::new(&root, &root) }
    }
}

impl UserData for YamlObject {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
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

struct LuaYamlArrayIter {
    value: YamlObject,
    next: LuaInteger,
}

#[self_referencing]
struct LuaYamlMapIter {
    value: YamlObject,

    #[borrows(value)]
    #[covariant]
    iter: serde_yaml::mapping::Iter<'this>,
}

fn decode(lua: &Lua, (data, opts): (StringOrBytes, Option<Table>)) -> Result<StdResult<Value, String>> {
    let opts = opts.as_ref();
    let mut options = SerializeOptions::new();
    if let Some(enabled) = opts.and_then(|t| t.get::<bool>("set_array_metatable").ok()) {
        options = options.set_array_metatable(enabled);
    }

    let mut yaml: serde_yaml::Value = lua_try!(serde_yaml::from_slice(&data.as_bytes_deref()));
    lua_try!(yaml.apply_merge());
    Ok(Ok(lua.to_value_with(&yaml, options)?))
}

fn decode_native(lua: &Lua, data: StringOrBytes) -> Result<StdResult<Value, String>> {
    let mut yaml: serde_yaml::Value = lua_try!(serde_yaml::from_slice(&data.as_bytes_deref()));
    lua_try!(yaml.apply_merge());
    Ok(Ok(lua_try!(YamlObject::from(yaml).into_lua(lua))))
}

fn encode(value: Value, opts: Option<Table>) -> StdResult<String, String> {
    let opts = opts.as_ref();
    let mut value = value.to_serializable();

    if opts.and_then(|t| t.get::<bool>("relaxed").ok()) == Some(true) {
        value = value.deny_recursive_tables(false).deny_unsupported_types(false);
    }

    serde_yaml::to_string(&value).map_err(|e| e.to_string())
}

/// A loader for the `yaml` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("decode", lua.create_function(decode)?)?;
    t.set("decode_native", lua.create_function(decode_native)?)?;
    t.set("encode", Function::wrap_raw(encode))?;
    Ok(t)
}

/// Registers the `yaml` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@yaml");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
