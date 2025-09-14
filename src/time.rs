use mlua::{Lua, MetaMethod, Result, UserData, UserDataMethods, UserDataRef};

pub(crate) struct Instant(std::time::Instant);

impl UserData for Instant {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_method("elapsed", |_, this, ()| Ok(Duration(this.0.elapsed())));

        registry.add_meta_method(MetaMethod::Sub, |_, this, other: UserDataRef<Self>| {
            Ok(Duration(this.0.duration_since(other.0)))
        });
    }
}

pub(crate) struct Duration(std::time::Duration);

impl UserData for Duration {
    fn register(registry: &mut mlua::UserDataRegistry<Self>) {
        registry.add_method("as_secs", |_, this, ()| Ok(this.0.as_secs()));
        registry.add_method("as_millis", |_, this, ()| Ok(this.0.as_millis() as u64));

        registry.add_meta_method(MetaMethod::ToString, |_, this, ()| Ok(format!("{:?}", this.0)));
    }
}

pub(crate) fn instant(_: &Lua, _: ()) -> Result<Instant> {
    Ok(Instant(std::time::Instant::now()))
}
