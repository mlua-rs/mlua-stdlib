#[rustfmt::skip]
#[allow(unused)]
pub(crate) const METAMETHOD_ITER: &str = if cfg!(feature = "luau") { "__iter" } else { "__pairs" };

#[macro_use]
mod macros;
mod types;

pub(crate) mod terminal;
pub(crate) mod time;

pub mod assertions;
pub mod bytes;
pub mod env;
pub mod testing;

#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "regex")]
pub mod regex;
#[cfg(feature = "yaml")]
pub mod yaml;

pub mod http;
