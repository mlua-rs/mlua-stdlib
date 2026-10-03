#![cfg(feature = "send")]

#[cfg(any(feature = "json", feature = "yaml"))]
fn native_object_across_threads(register: fn(&mlua::Lua, Option<&str>) -> mlua::Result<mlua::Table>) {
    let lua = mlua::Lua::new();
    register(&lua, Some("@native")).unwrap();
    lua.load(
        r#"
        child = assert(require("@native").decode_native('{"nested":{"answer":42}}')).nested
        next_value, iter_state = child:iter()
        "#,
    )
    .exec()
    .unwrap();
    lua.gc_collect().unwrap();

    // Both a child view and a self-referencing iterator must keep the immutable
    // root alive when the parent userdata is collected and the Lua state moves.
    std::thread::spawn(move || {
        lua.load(
            r#"
            assert(child.answer == 42)
            local key, value = next_value(iter_state)
            assert(key == "answer" and value == 42)
            assert(next_value(iter_state) == nil)
            "#,
        )
        .exec()
        .unwrap();
    })
    .join()
    .unwrap();
}

#[cfg(feature = "json")]
#[test]
fn json_native_object_across_threads() {
    native_object_across_threads(mlua_stdlib::json::register);
}

#[cfg(feature = "yaml")]
#[test]
fn yaml_native_object_across_threads() {
    native_object_across_threads(mlua_stdlib::yaml::register);
}

#[cfg(feature = "http")]
#[test]
fn http_body_reader_across_threads() {
    let lua = mlua::Lua::new();
    let body = mlua_stdlib::http::LuaBody::from(bytes::Bytes::from_static(b"hello"));
    lua.globals().set("body", body).unwrap();
    let reader: mlua::Function = lua.load("return body:reader()").eval().unwrap();

    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let chunk: mlua::AnyUserData = reader.call_async(()).await.unwrap();
                assert_eq!(&**chunk.borrow::<bytes::Bytes>().unwrap(), b"hello");
                let end: Option<mlua::AnyUserData> = reader.call_async(()).await.unwrap();
                assert!(end.is_none());
            });
    })
    .join()
    .unwrap();
}
