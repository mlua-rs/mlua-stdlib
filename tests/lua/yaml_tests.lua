local yaml = require("@yaml")

-- Test yaml encode
testing:test("encode", function(t)
    local data, err = yaml.encode({ c = { 3, 4, 5, "6" } })
    t.assert_eq(err, nil)
    -- YAML output might have different formatting than JSON
    t.assert(data:find("c:"), "should contain 'c:'")
    t.assert(data:find("- 3"), "should contain sequence items")

    data, err = yaml.encode({ f = function() end })
    t.assert_eq(data, nil)
    t.assert(err:find("cannot serialize <function>"), "unexpected error message: " .. err)

    -- Relaxed mode
    data = yaml.encode({ f = function() end, a = 1 }, { relaxed = true })
    t.assert(data:find("a: 1"), "relaxed mode should work")
end)

-- Test yaml decode
testing:test("decode", function(t)
    local orig_value = { a = 1, b = "2", c = { 3, 4, 5, "6" }, d = true }
    local yaml_str = yaml.encode(orig_value)
    local value, err = yaml.decode(yaml_str)
    t.assert_eq(err, nil)
    t.assert_same(value, orig_value)

    -- Invalid YAML
    value, err = yaml.decode("invalid: yaml: : syntax")
    t.assert_eq(value, nil)
    t.assert(err ~= nil, "should have an error for invalid YAML")

    -- Simple YAML string
    value, err = yaml.decode("key: value\nnumber: 42")
    t.assert_eq(err, nil)
    t.assert_eq(value.key, "value")
    t.assert_eq(value.number, 42)
end)

-- Test decode to native object
testing:test("decode_native", function(t)
    local orig_value = { a = 1, b = "2", c = { 3, 4, 5, "6" }, d = true }
    local yaml_str = yaml.encode(orig_value)
    local native_value, err = yaml.decode_native(yaml_str)
    t.assert_eq(err, nil, err)
    t.assert_eq(type(native_value), "userdata")
    t.assert_eq(native_value.a, 1)
    t.assert_eq(native_value.b, "2")
    t.assert_eq(native_value.c[1], 3)
    t.assert_eq(native_value.c[2], 4)
    t.assert_eq(native_value.c[3], 5)
    t.assert_eq(native_value.c[4], "6")
    t.assert_eq(native_value.d, true)

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
    -- YAML iteration order might be different from JSON
    t.assert(#result > 0, "should have iteration results")

    -- Dump (convert to a lua table)
    local lua_value = native_value:dump()
    t.assert_eq(type(lua_value), "table")
    t.assert_same(lua_value, orig_value)
end)

testing:test("encode_decode_roundtrip", function(t)
    local orig_data = { a = 1, b = "2", c = { 3, 4, 5, "6" }, d = true, e = {} }
    local value, err = yaml.decode(yaml.encode(orig_data))
    t.assert(err == nil, err)
    t.assert_eq(type(value), "table")
    t.assert_same(value, orig_data)

    -- Test various YAML features
    local yaml_str = [[
name: John Doe
age: 30
married: true
children:
  - Alice
  - Bob
address:
  street: 123 Main St
  city: Anytown
]]
    value = yaml.decode_native(yaml_str)
    t.assert_eq(value.name, "John Doe")
    t.assert_eq(value.age, 30)
    t.assert_eq(value.married, true)
    t.assert_eq(value.children[1], "Alice")
    t.assert_eq(value.children[2], "Bob")
    t.assert_eq(value.address.street, "123 Main St")
    t.assert_eq(value.address.city, "Anytown")
end)

testing:test("decode_merge", function(t)
    local yaml_str = [[
default: &default
    name: Default Name
    age: 25

user1:
    <<: *default
    name: Alice
]]
    local value, err = yaml.decode(yaml_str)
    t.assert(err == nil, err)
    t.assert_eq(value.user1.name, "Alice")
    t.assert_eq(value.user1.age, 25)
end)
