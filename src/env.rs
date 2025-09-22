use std::path::PathBuf;
use std::result::Result as StdResult;

use mlua::{Lua, Result, Table};

/// Returns the current working directory
pub fn current_dir(_lua: &Lua, _: ()) -> Result<StdResult<PathBuf, String>> {
    let dir = lua_try!(std::env::current_dir());
    Ok(Ok(dir))
}

/// Changes the current working directory to the specified path
pub fn set_current_dir(_lua: &Lua, path: String) -> Result<StdResult<bool, String>> {
    lua_try!(std::env::set_current_dir(path));
    Ok(Ok(true))
}

/// Returns the full filesystem path of the current running executable
pub fn current_exe(_lua: &Lua, _: ()) -> Result<StdResult<PathBuf, String>> {
    let exe = lua_try!(std::env::current_exe());
    Ok(Ok(exe))
}

/// Returns the path of the current user’s home directory if known
pub fn home_dir(_lua: &Lua, _: ()) -> Result<Option<PathBuf>> {
    Ok(std::env::home_dir())
}

/// Fetches the environment variable key from the current process
pub fn var(_lua: &Lua, key: String) -> Result<Option<String>> {
    Ok(std::env::var(key).ok())
}

/// Returns a table containing all environment variables of the current process
pub fn vars(lua: &Lua, _: ()) -> Result<Table> {
    lua.create_table_from(std::env::vars())
}

/// Sets the environment variable key to the value in the current process
///
/// If value is Nil, the environment variable will be removed
pub fn set_var(_lua: &Lua, (key, value): (String, Option<String>)) -> Result<()> {
    match value {
        Some(v) => unsafe { std::env::set_var(key, v) },
        None => unsafe { std::env::remove_var(key) },
    }
    Ok(())
}

/// A loader for the `env` module.
fn loader(lua: &Lua) -> Result<Table> {
    let t = lua.create_table()?;
    t.set("current_dir", lua.create_function(current_dir)?)?;
    t.set("set_current_dir", lua.create_function(set_current_dir)?)?;
    t.set("current_exe", lua.create_function(current_exe)?)?;
    t.set("home_dir", lua.create_function(home_dir)?)?;
    t.set("var", lua.create_function(var)?)?;
    t.set("vars", lua.create_function(vars)?)?;
    t.set("set_var", lua.create_function(set_var)?)?;

    // Constants
    t.set("ARCH", std::env::consts::ARCH)?;
    t.set("FAMILY", std::env::consts::FAMILY)?;
    t.set("OS", std::env::consts::OS)?;

    Ok(t)
}

/// Registers the `yaml` module in the given Lua state.
pub fn register(lua: &Lua, name: Option<&str>) -> Result<Table> {
    let name = name.unwrap_or("@env");
    let value = loader(lua)?;
    lua.register_module(name, &value)?;
    Ok(value)
}
