local assertions = require("@assertions")

local assert_eq = assertions.assert_eq
local assert_ne = assertions.assert_ne
local assert_lt = assertions.assert_lt
local assert_gt = assertions.assert_gt
local assert_le = assertions.assert_le
local assert_ge = assertions.assert_ge
local assert_match = assertions.assert_match
local assert_same = assertions.assert_same
local assert_starts_with = assertions.assert_starts_with
local assert_ends_with = assertions.assert_ends_with
local assert_contains = assertions.assert_contains

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

testing:test("assert_lt", function()
    assert_lt(1, 2)
    assert_lt(-1, 0)

    local ok, err = pcall(assert_lt, 1, 1)
    assert(not ok)
    assert(err:match("assertion `left < right` failed!"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: 1"))

    ok, err = pcall(assert_lt, 2, 1, "custom message")
    assert(not ok)
    assert(err:match("assertion `left < right` failed: custom message"))
    assert(err:match("  left: 2"))
    assert(err:match(" right: 1"))
end)

testing:test("assert_gt", function()
    assert_gt(2, 1)
    assert_gt(0, -1)

    local ok, err = pcall(assert_gt, 1, 1)
    assert(not ok)
    assert(err:match("assertion `left > right` failed!"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: 1"))

    ok, err = pcall(assert_gt, 1, 2, "custom message")
    assert(not ok)
    assert(err:match("assertion `left > right` failed: custom message"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: 2"))
end)

testing:test("assert_le", function()
    assert_le(1, 2)
    assert_le(1, 1)

    local ok, err = pcall(assert_le, 2, 1)
    assert(not ok)
    assert(err:match("assertion `left <= right` failed!"))
    assert(err:match("  left: 2"))
    assert(err:match(" right: 1"))

    ok, err = pcall(assert_le, 2, 1, "custom message")
    assert(not ok)
    assert(err:match("assertion `left <= right` failed: custom message"))
    assert(err:match("  left: 2"))
    assert(err:match(" right: 1"))
end)

testing:test("assert_ge", function()
    assert_ge(2, 1)
    assert_ge(1, 1)

    local ok, err = pcall(assert_ge, 1, 2)
    assert(not ok)
    assert(err:match("assertion `left >= right` failed!"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: 2"))

    ok, err = pcall(assert_ge, 1, 2, "custom message")
    assert(not ok)
    assert(err:match("assertion `left >= right` failed: custom message"))
    assert(err:match("  left: 1"))
    assert(err:match(" right: 2"))
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

testing:test("assert_starts_with", function()
    assert_starts_with("hello world", "hello")
    assert_starts_with("hello world", "")

    local ok, err = pcall(assert_starts_with, "hello world", "bye")
    assert(not ok)
    assert(err:match("assertion `str:starts_with%(prefix%)` failed!"))
    assert(err:match("  prefix: bye"))
    assert(err:match("     str: hello world"))

    ok, err = pcall(assert_starts_with, "foo", "bar", "custom message")
    assert(not ok)
    assert(err:match("assertion `str:starts_with%(prefix%)` failed: custom message"))
    assert(err:match("  prefix: bar"))
    assert(err:match("     str: foo"))
end)

testing:test("assert_ends_with", function()
    assert_ends_with("hello world", "world")
    assert_ends_with("hello world", "")

    local ok, err = pcall(assert_ends_with, "hello world", "bye")
    assert(not ok)
    assert(err:match("assertion `str:ends_with%(suffix%)` failed!"))
    assert(err:match("  suffix: bye"))
    assert(err:match("     str: hello world"))

    ok, err = pcall(assert_ends_with, "foo", "bar", "custom message")
    assert(not ok)
    assert(err:match("assertion `str:ends_with%(suffix%)` failed: custom message"))
    assert(err:match("  suffix: bar"))
    assert(err:match("     str: foo"))
end)

testing:test("assert_contains", function()
    assert_contains("hello world", "world")
    assert_contains("hello world", "hello")
    assert_contains("hello world", "o w")

    local ok, err = pcall(assert_contains, "hello world", "bye")
    assert(not ok)
    assert(err:match("assertion `str:contains%(substr%)` failed!"))
    assert(err:match("  substr: bye"))
    assert(err:match("     str: hello world"))

    ok, err = pcall(assert_contains, "foo", "bar", "custom message")
    assert(not ok)
    assert(err:match("assertion `str:contains%(substr%)` failed: custom message"))
    assert(err:match("  substr: bar"))
    assert(err:match("     str: foo"))
end)
