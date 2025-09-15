use std::ops::Deref;
use std::result::Result as StdResult;
use std::sync::LazyLock;

use mlua::{Lua, MetaMethod, Result, String as LuaString, Table, UserData, UserDataMethods, Value, Variadic};
use ouroboros::self_referencing;
use quick_cache::sync::Cache;

// A reasonable cache size for regexes. This can be adjusted as needed.
const REGEX_CACHE_SIZE: usize = 256;

#[derive(Clone, Debug)]
pub struct Regex(regex::bytes::Regex);

impl Deref for Regex {
    type Target = regex::bytes::Regex;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Global cache for regexes shared across all Lua states.
static CACHE: LazyLock<Cache<String, Regex>> = LazyLock::new(|| Cache::new(REGEX_CACHE_SIZE));

impl Regex {
    /// Creates a new cached regex or retrieves it from the cache if it already exists.
    pub fn new(_: &Lua, re: &str) -> StdResult<Self, regex::Error> {
        if let Some(re) = CACHE.get(re) {
            return Ok(re);
        }
        let regex = Self(regex::bytes::Regex::new(&re)?);
        CACHE.insert(re.to_string(), regex.clone());
        Ok(regex)
    }
}

impl UserData for Regex {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_method("is_match", |_, this, text: LuaString| {
            Ok(this.0.is_match(&text.as_bytes()))
        });

        registry.add_method("match", |lua, this, text: LuaString| {
            let text = (*text.as_bytes()).into();
            let caps = Captures::try_new(text, |text| this.0.captures(text).ok_or(()));
            match caps {
                Ok(caps) => Ok(Value::UserData(lua.create_userdata(caps)?)),
                Err(_) => Ok(Value::Nil),
            }
        });

        // Returns low level information about raw offsets of each submatch.
        registry.add_method("captures_read", |lua, this, text: LuaString| {
            let mut locs = this.capture_locations();
            match this.captures_read(&mut locs, &text.as_bytes()) {
                Some(_) => Ok(Value::UserData(lua.create_userdata(CaptureLocations(locs))?)),
                None => Ok(Value::Nil),
            }
        });

        registry.add_method("split", |lua, this, text: LuaString| {
            lua.create_sequence_from(this.split(&text.as_bytes()).map(LuaString::wrap))
        });

        registry.add_method("splitn", |lua, this, (text, limit): (LuaString, usize)| {
            lua.create_sequence_from(this.splitn(&text.as_bytes(), limit).map(LuaString::wrap))
        });

        registry.add_method("replace", |lua, this, (text, rep): (LuaString, LuaString)| {
            lua.create_string(this.replace(&text.as_bytes(), &*rep.as_bytes()))
        });
    }
}

#[self_referencing]
struct Captures {
    text: Box<[u8]>,

    #[borrows(text)]
    #[covariant]
    caps: regex::bytes::Captures<'this>,
}

impl UserData for Captures {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_meta_method(MetaMethod::Index, |lua, this, key: Value| match key {
            Value::String(s) => {
                let name = s.to_string_lossy();
                this.borrow_caps()
                    .name(&name)
                    .map(|v| lua.create_string(v.as_bytes()))
                    .transpose()
            }
            Value::Integer(i) => this
                .borrow_caps()
                .get(i as usize)
                .map(|v| lua.create_string(v.as_bytes()))
                .transpose(),
            _ => Ok(None),
        })
    }
}

struct CaptureLocations(regex::bytes::CaptureLocations);

impl UserData for CaptureLocations {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        // Returns the total number of capture groups.
        registry.add_method("len", |_, this, ()| Ok(this.0.len()));

        // Returns the start and end positions of the Nth capture group.
        registry.add_method("get", |_, this, i: usize| match this.0.get(i) {
            // We add 1 to the start position because Lua is 1-indexed.
            // End position is non-inclusive, so we don't need to add 1.
            Some((start, end)) => Ok(Variadic::from_iter([start + 1, end])),
            None => Ok(Variadic::new()),
        });
    }
}

struct RegexSet(regex::bytes::RegexSet);

impl Deref for RegexSet {
    type Target = regex::bytes::RegexSet;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UserData for RegexSet {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_function("new", |_, patterns: Vec<String>| {
            let set = lua_try!(regex::bytes::RegexSet::new(patterns).map(RegexSet));
            Ok(Ok(set))
        });

        registry.add_method("is_match", |_, this, text: LuaString| {
            Ok(this.is_match(&text.as_bytes()))
        });

        registry.add_method("len", |_, this, ()| Ok(this.len()));

        registry.add_method("matches", |_, this, text: LuaString| {
            Ok(this
                .matches(&text.as_bytes())
                .iter()
                .map(|i| i + 1)
                .collect::<Vec<_>>())
        });
    }
}

/// Compiles a regular expression.
///
/// Once compiled, it can be used repeatedly to search, split or replace substrings in a text.
fn regex_new(lua: &Lua, re: LuaString) -> Result<StdResult<Regex, String>> {
    let re = re.to_str()?;
    Ok(Ok(lua_try!(Regex::new(lua, &re))))
}

/// Escapes a string so that it can be used as a literal in a regular expression.
fn regex_escape(_: &Lua, text: LuaString) -> Result<String> {
    Ok(regex::escape(&text.to_str()?))
}

/// Returns true if there is a match for the regex anywhere in the given text.
fn regex_is_match(lua: &Lua, (re, text): (LuaString, LuaString)) -> Result<StdResult<bool, String>> {
    let re = re.to_str()?;
    let re = lua_try!(Regex::new(lua, &re));
    Ok(Ok(re.is_match(&text.as_bytes())))
}

/// Returns all matches of the regex in the given text or nil if there is no match.
fn regex_match(lua: &Lua, (re, text): (LuaString, LuaString)) -> Result<StdResult<Value, String>> {
    let re = re.to_str()?;
    let re = lua_try!(Regex::new(lua, &re));
    match re.captures(&text.as_bytes()) {
        Some(caps) => {
            let mut it = caps.iter().map(|om| om.map(|m| LuaString::wrap(m.as_bytes())));
            let first = it.next().unwrap();
            let table = lua.create_sequence_from(it)?;
            table.raw_set(0, first)?;
            Ok(Ok(Value::Table(table)))
        }
        None => Ok(Ok(Value::Nil)),
    }
}

/// A loader for the `regex` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("new", lua.create_function(regex_new)?)?;
    t.set("escape", lua.create_function(regex_escape)?)?;
    t.set("is_match", lua.create_function(regex_is_match)?)?;
    t.set("match", lua.create_function(regex_match)?)?;
    t.set("RegexSet", lua.create_proxy::<RegexSet>()?)?;
    Ok(t)
}

/// Registers the `regex` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@regex");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
