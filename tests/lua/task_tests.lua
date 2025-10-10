local task = require("@task")

testing:test("task spawn", function(t)
    local task_h = task.spawn(function(delay)
        task.sleep(delay)
        return "done"
    end, "10ms")
    t.assert_ne(task_h.id, nil, "task id should be set")
    t.assert_eq(task_h.name, nil, "task name should be nil by default")

    task.sleep("5ms")
    local elapsed = task_h:elapsed():as_secs()
    t.assert(elapsed >= 0.005 and elapsed < 0.01, "elapsed time should be around 5ms")
    task.sleep("10ms")
    t.assert(task_h:is_finished(), "task should be finished after total 10ms")
    local result = task_h:join()
    t.assert_eq(result, "done", "task result should be 'done'")

    -- One more join
    local _, err = task_h:join()
    t.assert_match(err, "task already joined")
end)

testing:test("task abort", function(t)
    local task_h = task.spawn(function()
        task.sleep("50ms")
        return "should not reach here"
    end)

    task.sleep("5ms")
    task_h:abort()
    task.sleep("5ms")
    t.assert(task_h:is_finished(), "task should be finished after abort")
    local elapsed1 = task_h:elapsed():as_secs()
    t.assert(elapsed1 >= 0.005 and elapsed1 < 0.01, "elapsed time should be around 5ms")
    task.sleep("5ms")
    local elapsed2 = task_h:elapsed():as_secs()
    t.assert_eq(elapsed1, elapsed2, "elapsed time should not change after abort")

    local _, err = task_h:join()
    t.assert_match(err, "task %d+ was cancelled")
end)

testing:test("task error", function(t)
    local task_h = task.spawn(function()
        error("task error occurred")
    end)

    task.sleep("10ms")
    t.assert(task_h:is_finished(), "task should be finished after error")
    local _, err = task_h:join()
    t.assert_match(err, "task error occurred")
end)

testing:test("task spawn_every", function(t)
    local count = 0
    local task_h = task.spawn_every("10ms", function()
        count = count + 1
    end)

    task.sleep("35ms")
    task_h:abort()
    t.assert_eq(count, 3, "task should have run 3 times")
    local _, err = task_h:join()
    t.assert_match(err, "task %d+ was cancelled")
end)

testing:test("task spawn with timeout", function(t)
    local task_h = task.spawn(task.create(function()
        task.yield()
        task.sleep("50ms")
        return "should not reach here"
    end, { name = "my_task", timeout = "20ms" }))

    t.assert_ne(task_h.id, nil, "task id should be set")
    t.assert_eq(task_h.name, "my_task")
    task.sleep("30ms")
    t.assert(task_h:is_finished(), "task should be finished after timeout")
    local elapsed = task_h:elapsed():as_secs()
    t.assert(elapsed >= 0.02 and elapsed < 0.03, "elapsed time should be around 20ms")
    local _, err = task_h:join()
    t.assert_match(err, "task exceeded timeout")
end)

testing:test("task spawn_every with timeout", function(t)
    local count = 0
    local task_h = task.spawn_every(
        "10ms",
        task.create(function()
            count = count + 1
        end, { name = "my_task", timeout = "35ms" })
    )

    t.assert_ne(task_h.id, nil, "task id should be set")
    t.assert_eq(task_h.name, "my_task")
    task.sleep("50ms")
    t.assert(task_h:is_finished(), "task should be finished after timeout")
    local elapsed = task_h:elapsed():as_secs()
    t.assert(elapsed >= 0.035 and elapsed < 0.04, "elapsed time should be around 35ms")
    local _, err = task_h:join()
    t.assert_match(err, "task exceeded timeout")
end)

testing:test("task group", function(t)
    local group = task.group()

    local task1 = group:spawn(function()
        task.sleep("10ms")
        return "task1 done"
    end)
    local task2 = group:spawn(function()
        task.sleep("20ms")
        return "task2 done"
    end)

    t.assert_eq(#group, 2, "group should have 2 tasks")
    task.sleep("15ms")
    t.assert(task1:is_finished(), "task1 should be finished after 15ms")
    t.assert(task1:elapsed():as_secs() >= 0.01, "task1 elapsed should be at least 10ms")
    t.assert(not task2:is_finished(), "task2 should not be finished after 15ms")

    local results = group:join_all()
    t.assert_same(results, { "task1 done", "task2 done" }, "group results should match")
    t.assert_eq(#group, 0, "group should have 0 tasks after joins")
end)

testing:test("task group with abort", function(t)
    local group = task.group()

    group:spawn(function()
        task.sleep("10ms")
        return "task1 done"
    end)
    local task2 = group:spawn(function()
        task.sleep("20ms")
        return "task2 done"
    end)

    task.sleep("15ms")
    task2:abort()

    local results = group:join_all()
    t.assert_eq(results[1], "task1 done", "task1 result should match")
    t.assert_match(results[2], "task %d+ was cancelled")
end)
