local opts = ... or {}
local assertions = {}

function assertions.assert_eq(left, right, message)
    if left ~= right then
        if message ~= nil then
            message = string.format("assertion `left == right` failed: %s", tostring(message))
        else
            message = "assertion `left == right` failed!"
        end
        local frame_level = opts.level or 2
        error(string.format("%s\n  left: %s\n right: %s", message, tostring(left), tostring(right)), frame_level)
    end
end

function assertions.assert_ne(left, right, message)
    if left == right then
        if message ~= nil then
            message = string.format("assertion `left ~= right` failed: %s", tostring(message))
        else
            message = "assertion `left ~= right` failed!"
        end
        local frame_level = opts.level or 2
        error(string.format("%s\n  left: %s\n right: %s", message, tostring(left), tostring(right)), frame_level)
    end
end

local function next_level(level, k)
    if type(k) == "string" then
        return level .. '["' .. k .. '"]'
    else
        return level .. "[" .. tostring(k) .. "]"
    end
end

local function deepcmp(left, right, level, visited, report_cb)
    if type(left) ~= type(right) then
        report_cb(left, right, level)
        return false
    end
    if rawequal(left, right) then
        return true
    end
    if type(left) ~= "table" then
        report_cb(left, right, level)
        return false
    end

    -- Prevent recursion
    if visited[left] or visited[right] then
        report_cb(left, right, level)
        return false
    end

    -- Iterate over all keys in left and right, and compare their values recursively.
    visited[left] = true
    for k, v in next, left do
        if not deepcmp(v, rawget(right, k), next_level(level, k), visited, report_cb) then
            return false
        end
    end
    visited[left] = nil

    visited[right] = true
    for k, v in next, right do
        if not deepcmp(rawget(left, k), v, next_level(level, k), visited, report_cb) then
            return false
        end
    end
    visited[right] = nil

    return true
end

function assertions.assert_same(left, right, message)
    local left_v, right_v, level
    local function report_cb(lv, rv, l)
        left_v, right_v, level = lv, rv, l
    end
    if not deepcmp(left, right, "", {}, report_cb) then
        if message ~= nil then
            message = string.format("assertion `left ~ right` failed: %s", tostring(message))
        else
            message = "assertion `left ~ right` failed!"
        end
        local error_msg =
            string.format("%s\n  left%s: %s\n right%s: %s", message, level, tostring(left_v), level, tostring(right_v))
        local frame_level = opts.level or 2
        error(error_msg, frame_level)
    end
end

return assertions
