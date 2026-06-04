# Absurd-in-SQLite

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A Python implementation of durable execution workflows using SQLite, inspired by [Absurd](https://github.com/earendil-works/absurd) by **Armin Ronacher**.

## Credits

This project is a proof-of-concept implementation based on the concepts from:
- **Absurd** by Armin Ronacher: https://github.com/earendil-works/absurd
- **Blog post**: ["Absurd Workflows"](https://lucumr.pocoo.org/2025/11/3/absurd-workflows/) by Armin Ronacher

The design patterns, concepts, and workflow semantics are all derived from Armin's excellent work on making durable execution simple and accessible.

## What is Durable Execution?

Durable execution (or durable workflows) is a way to run long-lived, reliable functions that can survive crashes, restarts, and network failures without losing state or duplicating work. Instead of running your logic purely in memory, a durable execution system:

1. **Decomposes tasks into steps**: Each step is a checkpoint
2. **Records every step**: State is persisted to a database
3. **Enables resumption**: When a process crashes or suspends, it can resume from the last completed step
4. **Provides "exactly-once" semantics**: Steps that have completed won't re-execute

This makes it possible to build dependable systems for things like:
- LLM-based agents with multiple reasoning steps
- Payment processing workflows
- Order fulfillment systems
- Email scheduling
- Any process that spans minutes, days, or even years

## Key Learnings from Implementing Absurd Patterns

### 1. **The Replay Model**

The most important concept I learned: **When a task resumes, it replays the entire function from the beginning.** However, steps that have already been checkpointed return their cached results immediately without re-executing.

```python
def my_task(params, ctx):
    # This code WILL run multiple times if the task suspends/resumes
    print("Starting task...")

    # But this step only executes once - on resume, it returns cached result
    result = ctx.step("expensive_operation", lambda: call_expensive_api())

    # This will ALSO run multiple times
    print(f"Got result: {result}")

    return result
```

**Implication**: Any code outside of `ctx.step()` calls should be idempotent or have no side effects, because it will re-execute on every resume.

### 2. **Steps Are the Unit of Progress**

Steps serve multiple purposes:
- **Checkpointing**: State is saved after each successful step
- **Atomicity**: Either a step completes fully or not at all
- **Idempotency**: Steps with the same name return cached results
- **Progress tracking**: You always know exactly where you are in a workflow

### 3. **Pull-Based is Simpler Than Push**

Absurd (and this implementation) uses a **pull-based model**: workers pull tasks from a queue when they have capacity. This is much simpler than push-based systems because:
- No coordinator needed to route work
- Natural backpressure: workers only claim what they can handle
- Easy to scale: just add more workers

### 4. **SQLite is Sufficient (with caveats)**

For moderate workloads, SQLite can absolutely handle durable workflows:
- **WAL mode** provides good concurrency
- **Transactions** ensure consistency
- **File-based** means no separate database server

However, Postgres (as used in Absurd) has advantages:
- `SELECT ... FOR UPDATE SKIP LOCKED` for lock-free task claiming
- Better support for high-concurrency scenarios
- Advisory locks and more sophisticated queueing primitives

### 5. **Events Enable Coordination**

The event system is crucial for real-world workflows:
- **Cached events**: Events are stored, so tasks can receive them even if emitted before waiting
- **Race-free**: No race conditions between emitting and waiting
- **Decoupled**: External systems can trigger workflow progression without knowing implementation details

### 6. **Retry Strategies Need Thought**

Implementing retry strategies taught me:
- **Exponential backoff** is essential for transient failures
- **Max attempts** prevents infinite loops on permanent failures
- **Claim timeouts** handle crashed workers
- Retries happen at the **task level**, not the step level

## Architecture

### Core Components

1. **Tasks**: Units of work that can be decomposed into steps
2. **Steps**: Checkpointed operations within a task
3. **Runs**: Attempts to execute a task (tasks may have multiple runs due to retries)
4. **Checkpoints**: Saved state for each completed step
5. **Events**: Messages that tasks can emit and await
6. **Queues**: Named channels for organizing different types of work

### Database Schema

Each queue gets 5 tables:
- `t_{queue}`: Tasks (what needs to be done)
- `r_{queue}`: Runs (execution attempts)
- `c_{queue}`: Checkpoints (saved step results)
- `e_{queue}`: Events (emitted messages)
- `w_{queue}`: Wait registrations (tasks waiting for events)

## Usage

### Basic Example

```python
from absurd_sqlite import AbsurdSQLite, TaskContext

# Create instance and queue
absurd = AbsurdSQLite("workflows.db")
absurd.create_queue("default")

# Define a task
def process_order(params, ctx: TaskContext):
    # Step 1: Charge payment
    payment = ctx.step("charge", lambda: {
        "transaction_id": charge_credit_card(params['amount']),
        "amount": params['amount']
    })

    # Step 2: Update inventory
    inventory = ctx.step("inventory", lambda: {
        "reserved": reserve_items(params['items'])
    })

    # Step 3: Wait for fulfillment
    shipment = ctx.await_event(f"shipment:{params['order_id']}")

    # Step 4: Send notification
    ctx.step("notify", lambda: send_email(
        params['email'],
        f"Your order has shipped! Tracking: {shipment['tracking']}"
    ))

    return {"success": True, "tracking": shipment['tracking']}

# Register the task
absurd.register_task("process_order", process_order)

# Spawn a task
task_id = absurd.spawn("default", "process_order", {
    "order_id": "12345",
    "amount": 99.99,
    "items": ["SKU-001", "SKU-002"],
    "email": "customer@example.com"
})

# Process tasks (in a worker)
while True:
    task = absurd.claim_task("default", worker_id="worker-1")
    if task:
        absurd.process_task("default", task)
    else:
        time.sleep(1)
```

### External Event Emission

```python
# From another process (e.g., warehouse system):
absurd.emit_event("default", "shipment:12345", {
    "tracking": "TRACK-789",
    "carrier": "UPS"
})
```

### Agent Loop Pattern

```python
def agent_task(params, ctx: TaskContext):
    messages = [{"role": "user", "content": params['prompt']}]

    for i in range(20):  # Max 20 iterations
        step_name = "iteration" if i == 0 else f"iteration#{i+1}"

        response = ctx.step(step_name, lambda: call_llm(messages))
        messages.append({"role": "assistant", "content": response})

        if response.get("done"):
            break

    return {"messages": messages}
```

## Key Differences from Postgres Absurd

1. **Connection Handling**: SQLite's `:memory:` databases require a persistent connection, while file-based DBs can create connections per operation

2. **Task Claiming**: SQLite doesn't have `SELECT ... FOR UPDATE SKIP LOCKED`, so claiming uses a simpler lock-and-update approach

3. **Concurrency**: SQLite's WAL mode provides good concurrency, but Postgres handles higher concurrent worker counts better

4. **UUID Generation**: Uses Python's `uuid.uuid4()` instead of Postgres's `uuid_generate_v7()`

5. **Language**: Python functions instead of PL/pgSQL stored procedures

## Testing

The test suite demonstrates all key patterns:

```bash
pytest test_absurd_sqlite.py -v
```

Tests cover:
- Basic task execution with steps
- Checkpoint recovery after failures
- Retry strategies (fixed and exponential backoff)
- Sleep/suspension
- Event emission and waiting
- Multi-task event coordination
- Real-world order fulfillment workflow
- Agent loop patterns
- Concurrent workers

All 13 tests pass! ✅

## Important Design Patterns

### 1. Side Effects in Steps

❌ **Wrong**:
```python
def my_task(params, ctx):
    result = ctx.step("work", lambda: do_work())
    send_email(result)  # Will send email EVERY time task resumes!
    return result
```

✅ **Right**:
```python
def my_task(params, ctx):
    result = ctx.step("work", lambda: do_work())
    ctx.step("notify", lambda: send_email(result))  # Only sends once
    return result
```

### 2. Idempotency Keys

For external APIs that support idempotency:

```python
def my_task(params, ctx):
    payment = ctx.step("charge", lambda: stripe.charges.create(
        amount=params['amount'],
        idempotency_key=f"{ctx.task_id}:charge"  # Unique per task+step
    ))
    return payment
```

### 3. Event Naming

Use structured event names for clarity:

```python
# Good: namespace:entity:id
await ctx.await_event(f"shipment.packed:{order_id}")
await ctx.await_event(f"user.verified:{user_id}")

# Less clear
await ctx.await_event("shipment_event")
```

## Limitations

1. **No distributed transactions**: Each task runs in its own transaction context
2. **No nested workflows**: Tasks can't spawn sub-tasks and wait for them (yet)
3. **No time-travel debugging**: Unlike some systems, can't replay from arbitrary points
4. **Limited observability**: No built-in UI (Absurd has "habitat" for this)
5. **SQLite concurrency limits**: For very high throughput, Postgres is better

## Future Enhancements

- [ ] Add task cancellation support
- [ ] Implement scheduled/delayed task spawning
- [ ] Add metrics and observability hooks
- [ ] Support for task priorities
- [ ] Dead letter queue for failed tasks
- [ ] Nested workflow support
- [ ] Web UI for monitoring (like Absurd's "habitat")

## Why This Matters

Building this taught me that **durable execution doesn't have to be complicated**. You don't need:
- Separate workflow coordinators
- Custom DSLs or code generation
- Complex distributed systems
- Heavyweight frameworks

With just:
- A database (SQLite/Postgres)
- Checkpointing at the right granularity
- A replay model
- Event coordination

You can build reliable, long-running workflows that survive crashes and can be reasoned about clearly.

Armin's Absurd proves this beautifully with Postgres, and this SQLite version shows the patterns are portable and understandable.

## License

This is a proof-of-concept implementation for educational purposes. The original Absurd is licensed under Apache 2.0.

## Learn More

- **Absurd (original)**: https://github.com/earendil-works/absurd
- **Blog post**: https://lucumr.pocoo.org/2025/11/3/absurd-workflows/
- **Armin's blog**: https://lucumr.pocoo.org/

Thank you to Armin Ronacher for creating Absurd and sharing these elegant patterns with the community! 🙏
