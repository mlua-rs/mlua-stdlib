local json = require("@json")

-- Test json encode
testing:test("encode", function(t)
    local data, err = json.encode({ c = { 3, 4, 5, "6" } })
    t.assert_eq(err, nil)
    t.assert_eq(data, '{"c":[3,4,5,"6"]}')

    data, err = json.encode({ f = function() end })
    t.assert_eq(data, nil)
    t.assert(err:find("cannot serialize <function>"), "unexpected error message: " .. err)

    -- Relaxed mode
    data = json.encode({ f = function() end, a = 1 }, { relaxed = true })
    t.assert_eq(data, '{"a":1}', "relaxed mode")

    -- Pretty mode
    data = json.encode({ b = 2, a = 1 }, { pretty = true })
    t.assert_eq(data, '{\n  "a": 1,\n  "b": 2\n}', "pretty encoding")
end)

-- Test json decode
testing:test("decode", function(t)
    local orig_value = { a = 1, b = "2", c = { 3, 4, 5, "6" }, d = true }
    local json_str = json.encode(orig_value)
    local value, err = json.decode(json_str)
    t.assert_eq(err, nil)
    t.assert_same(value, orig_value)

    -- Invalid JSON
    value, err = json.decode("{a:1}")
    t.assert_eq(value, nil)
    t.assert(err:find("key must be a string"), "unexpected error message: " .. err)

    -- No array metatable by default
    value, err = json.decode("[1,2,3]", { set_array_metatable = false })
    t.assert_eq(err, nil)
    t.assert_eq(type(value), "table", "value is not a table")
    t.assert_eq(getmetatable(value), nil, "array metatable is set")
end)

-- Test decode to native object
testing:test("decode_native", function(t)
    local orig_value = { a = 1, b = "2", c = { 3, 4, 5, "6" }, d = true }
    local json_str = json.encode(orig_value)
    local native_value, err = json.decode_native(json_str)
    t.assert_eq(err, nil, err)
    t.assert_eq(type(native_value), "userdata")
    t.assert_eq(native_value.a, 1)
    t.assert_eq(native_value.b, "2")
    t.assert_eq(native_value.c[1], 3)
    t.assert_eq(native_value.c[2], 4)
    t.assert_eq(native_value.c[3], 5)
    t.assert_eq(native_value.c[4], "6")
    t.assert_eq(native_value.d, true)

    -- Pointers
    t.assert_eq(native_value:pointer("/a"), 1, "/a pointer is not 1")
    t.assert_eq(native_value:pointer("/c/0"), 3, "/c/0 pointer is not 3")

    -- Preserving data types
    local float_data = '{"f":[[],{},0.0,1.0,3]}'
    t.assert_eq(json.encode(json.decode_native(float_data)), float_data)

    -- Iteration
    local function iterate(value, result)
        if result == nil then
            result = {}
        end
        for k, v in value:iter() do
            if type(v) ~= "userdata" then
                table.insert(result, tostring(k))
                table.insert(result, tostring(v))
            else
                iterate(v, result)
            end
        end
        return result
    end
    local result = iterate(native_value)
    t.assert_eq(table.concat(result, ","), "a,1,b,2,1,3,2,4,3,5,4,6,d,true")

    -- Dump (convert to a lua table)
    local lua_value = native_value:dump()
    t.assert_eq(type(lua_value), "table")
    t.assert_same(lua_value, orig_value)
end)

testing:test("encode_decode_roundtrip", function(t)
    local orig_data = { a = 1, b = "2", c = { 3, 4, 5, "6" }, d = true, e = {} }
    local value, err = json.decode(json.encode(orig_data))
    t.assert(err == nil, err)
    t.assert_eq(type(value), "table")
    t.assert_same(value, orig_data)

    -- Preserve data types through "native" roundtrip
    local json_str = [[
        {"float":1.0,"null":null,"bool":true,"array":[1,2,3],"object":{"a":1}}
    ]]
    value = json.decode_native(json_str)
    local alt_str = [[{"array":[1,2,3],"bool":true,"float":1.0,"null":null,"object":{"a":1}}]]
    t.assert_eq(json.encode(value), alt_str, "roundtrip failed")
end)
