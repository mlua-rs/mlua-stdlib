#[rustfmt::skip]
#[allow(unused)]
pub(crate) const METAMETHOD_ITER: &str = if cfg!(feature = "luau") { "__iter" } else { "__pairs" };

#[macro_use]
mod macros;
mod types;
mod util;

pub(crate) mod terminal;

pub mod assertions;
pub mod bytes;
pub mod env;
pub mod testing;
pub mod time;

#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(feature = "yaml")]
pub mod yaml;

#[cfg(feature = "http")]
pub mod http;

#[cfg(feature = "task")]
pub mod task;
