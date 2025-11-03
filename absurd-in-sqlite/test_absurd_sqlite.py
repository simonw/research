"""
Tests for Absurd-in-SQLite
"""

import pytest
import time
import threading
from datetime import datetime, timedelta
from absurd_sqlite import AbsurdSQLite, TaskContext


def test_create_queue():
    """Test queue creation"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("test_queue")

    # Verify queue exists
    conn = absurd._get_conn()
    cursor = conn.execute("SELECT queue_name FROM queues WHERE queue_name = ?", ("test_queue",))
    assert cursor.fetchone() is not None
    conn.close()


def test_spawn_task():
    """Test task spawning"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    task_id = absurd.spawn("default", "test_task", {"value": 42})

    # Verify task was created
    status = absurd.get_task_status("default", task_id)
    assert status is not None
    assert status['task_name'] == "test_task"
    assert status['params']['value'] == 42
    assert status['state'] == "pending"


def test_simple_task_execution():
    """Test execution of a simple task with steps"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    results = []

    def my_task(params, ctx: TaskContext):
        # Step 1
        result1 = ctx.step("step1", lambda: params['x'] * 2)
        results.append(('step1', result1))

        # Step 2
        result2 = ctx.step("step2", lambda: result1 + 10)
        results.append(('step2', result2))

        return result2

    absurd.register_task("my_task", my_task)

    # Spawn and execute
    task_id = absurd.spawn("default", "my_task", {"x": 5})
    task_data = absurd.claim_task("default")
    assert task_data is not None

    completed = absurd.process_task("default", task_data)
    assert completed is True

    # Check results
    assert results == [('step1', 10), ('step2', 20)]

    # Check final status
    status = absurd.get_task_status("default", task_id)
    assert status['state'] == "completed"
    assert status['completed_payload'] == 20


def test_checkpoint_recovery():
    """Test that checkpoints allow resuming from where we left off"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    execution_log = []

    def failing_task(params, ctx: TaskContext):
        # Step 1 - will succeed
        result1 = ctx.step("step1", lambda: execution_log.append("step1") or 10)

        # Step 2 - will succeed
        result2 = ctx.step("step2", lambda: execution_log.append("step2") or result1 + 5)

        # Step 3 - will fail on first attempt
        if ctx.attempt == 1:
            execution_log.append("step3_fail")
            raise Exception("Intentional failure")

        # On retry, step3 succeeds
        result3 = ctx.step("step3", lambda: execution_log.append("step3_success") or result2 * 2)

        return result3

    absurd.register_task("failing_task", failing_task)

    # Spawn task
    task_id = absurd.spawn("default", "failing_task", {}, max_attempts=2)

    # First execution - will fail after step2
    task_data = absurd.claim_task("default")
    completed = absurd.process_task("default", task_data)
    assert completed is False

    # Check execution log - steps 1 and 2 ran, then failed
    assert execution_log == ["step1", "step2", "step3_fail"]

    # Clear log for retry
    execution_log.clear()

    # Wait a bit for retry to be available (no delay configured, should be immediate)
    time.sleep(0.1)

    # Second execution - should resume and complete
    task_data = absurd.claim_task("default")
    assert task_data is not None
    assert task_data['attempt'] == 2

    completed = absurd.process_task("default", task_data)
    assert completed is True

    # Steps 1 and 2 should NOT re-execute (checkpointed)
    # Only step3 should execute
    assert execution_log == ["step3_success"]

    # Check final result
    status = absurd.get_task_status("default", task_id)
    assert status['state'] == "completed"
    assert status['completed_payload'] == 30  # (10 + 5) * 2


def test_retry_with_exponential_backoff():
    """Test retry with exponential backoff strategy"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    attempts = []

    def flaky_task(params, ctx: TaskContext):
        attempts.append(ctx.attempt)
        if ctx.attempt < 3:
            raise Exception("Not ready yet")
        return "success"

    absurd.register_task("flaky_task", flaky_task)

    # Spawn with exponential backoff
    task_id = absurd.spawn(
        "default",
        "flaky_task",
        {},
        max_attempts=3,
        retry_strategy={'kind': 'exponential', 'base_seconds': 0.1, 'factor': 2}
    )

    # First attempt
    task_data = absurd.claim_task("default")
    absurd.process_task("default", task_data)
    assert attempts == [1]

    # Should not be available immediately
    task_data = absurd.claim_task("default")
    assert task_data is None

    # Wait for retry (0.1 seconds)
    time.sleep(0.15)

    # Second attempt
    task_data = absurd.claim_task("default")
    assert task_data is not None
    assert task_data['attempt'] == 2
    absurd.process_task("default", task_data)
    assert attempts == [1, 2]

    # Wait for second retry (0.2 seconds due to exponential backoff)
    time.sleep(0.25)

    # Third attempt - should succeed
    task_data = absurd.claim_task("default")
    assert task_data is not None
    assert task_data['attempt'] == 3
    completed = absurd.process_task("default", task_data)
    assert completed is True
    assert attempts == [1, 2, 3]

    status = absurd.get_task_status("default", task_id)
    assert status['state'] == "completed"


def test_max_attempts_exhausted():
    """Test that task fails when max_attempts is exhausted"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    def always_fails(params, ctx: TaskContext):
        raise Exception("Always fails")

    absurd.register_task("always_fails", always_fails)

    task_id = absurd.spawn("default", "always_fails", {}, max_attempts=2)

    # First attempt
    task_data = absurd.claim_task("default")
    absurd.process_task("default", task_data)

    # Second attempt
    time.sleep(0.1)
    task_data = absurd.claim_task("default")
    assert task_data['attempt'] == 2
    absurd.process_task("default", task_data)

    # Should be no more retries
    time.sleep(0.1)
    task_data = absurd.claim_task("default")
    assert task_data is None

    # Task should be in failed state
    status = absurd.get_task_status("default", task_id)
    assert status['state'] == "failed"


def test_sleep():
    """Test task sleep functionality"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    execution_count = []

    def sleeping_task(params, ctx: TaskContext):
        # This code runs every time (outside of steps), so we use a step to track first execution
        first_run_time = ctx.step("before_sleep", lambda: datetime.now().isoformat())

        # Sleep for 0.2 seconds
        ctx.sleep(0.2)

        # After sleep - put in a step to avoid re-execution
        after_sleep_result = ctx.step("after_sleep", lambda: {
            "message": "after_sleep",
            "time": datetime.now().isoformat()
        })

        return after_sleep_result['message']

    absurd.register_task("sleeping_task", sleeping_task)

    task_id = absurd.spawn("default", "sleeping_task", {})

    # First execution - should suspend at sleep
    task_data = absurd.claim_task("default")
    start_time = datetime.now()
    completed = absurd.process_task("default", task_data)
    assert completed is False  # Task suspended

    # Should not be available immediately
    task_data = absurd.claim_task("default")
    assert task_data is None

    # Wait for sleep duration
    time.sleep(0.25)

    # Should be available now
    task_data = absurd.claim_task("default")
    assert task_data is not None

    # Resume execution
    completed = absurd.process_task("default", task_data)
    assert completed is True

    status = absurd.get_task_status("default", task_id)
    assert status['state'] == "completed"
    assert status['completed_payload'] == "after_sleep"


def test_event_emit_and_await():
    """Test event emission and waiting"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    results = []

    def waiting_task(params, ctx: TaskContext):
        # Code outside steps will re-execute on resume
        # Use a step to track execution
        ctx.step("mark_started", lambda: results.append("started") or True)

        # Wait for event
        event_data = ctx.await_event("test_event")

        # Process the event in a step so it doesn't re-execute
        result = ctx.step("process_event", lambda: {
            "log": results.append(f"received: {event_data}"),
            "value": event_data['value']
        })

        return result['value']

    absurd.register_task("waiting_task", waiting_task)

    task_id = absurd.spawn("default", "waiting_task", {})

    # Start task - it will suspend waiting for event
    task_data = absurd.claim_task("default")
    completed = absurd.process_task("default", task_data)
    assert completed is False

    assert results == ["started"]

    # Task should not be claimable (waiting for event)
    task_data = absurd.claim_task("default")
    assert task_data is None

    # Emit the event
    absurd.emit_event("default", "test_event", {"value": 42})

    # Now task should be available
    task_data = absurd.claim_task("default")
    assert task_data is not None

    # Resume task
    completed = absurd.process_task("default", task_data)
    assert completed is True

    assert results == ["started", "received: {'value': 42}"]

    status = absurd.get_task_status("default", task_id)
    assert status['state'] == "completed"
    assert status['completed_payload'] == 42


def test_event_already_emitted():
    """Test that tasks can receive events that were emitted before they waited"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    # Emit event BEFORE task starts
    absurd.emit_event("default", "early_event", {"message": "I'm early!"})

    results = []

    def late_waiter(params, ctx: TaskContext):
        # This should receive the event immediately since it was already emitted
        event_data = ctx.await_event("early_event")
        results.append(event_data)
        return "done"

    absurd.register_task("late_waiter", late_waiter)

    task_id = absurd.spawn("default", "late_waiter", {})

    # Task should complete in one execution (event already available)
    task_data = absurd.claim_task("default")
    completed = absurd.process_task("default", task_data)
    assert completed is True

    assert results == [{"message": "I'm early!"}]

    status = absurd.get_task_status("default", task_id)
    assert status['state'] == "completed"


def test_multiple_tasks_waiting_for_same_event():
    """Test that multiple tasks can wait for the same event"""
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("default")

    results = []

    def waiter_task(params, ctx: TaskContext):
        event_data = ctx.await_event("shared_event")
        results.append((params['id'], event_data['value']))
        return event_data['value']

    absurd.register_task("waiter_task", waiter_task)

    # Spawn multiple tasks
    task_id1 = absurd.spawn("default", "waiter_task", {"id": 1})
    task_id2 = absurd.spawn("default", "waiter_task", {"id": 2})
    task_id3 = absurd.spawn("default", "waiter_task", {"id": 3})

    # Start all tasks - they will all suspend
    for _ in range(3):
        task_data = absurd.claim_task("default")
        absurd.process_task("default", task_data)

    # Emit the event
    absurd.emit_event("default", "shared_event", {"value": 99})

    # All tasks should now be available
    completed_count = 0
    for _ in range(3):
        task_data = absurd.claim_task("default")
        if task_data:
            completed = absurd.process_task("default", task_data)
            if completed:
                completed_count += 1

    assert completed_count == 3
    assert len(results) == 3
    assert all(value == 99 for _, value in results)


def test_order_fulfillment_example():
    """
    Test a realistic order fulfillment workflow similar to the Absurd README example.

    Note: In durable execution, code outside of steps re-executes on resume.
    To avoid duplicate logging, we put logging inside steps.
    """
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("orders")

    workflow_log = []

    def order_fulfillment(params, ctx: TaskContext):
        # Process payment - include logging in the step
        payment = ctx.step("process-payment", lambda: {
            "payment_id": f"pay_{params['order_id']}",
            "amount": params['amount'],
            "log": workflow_log.append(f"Payment processed: pay_{params['order_id']}")
        })

        # Reserve inventory - include logging in the step
        inventory = ctx.step("reserve-inventory", lambda: {
            "reserved": True,
            "items": params['items'],
            "log": workflow_log.append(f"Inventory reserved: {params['items']}")
        })

        # Wait for shipment packed event
        shipment = ctx.await_event(f"shipment.packed:{params['order_id']}")

        # Log shipment in a step
        ctx.step("log-shipment", lambda: workflow_log.append(
            f"Shipment packed: {shipment['tracking_number']}"
        ))

        # Send notification - include logging in the step
        notification = ctx.step("send-notification", lambda: {
            "sent": True,
            "email": params['email'],
            "log": workflow_log.append(f"Notification sent to: {params['email']}")
        })

        return {
            "order_id": payment['payment_id'],
            "tracking_number": shipment['tracking_number']
        }

    absurd.register_task("order-fulfillment", order_fulfillment)

    # Start order
    task_id = absurd.spawn("orders", "order-fulfillment", {
        "order_id": "42",
        "amount": 9999,
        "items": ["widget-1", "gadget-2"],
        "email": "customer@example.com"
    })

    # Process first part (payment and inventory)
    task_data = absurd.claim_task("orders")
    completed = absurd.process_task("orders", task_data)
    assert completed is False  # Suspended waiting for shipment

    # Verify first two steps completed
    assert len(workflow_log) == 2
    assert "Payment processed" in workflow_log[0]
    assert "Inventory reserved" in workflow_log[1]

    # Simulate warehouse packing the shipment
    absurd.emit_event("orders", "shipment.packed:42", {
        "tracking_number": "TRACK-12345"
    })

    # Resume workflow
    task_data = absurd.claim_task("orders")
    assert task_data is not None
    completed = absurd.process_task("orders", task_data)
    assert completed is True

    # Verify all steps completed
    assert len(workflow_log) == 4
    assert "Shipment packed: TRACK-12345" in workflow_log[2]
    assert "Notification sent" in workflow_log[3]

    # Check final result
    status = absurd.get_task_status("orders", task_id)
    assert status['state'] == "completed"
    assert status['completed_payload']['tracking_number'] == "TRACK-12345"


def test_agent_loop_pattern():
    """
    Test an agent loop pattern with multiple iterations and checkpoints.
    """
    absurd = AbsurdSQLite(":memory:")
    absurd.create_queue("agents")

    iteration_log = []

    def agent_task(params, ctx: TaskContext):
        messages = [{"role": "user", "content": params['prompt']}]

        for i in range(5):
            # Each iteration gets its own checkpoint
            step_name = "iteration" if i == 0 else f"iteration#{i+1}"

            result = ctx.step(step_name, lambda i=i: {
                "iteration": i + 1,
                "response": f"Response {i + 1}"
            })

            iteration_log.append(result)
            messages.append({"role": "assistant", "content": result['response']})

            # Stop if we've reached iteration 3
            if result['iteration'] >= 3:
                break

        return {"messages": messages, "iterations": len(iteration_log)}

    absurd.register_task("agent-task", agent_task)

    task_id = absurd.spawn("agents", "agent-task", {
        "prompt": "Hello, agent!"
    })

    # Execute task
    task_data = absurd.claim_task("agents")
    completed = absurd.process_task("agents", task_data)
    assert completed is True

    # Should have completed 3 iterations
    assert len(iteration_log) == 3
    assert iteration_log[0]['iteration'] == 1
    assert iteration_log[1]['iteration'] == 2
    assert iteration_log[2]['iteration'] == 3

    status = absurd.get_task_status("agents", task_id)
    assert status['state'] == "completed"
    assert status['completed_payload']['iterations'] == 3


def test_concurrent_workers():
    """Test multiple workers processing tasks concurrently"""
    absurd = AbsurdSQLite("test_concurrent.db")  # Use file db for thread safety
    absurd.create_queue("default")

    results = []
    results_lock = threading.Lock()

    def concurrent_task(params, ctx: TaskContext):
        time.sleep(0.1)  # Simulate work
        with results_lock:
            results.append(params['id'])
        return params['id']

    absurd.register_task("concurrent_task", concurrent_task)

    # Spawn multiple tasks
    for i in range(5):
        absurd.spawn("default", "concurrent_task", {"id": i})

    # Process with multiple threads
    def worker():
        for _ in range(2):  # Each worker processes up to 2 tasks
            task_data = absurd.claim_task("default", worker_id=f"worker_{threading.current_thread().name}")
            if task_data:
                absurd.process_task("default", task_data)
            time.sleep(0.05)

    threads = []
    for i in range(3):
        t = threading.Thread(target=worker, name=f"thread_{i}")
        t.start()
        threads.append(t)

    for t in threads:
        t.join()

    # All tasks should be processed
    assert len(results) == 5
    assert set(results) == {0, 1, 2, 3, 4}

    # Cleanup
    import os
    if os.path.exists("test_concurrent.db"):
        os.remove("test_concurrent.db")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
