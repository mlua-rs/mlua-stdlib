local regex = require("@regex")

-- Test basic regex functionality
testing:test("regex_basic", function(t)
    local re = regex.new(".*(?P<gr1>abc)")

    t.assert(re:is_match("123abc321"), "is_match() should have matches")
    t.assert(not re:is_match("123"), "is_match() should not have matches")

    local matches = re:match("123abc321")
    t.assert_eq(matches[0], "123abc", "zero capture group should match the whole text")
    t.assert_eq(matches[1], "abc", "first capture group should match `abc`")
    t.assert_eq(matches["gr1"], "abc", "named capture group should match `abc`")
    t.assert_eq(matches[true], nil, "bad key should have no match")

    -- Test split
    local re_split = regex.new("[,.]")
    local vec = re_split:split("abc.qwe,rty.asd")
    t.assert_eq(#vec, 4, "vec len should be 4")
    t.assert(
        vec[1] == "abc" and vec[2] == "qwe" and vec[3] == "rty" and vec[4] == "asd",
        "vec must be 'abc', 'qwe', 'rty', 'asd'"
    )

    vec = re_split:splitn("abc,bcd,cde", 2)
    t.assert_eq(#vec, 2, "vec len should be 2")
    t.assert(vec[1] == "abc" and vec[2] == "bcd,cde", "vec must be 'abc', 'bcd,cde'")

    -- Test invalid regex
    local re_invalid, err = regex.new("(")
    t.assert_eq(re_invalid, nil, "re is not nil")
    t.assert(string.find(err, "regex parse error"), "err must contain 'regex parse error'")

    -- Test replace
    local re_replace = regex.new("(?P<last>[^,\\s]+),\\s+(?P<first>\\S+)")
    local str = re_replace:replace("Smith, John", "$first $last")
    t.assert_eq(str, "John Smith", "str must be 'John Smith'")
end)

-- Test regex shortcuts (escape, is_match, match functions)
testing:test("regex_shortcuts", function(t)
    -- Test escape
    t.assert_eq(regex.escape("a*b"), "a\\*b", "escaped regex must be 'a\\*b'")

    -- Test "is_match"
    t.assert(regex.is_match("\\b\\w{13}\\b", "I categorically deny having ..."), "is_match should have matches")
    t.assert(not regex.is_match("abc", "bca"), "is_match should not have matches")
    local is_match, err = regex.is_match("(", "")
    t.assert(is_match == nil and string.find(err, "regex parse error") ~= nil, "is_match should return error")

    -- Test "match"
    local matches = regex.match("^(\\d{4})-(\\d{2})-(\\d{2})$", "2014-05-01")
    t.assert_eq(matches[0], "2014-05-01", "zero capture group should match the whole text")
    t.assert_eq(matches[1], "2014", "first capture group should match year")
    t.assert_eq(matches[2], "05", "second capture group should match month")
    t.assert_eq(matches[3], "01", "third capture group should match day")
    matches, err = regex.match("(", "")
    t.assert(matches == nil and string.find(err, "regex parse error") ~= nil, "match should return error")
end)

-- Test RegexSet functionality
testing:test("regex_set", function(t)
    local set = regex.RegexSet.new({ "\\w+", "\\d+", "\\pL+", "foo", "bar", "barfoo", "foobar" })
    t.assert_eq(set:len(), 7, "len should be 7")
    t.assert(set:is_match("foobar"), "is_match should have matches")
    t.assert_eq(table.concat(set:matches("foobar"), ","), "1,3,4,5,7", "matches should return 1,3,4,5,7")
end)

-- Test capture locations
testing:test("capture_locations", function(t)
    local re = regex.new("\\d+(abc)\\d+")

    local str = "123abc321"
    local locs = re:captures_read(str)
    t.assert(locs, "locs is nil")
    t.assert_eq(locs:len(), 2, "locs len is not 2")
    local i, j = locs:get(0)
    t.assert(i == 1 and j == 9, "locs:get(0) is not 1, 9")
    i, j = locs:get(1)
    t.assert(i == 4 and j == 6, "locs:get(1) is not 4, 6")
    t.assert_eq(str:sub(i, j), "abc", "str:sub(i, j) is not 'abc'")
    t.assert_eq(locs:get(2), nil, "locs:get(2) is nil")

    -- Test no match
    locs = re:captures_read("123")
    t.assert_eq(locs, nil, "locs is not nil")
end)
