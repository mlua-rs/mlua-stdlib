local deps = ...
local assertions = deps.assertions

local println, style = deps.println, deps.style
local instant = deps.instant

local Testing = {}
Testing.__index = Testing

function Testing.new(name)
    local self = setmetatable({}, Testing)
    self._name = name
    self._tests = {}
    self._hooks = {
        before_all = {},
        after_all = {},
        before_each = {},
        after_each = {},
    }
    self._results = {}
    return self
end

-- Test context passed to each test function
local TestContext = {}
TestContext.__index = TestContext

function TestContext.new(name)
    local self = setmetatable({}, TestContext)
    self.name = name
    return self
end

-- Forward assertion methods to the assertions module
function TestContext.assert_eq(a, b, msg)
    assertions.assert_eq(a, b, msg)
end

function TestContext.assert_ne(a, b, msg)
    assertions.assert_ne(a, b, msg)
end

function TestContext.assert_match(a, b, msg)
    assertions.assert_match(a, b, msg)
end

function TestContext.assert_same(a, b, msg)
    assertions.assert_same(a, b, msg)
end

function TestContext.assert(cond, msg)
    if not cond then
        if msg ~= nil then
            error("assertion failed: " .. tostring(msg), 2)
        else
            error("assertion failed!", 2)
        end
    end
end

-- Add convenience methods
function TestContext.skip(reason)
    error("__SKIP__: " .. (reason or "skipped"), 0)
end

-- Hooks registration
function Testing:before_all(func)
    table.insert(self._hooks.before_all, func)
end

function Testing:after_all(func)
    table.insert(self._hooks.after_all, func)
end

function Testing:before_each(func)
    table.insert(self._hooks.before_each, func)
end

function Testing:after_each(func)
    table.insert(self._hooks.after_each, func)
end

-- Tests registration
function Testing:test(name, func)
    table.insert(self._tests, { name = name, func = func })
end

-- Run a single test
function Testing:_run_single_test(test)
    local ctx = TestContext.new(test.name)
    local start_time = instant()
    local success, err = true, nil

    -- Run before_each hooks
    for _, func in ipairs(self._hooks.before_each) do
        local ok, hook_err = pcall(func)
        if not ok then
            return {
                name = test.name,
                passed = false,
                skipped = false,
                error = "before_each failed: " .. tostring(hook_err),
                duration = start_time:elapsed(),
            }
        end
    end

    -- Run the test
    local test_ok, test_err = pcall(test.func, ctx)
    if not test_ok then
        if type(test_err) == "string" and test_err:match("^__SKIP__:") then
            success, err = "skip", test_err:match("^__SKIP__: (.*)")
        else
            success, err = false, test_err
        end
    end

    -- Run after_each hooks (even if test failed)
    for _, func in ipairs(self._hooks.after_each) do
        local ok, hook_err = pcall(func)
        if not ok then
            return {
                name = test.name,
                passed = false,
                skipped = false,
                error = "after_each failed: " .. tostring(hook_err),
                duration = start_time:elapsed(),
            }
        end
    end

    return {
        name = test.name,
        passed = success == true,
        skipped = success == "skip",
        error = err,
        duration = start_time:elapsed(),
    }
end

-- Run all tests
function Testing:run(opts)
    opts = opts or {}
    local pattern = opts.pattern
    self._results = {}
    local start_time = instant()

    -- Run before_all hooks
    for _, func in ipairs(self._hooks.before_all) do
        func()
    end

    -- Run tests
    for _, test in ipairs(self._tests) do
        if not pattern or test.name:find(pattern) then
            local result = self:_run_single_test(test)
            table.insert(self._results, result)
        end
    end

    -- Run after_all hooks
    for _, func in ipairs(self._hooks.after_all) do
        func()
    end

    self._results.duration = start_time:elapsed()

    -- Print results unless quiet
    if not opts.quiet then
        self:_print_results()
    end

    -- Return success status
    local failed = 0
    for _, result in ipairs(self._results) do
        if not result.passed and not result.skipped then
            failed = failed + 1
        end
    end

    return failed == 0, self._results
end

function Testing:_print_results()
    local passed, failed, skipped = 0, 0, 0

    for _, result in ipairs(self._results) do
        local status = style(result.passed and "✓" or (result.skipped and "⊝" or "✗"))
        status:color(result.passed and "green" or (result.skipped and "yellow" or "red"))

        println(status, result.name)
        if result.error then
            println(tostring(result.error))
        end

        if result.passed then
            passed = passed + 1
        elseif result.skipped then
            skipped = skipped + 1
        else
            failed = failed + 1
        end
    end

    local total = passed + failed + skipped
    if total == 0 then
        -- No tests were run
        return
    end

    local prefix = "test results:"
    if self._name then
        prefix = string.format("`%s` %s", self._name, prefix)
    end
    local duration = self._results.duration
    local stats = string.format(
        "%d passed, %d failed, %d skipped (%d total finished in %s)",
        passed,
        failed,
        skipped,
        total,
        tostring(duration)
    )
    println()
    println(prefix, stats)
end

-- Get results for the Rust integration
function Testing:results()
    return self._results
end

return Testing
