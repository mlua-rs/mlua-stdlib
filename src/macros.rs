#![allow(unused_macros)]

macro_rules! lua_try {
    ($result:expr) => {
        match $result {
            Ok(ok) => ok,
            Err(err) => return Ok(Err(format!("{err:#}"))),
        }
    };
}

macro_rules! defer {
    ($($item:tt)*) => {
        let _guard = crate::util::defer(|| { $($item)* });
    };
}

macro_rules! opt_param {
    ($table:expr, $name:expr) => {
        match ($table.as_ref())
            .map(|t| t.raw_get::<Option<_>>($name))
            .transpose()
        {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Ok(None),
            Err(err) => {
                use mlua::ErrorContext as _;
                Err(err.with_context(|_| format!("invalid `{}` param", $name)))
            }
        }
    };

    ($ty:ty, $table:expr, $name:expr) => {
        match ($table.as_ref())
            .map(|t| t.raw_get::<Option<$ty>>($name))
            .transpose()
        {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Ok(None),
            Err(err) => {
                use mlua::ErrorContext as _;
                Err(err.with_context(|_| format!("invalid `{}` param", $name)))
            }
        }
    };
}

macro_rules! param {
    ($table:expr, $name:expr) => {
        match $table.raw_get::<Option<_>>($name) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Err(mlua::Error::runtime(format!("`{}` param is required", $name))),
            Err(err) => {
                use mlua::ErrorContext as _;
                Err(err.with_context(|_| format!("invalid `{}` param", $name)))
            }
        }
    };
}
