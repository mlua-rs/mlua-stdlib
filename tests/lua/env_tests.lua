local env = require("@env")

testing:test("current_dir", function(t)
    local dir, err = env.current_dir()
    t.assert_eq(err, nil)
    t.assert(type(dir) == "string", "current_dir should return a string")
    t.assert(#dir > 0, "current_dir should not be empty")
end)

testing:test("set_current_dir", function(t)
    if env.FAMILY ~= "unix" then
        t.skip("Skipping set_current_dir test on Windows")
    end

    local original_dir = env.current_dir()
    local parent_dir = original_dir .. "/.."
    local ok, err1 = env.set_current_dir(parent_dir)
    t.assert_eq(err1, nil)
    t.assert_eq(ok, true)

    -- Verify the directory changed
    local new_dir, err2 = env.current_dir()
    t.assert_eq(err2, nil)
    t.assert(new_dir ~= original_dir, "directory should have changed")

    -- Change back to original directory
    local _, err3 = env.set_current_dir(original_dir)
    t.assert_eq(err3, nil)

    -- Test invalid directory
    local _, err5 = env.set_current_dir("/nonexistent/directory/path")
    t.assert(err5 ~= nil, "should fail for nonexistent directory")
    t.assert(type(err5) == "string", "error should be a string")
end)

testing:test("current_exe", function(t)
    local exe, err = env.current_exe()
    t.assert_eq(err, nil)
    t.assert(type(exe) == "string", "current_exe should return a string")
    t.assert(#exe > 0, "current_exe should not be empty")
    -- The executable path should be a valid path (contains forward slash on Unix systems)
    if env.FAMILY == "unix" then
        t.assert(exe:match("/"), "executable should be a full path")
    end
end)

testing:test("home_dir", function(t)
    local home = env.home_dir()
    -- home_dir can return nil if home directory is not known
    if home ~= nil then
        t.assert(type(home) == "string", "home_dir should return a string when available")
        t.assert(#home > 0, "home_dir should not be empty when available")
    end
end)

testing:test("var", function(t)
    -- Test getting a variable that likely doesn't exist
    local value = env.var("MLUA_STDLIB_NONEXISTENT_VAR")
    t.assert_eq(value, nil, "nonexistent variable should return nil")

    -- Test getting PATH (should exist on most systems)
    local path = env.var("PATH")
    t.assert(type(path) == "string", "PATH should be a string")
    t.assert(#path > 0, "PATH should not be empty")
end)

testing:test("set_var", function(t)
    local test_key = "MLUA_STDLIB_TEST_VAR"
    local test_value = "test_value_123"

    -- Ensure the variable doesn't exist initially
    local initial = env.var(test_key)
    t.assert_eq(initial, nil, "test variable should not exist initially")

    -- Set the variable
    env.set_var(test_key, test_value)
    t.assert_eq(env.var(test_key), test_value, "variable should be set correctly")

    -- Update the variable
    local new_value = "updated_value_456"
    env.set_var(test_key, new_value)
    t.assert_eq(env.var(test_key), new_value, "variable should be updated correctly")

    -- Remove the variable
    env.set_var(test_key, nil)
    t.assert_eq(env.var(test_key), nil, "variable should be removed when set to nil")
end)

testing:test("vars", function(t)
    local all_vars = env.vars()
    t.assert(type(all_vars) == "table", "vars should return a table")

    -- Check that common environment variables exist
    local path = all_vars["PATH"]
    t.assert(type(path) == "string", "PATH in vars should be a string")

    -- Set a test variable and verify it appears in vars
    local test_key = "MLUA_STDLIB_TEST_VARS"
    local test_value = "test_vars_value"
    env.set_var(test_key, test_value)
    t.assert_eq(env.vars()[test_key], test_value, "test variable should appear in vars")

    -- Clean up
    env.set_var(test_key, nil)
    t.assert_eq(env.vars()[test_key], nil, "test variable should be removed from vars")
end)
