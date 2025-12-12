local time = require("@time")

testing:test("Instant", function(t)
    local now = time.Instant.now()
    t.assert_eq(now, now)

    local future = now + 1
    t.assert_lt(now, future, "now should be less than future")
    t.assert_le(now, future, "now should be less than or equal to future")

    local diff = future - now
    t.assert_eq(diff:as_secs(), 1.0)
    t.assert_eq(diff:as_millis(), 1000)
    t.assert_eq(diff:as_micros(), 1000000)

    local shifted = future - "250ms"
    local expected = now + 0.75
    t.assert_eq(shifted, expected)
end)

testing:test("Duration", function(t)
    local now = time.Instant.now()
    local quarter = (now + "0.25s") - now
    t.assert_eq(quarter:as_secs(), 0.25)

    local half = quarter + quarter
    t.assert_eq(half:as_millis(), 500)

    local remainder = half - quarter
    t.assert_eq(remainder, quarter)

    local elapsed = now:elapsed()
    t.assert_gt(elapsed:as_secs(), 0.0, "elapsed time should be non-negative")
end)
