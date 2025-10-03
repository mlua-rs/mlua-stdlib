local assertions = require("@assertions")

local assert_eq = assertions.assert_eq
local assert_ne = assertions.assert_ne
local assert_match = assertions.assert_match
local assert_same = assertions.assert_same

testing:test("assert_eq", function()
    assert_eq(1, 1)
    assert_eq(nil, nil)
    assert_eq(true, true)
    assert_eq("foo", "foo")

    local ok, err = pcall(assert_eq, 1, 2)
    assert(not ok)
    assert(err:match("assertion `left == right` failed!"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: 2"))

    ok, err = pcall(assert_eq, 1, nil, "custom message")
    assert(not ok)
    assert(err:match("assertion `left == right` failed: custom message"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: nil"))

    ok, err = pcall(assert_eq, "foo", "bar")
    assert(not ok)
    assert(err:match("  left: foo"))
    assert(err:match(" right: bar"))

    ok, err = pcall(assert_eq, {}, {})
    assert(not ok)
    assert(err:match("  left: table:"))
    assert(err:match(" right: table:"))
end)

testing:test("assert_ne", function()
    assert_ne(1, 2)
    assert_ne(nil, 1)
    assert_ne(true, false)
    assert_ne("foo", "bar")

    local ok, err = pcall(assert_ne, 1, 1)
    assert(not ok)
    assert(err:match("assertion `left ~= right` failed!"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: 1"))

    ok, err = pcall(assert_ne, nil, nil, "custom message")
    assert(not ok)
    assert(err:match("assertion `left ~= right` failed: custom message"))
    assert(err:match("  left: nil"))
    assert(err:match(" right: nil"))
end)

testing:test("assert_match", function()
    assert_match("hello world", "hello")
    assert_match("12345", "%d+")

    local ok, err = pcall(assert_match, "hello world", "bye")
    assert(not ok)
    assert(err:match("assertion `obj:match%(pattern%)` failed!"))
    assert(err:match("  pattern: bye"))
    assert(err:match("  obj: hello world"))

    ok, err = pcall(assert_match, "foo", "bar", "custom message")
    assert(not ok)
    assert(err:match("assertion `obj:match%(pattern%)` failed: custom message"))
    assert(err:match("  pattern: bar"))
    assert(err:match("  obj: foo"))
end)

testing:test("assert_same", function()
    assert_same(1, 1)
    assert_same(nil, nil)
    assert_same(true, true)
    assert_same("foo", "foo")
    assert_same({ 1, 2, 3 }, { 1, 2, 3 })
    assert_same({ foo = "bar" }, { foo = "bar" })

    local ok, err = pcall(assert_same, 1, 2)
    assert(not ok)
    assert(err:match("assertion `left ~ right` failed!"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: 2"))

    ok, err = pcall(assert_same, 1, nil, "custom message")
    assert(not ok)
    assert(err:match("assertion `left ~ right` failed: custom message"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: nil"))

    ok, err = pcall(assert_same, "foo", "bar")
    assert(not ok)
    assert(err:match("  left: foo"))
    assert(err:match(" right: bar"))

    ok, err = pcall(assert_same, { 1, 2, 3 }, { 1, 2, 4 })
    assert(not ok)
    assert(err:match("  left%[3%]: 3"))
    assert(err:match(" right%[3%]: 4"))

    ok, err = pcall(assert_same, { foo = "bar" }, { foo = "baz" })
    assert(not ok)
    assert(err:match('  left%["foo"%]: bar'))
    assert(err:match(' right%["foo"%]: baz'))

    -- Deep nested tables
    ok, err = pcall(assert_same, { foo = { bar = { baz = 1 } } }, { foo = { bar = { baz = function() end } } })
    assert(not ok)
    assert(err:match('  left%["foo"%]%["bar"%]%["baz"%]: 1'))
    assert(err:match(' right%["foo"%]%["bar"%]%["baz"%]: function:'))

    -- Recursion
    local t = {}
    t[1] = t
    ok, err = pcall(assert_same, t, { {} })
    assert(not ok)
    assert(err:match("  left%[1%]: table:"))
    assert(err:match(" right%[1%]: table:"))
end)
