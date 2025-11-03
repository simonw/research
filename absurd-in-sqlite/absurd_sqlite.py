"""
Absurd-in-SQLite: A simplified durable execution workflow system for SQLite
Inspired by Absurd by Armin Ronacher (https://github.com/earendil-works/absurd)
"""

import sqlite3
import uuid
import json
import time
from datetime import datetime, timedelta
from typing import Optional, Dict, Any, Callable, List
from contextlib import contextmanager
from dataclasses import dataclass
import threading


@dataclass
class TaskContext:
    """Context object passed to task functions"""
    task_id: str
    run_id: str
    attempt: int
    params: Dict[str, Any]
    _absurd: 'AbsurdSQLite'
    _queue_name: str
    _checkpoints: Dict[str, Any]

    def step(self, name: str, fn: Callable[[], Any]) -> Any:
        """
        Execute a step with checkpointing. If the step has been executed before,
        return the cached result. Otherwise, execute the function and checkpoint.
        """
        # Check if we already have this checkpoint
        if name in self._checkpoints:
            return self._checkpoints[name]

        # Execute the step
        result = fn()

        # Checkpoint the result
        self._absurd._set_checkpoint(
            self._queue_name,
            self.task_id,
            name,
            result,
            self.run_id
        )

        # Cache it locally
        self._checkpoints[name] = result

        return result

    def sleep(self, seconds: float) -> None:
        """
        Sleep for a specified duration. The task will be suspended and
        resumed after the duration expires.
        """
        # Generate a unique checkpoint name for this sleep based on the call location
        # We use the checkpoint mechanism to ensure we don't sleep again on resume
        import traceback
        frame = traceback.extract_stack()[-2]
        sleep_checkpoint_name = f"__sleep:{frame.filename}:{frame.lineno}"

        # Check if we've already slept at this location
        if sleep_checkpoint_name in self._checkpoints:
            # We've already slept here, just return
            return

        # Mark that we're sleeping at this location
        self._absurd._set_checkpoint(
            self._queue_name,
            self.task_id,
            sleep_checkpoint_name,
            True,
            self.run_id
        )

        # Schedule the wake-up
        wake_at = datetime.now() + timedelta(seconds=seconds)
        self._absurd._schedule_run(self._queue_name, self.run_id, wake_at)
        raise _SleepException()

    def await_event(self, event_name: str, timeout: Optional[float] = None) -> Any:
        """
        Wait for an event to be emitted. The task suspends until the event arrives.
        If timeout is specified, the task will fail if the event doesn't arrive in time.
        """
        step_name = f"await_event:{event_name}"

        # Check if event already in checkpoint
        if step_name in self._checkpoints:
            return self._checkpoints[step_name]

        # Check if event already emitted
        event_payload = self._absurd._check_event(self._queue_name, event_name)
        if event_payload is not None:
            # Cache the event payload as a checkpoint
            self._absurd._set_checkpoint(
                self._queue_name,
                self.task_id,
                step_name,
                event_payload,
                self.run_id
            )
            self._checkpoints[step_name] = event_payload
            return event_payload

        # Register wait and suspend
        timeout_at = None
        if timeout is not None:
            timeout_at = datetime.now() + timedelta(seconds=timeout)

        self._absurd._register_wait(
            self._queue_name,
            self.task_id,
            self.run_id,
            step_name,
            event_name,
            timeout_at
        )

        raise _SleepException()


class _SleepException(Exception):
    """Internal exception to signal task suspension"""
    pass


class AbsurdSQLite:
    """
    A simple durable execution workflow system built on SQLite.

    Based on Absurd by Armin Ronacher.
    """

    def __init__(self, db_path: str = ":memory:"):
        self.db_path = db_path
        self._tasks: Dict[str, Callable] = {}
        self._shared_conn = None  # For in-memory databases

        # For in-memory databases, we need to keep a persistent connection
        if db_path == ":memory:":
            self._shared_conn = self._create_conn()

        self._init_db()

    def _create_conn(self) -> sqlite3.Connection:
        """Create a new database connection"""
        conn = sqlite3.connect(self.db_path, isolation_level=None, check_same_thread=False)
        conn.row_factory = sqlite3.Row
        if self.db_path != ":memory:":
            conn.execute("PRAGMA journal_mode=WAL")
        return conn

    def _get_conn(self) -> sqlite3.Connection:
        """Get a database connection"""
        if self._shared_conn is not None:
            return self._shared_conn
        return self._create_conn()

    @contextmanager
    def _transaction(self):
        """Context manager for transactions"""
        conn = self._get_conn()
        should_close = (self._shared_conn is None)
        try:
            conn.execute("BEGIN IMMEDIATE")
            yield conn
            conn.commit()
        except Exception:
            conn.rollback()
            raise
        finally:
            if should_close:
                conn.close()

    def _init_db(self):
        """Initialize the database schema"""
        conn = self._get_conn()
        should_close = (self._shared_conn is None)
        try:
            # Queues table
            conn.execute("""
                CREATE TABLE IF NOT EXISTS queues (
                    queue_name TEXT PRIMARY KEY,
                    created_at TEXT NOT NULL
                )
            """)
            conn.commit()
        finally:
            if should_close:
                conn.close()

    def create_queue(self, queue_name: str):
        """Create a new queue"""
        with self._transaction() as conn:
            # Insert queue
            conn.execute("""
                INSERT OR IGNORE INTO queues (queue_name, created_at)
                VALUES (?, ?)
            """, (queue_name, datetime.now().isoformat()))

            # Create tasks table
            conn.execute(f"""
                CREATE TABLE IF NOT EXISTS t_{queue_name} (
                    task_id TEXT PRIMARY KEY,
                    task_name TEXT NOT NULL,
                    params TEXT NOT NULL,
                    state TEXT NOT NULL CHECK (state IN ('pending', 'running', 'sleeping', 'completed', 'failed', 'cancelled')),
                    attempts INTEGER NOT NULL DEFAULT 0,
                    enqueue_at TEXT NOT NULL,
                    first_started_at TEXT,
                    last_attempt_run TEXT,
                    completed_payload TEXT,
                    max_attempts INTEGER,
                    retry_strategy TEXT
                )
            """)

            # Create runs table
            conn.execute(f"""
                CREATE TABLE IF NOT EXISTS r_{queue_name} (
                    run_id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    attempt INTEGER NOT NULL,
                    state TEXT NOT NULL CHECK (state IN ('pending', 'running', 'sleeping', 'completed', 'failed', 'cancelled')),
                    claimed_by TEXT,
                    claim_expires_at TEXT,
                    available_at TEXT NOT NULL,
                    wake_event TEXT,
                    event_payload TEXT,
                    started_at TEXT,
                    completed_at TEXT,
                    failed_at TEXT,
                    result TEXT,
                    failure_reason TEXT,
                    created_at TEXT NOT NULL
                )
            """)

            # Create checkpoints table
            conn.execute(f"""
                CREATE TABLE IF NOT EXISTS c_{queue_name} (
                    task_id TEXT NOT NULL,
                    checkpoint_name TEXT NOT NULL,
                    state TEXT,
                    owner_run_id TEXT,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY (task_id, checkpoint_name)
                )
            """)

            # Create events table
            conn.execute(f"""
                CREATE TABLE IF NOT EXISTS e_{queue_name} (
                    event_name TEXT PRIMARY KEY,
                    payload TEXT,
                    emitted_at TEXT NOT NULL
                )
            """)

            # Create wait registrations table
            conn.execute(f"""
                CREATE TABLE IF NOT EXISTS w_{queue_name} (
                    task_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    step_name TEXT NOT NULL,
                    event_name TEXT NOT NULL,
                    timeout_at TEXT,
                    created_at TEXT NOT NULL,
                    PRIMARY KEY (run_id, step_name)
                )
            """)

            # Create indexes
            conn.execute(f"""
                CREATE INDEX IF NOT EXISTS r_{queue_name}_state_available
                ON r_{queue_name} (state, available_at)
            """)

            conn.execute(f"""
                CREATE INDEX IF NOT EXISTS r_{queue_name}_task_id
                ON r_{queue_name} (task_id)
            """)

            conn.execute(f"""
                CREATE INDEX IF NOT EXISTS w_{queue_name}_event_name
                ON w_{queue_name} (event_name)
            """)

    def register_task(self, name: str, fn: Callable[[Dict[str, Any], TaskContext], Any]):
        """Register a task handler"""
        self._tasks[name] = fn

    def spawn(self, queue_name: str, task_name: str, params: Dict[str, Any],
              max_attempts: Optional[int] = None,
              retry_strategy: Optional[Dict[str, Any]] = None) -> str:
        """
        Spawn a new task to be executed.

        Returns the task_id.
        """
        task_id = str(uuid.uuid4())
        run_id = str(uuid.uuid4())
        now = datetime.now().isoformat()

        with self._transaction() as conn:
            # Insert task
            conn.execute(f"""
                INSERT INTO t_{queue_name}
                (task_id, task_name, params, state, attempts, enqueue_at, last_attempt_run, max_attempts, retry_strategy)
                VALUES (?, ?, ?, 'pending', 1, ?, ?, ?, ?)
            """, (task_id, task_name, json.dumps(params), now, run_id, max_attempts,
                  json.dumps(retry_strategy) if retry_strategy else None))

            # Insert first run
            conn.execute(f"""
                INSERT INTO r_{queue_name}
                (run_id, task_id, attempt, state, available_at, created_at)
                VALUES (?, ?, 1, 'pending', ?, ?)
            """, (run_id, task_id, now, now))

        return task_id

    def claim_task(self, queue_name: str, worker_id: str = "worker",
                   claim_timeout: int = 30) -> Optional[Dict[str, Any]]:
        """
        Claim a task from the queue. Returns task details or None if no tasks available.
        """
        now = datetime.now().isoformat()
        claim_until = (datetime.now() + timedelta(seconds=claim_timeout)).isoformat()

        with self._transaction() as conn:
            # First, handle expired claims (simplified version)
            conn.execute(f"""
                UPDATE r_{queue_name}
                SET state = 'failed',
                    failed_at = ?
                WHERE state = 'running'
                AND claim_expires_at IS NOT NULL
                AND claim_expires_at <= ?
            """, (now, now))

            # Find and claim an available task
            cursor = conn.execute(f"""
                SELECT r.run_id, r.task_id, r.attempt, r.wake_event, r.event_payload,
                       t.task_name, t.params, t.retry_strategy, t.max_attempts
                FROM r_{queue_name} r
                JOIN t_{queue_name} t ON t.task_id = r.task_id
                WHERE r.state IN ('pending', 'sleeping')
                AND t.state IN ('pending', 'sleeping', 'running')
                AND r.available_at <= ?
                ORDER BY r.available_at, r.run_id
                LIMIT 1
            """, (now,))

            row = cursor.fetchone()
            if not row:
                return None

            run_id = row['run_id']
            task_id = row['task_id']
            attempt = row['attempt']

            # Claim the run
            conn.execute(f"""
                UPDATE r_{queue_name}
                SET state = 'running',
                    claimed_by = ?,
                    claim_expires_at = ?,
                    started_at = ?,
                    available_at = ?
                WHERE run_id = ?
            """, (worker_id, claim_until, now, now, run_id))

            # Update task
            conn.execute(f"""
                UPDATE t_{queue_name}
                SET state = 'running',
                    attempts = MAX(attempts, ?),
                    first_started_at = COALESCE(first_started_at, ?),
                    last_attempt_run = ?
                WHERE task_id = ?
            """, (attempt, now, run_id, task_id))

            # Clean up expired waits
            conn.execute(f"""
                DELETE FROM w_{queue_name}
                WHERE run_id = ?
                AND timeout_at IS NOT NULL
                AND timeout_at <= ?
            """, (run_id, now))

            return {
                'run_id': run_id,
                'task_id': task_id,
                'attempt': attempt,
                'task_name': row['task_name'],
                'params': json.loads(row['params']),
                'retry_strategy': json.loads(row['retry_strategy']) if row['retry_strategy'] else None,
                'max_attempts': row['max_attempts'],
                'wake_event': row['wake_event'],
                'event_payload': json.loads(row['event_payload']) if row['event_payload'] else None
            }

    def _load_checkpoints(self, queue_name: str, task_id: str) -> Dict[str, Any]:
        """Load all checkpoints for a task"""
        conn = self._get_conn()
        should_close = (self._shared_conn is None)
        try:
            cursor = conn.execute(f"""
                SELECT checkpoint_name, state
                FROM c_{queue_name}
                WHERE task_id = ?
            """, (task_id,))

            checkpoints = {}
            for row in cursor:
                checkpoints[row['checkpoint_name']] = json.loads(row['state'])

            return checkpoints
        finally:
            if should_close:
                conn.close()

    def _set_checkpoint(self, queue_name: str, task_id: str, step_name: str,
                       state: Any, owner_run_id: str):
        """Set a checkpoint for a step"""
        now = datetime.now().isoformat()

        with self._transaction() as conn:
            conn.execute(f"""
                INSERT OR REPLACE INTO c_{queue_name}
                (task_id, checkpoint_name, state, owner_run_id, updated_at)
                VALUES (?, ?, ?, ?, ?)
            """, (task_id, step_name, json.dumps(state), owner_run_id, now))

    def _complete_run(self, queue_name: str, run_id: str, result: Any = None):
        """Mark a run and its task as completed"""
        now = datetime.now().isoformat()

        with self._transaction() as conn:
            # Get task_id
            cursor = conn.execute(f"""
                SELECT task_id FROM r_{queue_name} WHERE run_id = ?
            """, (run_id,))
            row = cursor.fetchone()
            if not row:
                raise Exception(f"Run {run_id} not found")
            task_id = row['task_id']

            # Update run
            conn.execute(f"""
                UPDATE r_{queue_name}
                SET state = 'completed',
                    completed_at = ?,
                    result = ?
                WHERE run_id = ?
            """, (now, json.dumps(result), run_id))

            # Update task
            conn.execute(f"""
                UPDATE t_{queue_name}
                SET state = 'completed',
                    completed_payload = ?,
                    last_attempt_run = ?
                WHERE task_id = ?
            """, (json.dumps(result), run_id, task_id))

            # Clean up waits
            conn.execute(f"""
                DELETE FROM w_{queue_name} WHERE run_id = ?
            """, (run_id,))

    def _schedule_run(self, queue_name: str, run_id: str, wake_at: datetime):
        """Schedule a run to wake at a specific time"""
        wake_at_str = wake_at.isoformat()

        with self._transaction() as conn:
            # Get task_id
            cursor = conn.execute(f"""
                SELECT task_id FROM r_{queue_name} WHERE run_id = ?
            """, (run_id,))
            row = cursor.fetchone()
            if not row:
                raise Exception(f"Run {run_id} not found")
            task_id = row['task_id']

            # Update run
            conn.execute(f"""
                UPDATE r_{queue_name}
                SET state = 'sleeping',
                    claimed_by = NULL,
                    claim_expires_at = NULL,
                    available_at = ?,
                    wake_event = NULL
                WHERE run_id = ?
            """, (wake_at_str, run_id))

            # Update task
            conn.execute(f"""
                UPDATE t_{queue_name}
                SET state = 'sleeping'
                WHERE task_id = ?
            """, (task_id,))

    def _fail_run(self, queue_name: str, run_id: str, reason: str):
        """
        Mark a run as failed and schedule a retry if appropriate.
        """
        now = datetime.now().isoformat()

        with self._transaction() as conn:
            # Get run and task info
            cursor = conn.execute(f"""
                SELECT r.task_id, r.attempt, t.retry_strategy, t.max_attempts
                FROM r_{queue_name} r
                JOIN t_{queue_name} t ON t.task_id = r.task_id
                WHERE r.run_id = ?
            """, (run_id,))
            row = cursor.fetchone()
            if not row:
                raise Exception(f"Run {run_id} not found")

            task_id = row['task_id']
            attempt = row['attempt']
            max_attempts = row['max_attempts']
            retry_strategy = json.loads(row['retry_strategy']) if row['retry_strategy'] else None

            # Mark run as failed
            conn.execute(f"""
                UPDATE r_{queue_name}
                SET state = 'failed',
                    failed_at = ?,
                    failure_reason = ?
                WHERE run_id = ?
            """, (now, json.dumps({'error': reason}), run_id))

            # Check if we should retry
            next_attempt = attempt + 1
            should_retry = max_attempts is None or next_attempt <= max_attempts

            if should_retry:
                # Calculate delay
                delay = self._calculate_retry_delay(retry_strategy, attempt)
                next_available = (datetime.now() + timedelta(seconds=delay)).isoformat()

                # Create new run
                new_run_id = str(uuid.uuid4())
                conn.execute(f"""
                    INSERT INTO r_{queue_name}
                    (run_id, task_id, attempt, state, available_at, created_at)
                    VALUES (?, ?, ?, ?, ?, ?)
                """, (new_run_id, task_id, next_attempt,
                      'sleeping' if delay > 0 else 'pending', next_available, now))

                # Update task
                conn.execute(f"""
                    UPDATE t_{queue_name}
                    SET state = ?,
                        attempts = MAX(attempts, ?),
                        last_attempt_run = ?
                    WHERE task_id = ?
                """, ('sleeping' if delay > 0 else 'pending', next_attempt, new_run_id, task_id))
            else:
                # No more retries, mark task as failed
                conn.execute(f"""
                    UPDATE t_{queue_name}
                    SET state = 'failed'
                    WHERE task_id = ?
                """, (task_id,))

            # Clean up waits
            conn.execute(f"""
                DELETE FROM w_{queue_name} WHERE run_id = ?
            """, (run_id,))

    def _calculate_retry_delay(self, retry_strategy: Optional[Dict[str, Any]],
                               attempt: int) -> float:
        """Calculate retry delay based on strategy"""
        if not retry_strategy:
            return 0

        kind = retry_strategy.get('kind', 'none')

        if kind == 'fixed':
            return retry_strategy.get('base_seconds', 60)
        elif kind == 'exponential':
            base = retry_strategy.get('base_seconds', 30)
            factor = retry_strategy.get('factor', 2)
            max_seconds = retry_strategy.get('max_seconds')
            delay = base * (factor ** (attempt - 1))
            if max_seconds:
                delay = min(delay, max_seconds)
            return delay
        else:
            return 0

    def _check_event(self, queue_name: str, event_name: str) -> Optional[Any]:
        """Check if an event has been emitted"""
        conn = self._get_conn()
        should_close = (self._shared_conn is None)
        try:
            cursor = conn.execute(f"""
                SELECT payload FROM e_{queue_name} WHERE event_name = ?
            """, (event_name,))
            row = cursor.fetchone()
            if row:
                return json.loads(row['payload'])
            return None
        finally:
            if should_close:
                conn.close()

    def _register_wait(self, queue_name: str, task_id: str, run_id: str,
                      step_name: str, event_name: str, timeout_at: Optional[datetime]):
        """Register a task as waiting for an event"""
        now = datetime.now().isoformat()
        timeout_str = timeout_at.isoformat() if timeout_at else None

        with self._transaction() as conn:
            # Register wait
            conn.execute(f"""
                INSERT OR REPLACE INTO w_{queue_name}
                (task_id, run_id, step_name, event_name, timeout_at, created_at)
                VALUES (?, ?, ?, ?, ?, ?)
            """, (task_id, run_id, step_name, event_name, timeout_str, now))

            # Update run to sleeping state
            conn.execute(f"""
                UPDATE r_{queue_name}
                SET state = 'sleeping',
                    claimed_by = NULL,
                    claim_expires_at = NULL,
                    available_at = ?,
                    wake_event = ?,
                    event_payload = NULL
                WHERE run_id = ?
            """, (timeout_str or '9999-12-31', event_name, run_id))

            # Update task
            conn.execute(f"""
                UPDATE t_{queue_name}
                SET state = 'sleeping'
                WHERE task_id = ?
            """, (task_id,))

    def emit_event(self, queue_name: str, event_name: str, payload: Any = None):
        """
        Emit an event. Any tasks waiting for this event will be woken up.
        """
        now = datetime.now().isoformat()

        with self._transaction() as conn:
            # Store the event
            conn.execute(f"""
                INSERT OR REPLACE INTO e_{queue_name}
                (event_name, payload, emitted_at)
                VALUES (?, ?, ?)
            """, (event_name, json.dumps(payload), now))

            # Find all tasks waiting for this event
            cursor = conn.execute(f"""
                SELECT w.task_id, w.run_id, w.step_name
                FROM w_{queue_name} w
                JOIN r_{queue_name} r ON r.run_id = w.run_id
                WHERE w.event_name = ?
                AND r.state = 'sleeping'
                AND (w.timeout_at IS NULL OR w.timeout_at > ?)
            """, (event_name, now))

            waiting_tasks = cursor.fetchall()

            for row in waiting_tasks:
                task_id = row['task_id']
                run_id = row['run_id']
                step_name = row['step_name']

                # Create checkpoint for the event
                conn.execute(f"""
                    INSERT OR REPLACE INTO c_{queue_name}
                    (task_id, checkpoint_name, state, owner_run_id, updated_at)
                    VALUES (?, ?, ?, ?, ?)
                """, (task_id, step_name, json.dumps(payload), run_id, now))

                # Wake up the run
                conn.execute(f"""
                    UPDATE r_{queue_name}
                    SET state = 'pending',
                        available_at = ?,
                        wake_event = NULL,
                        event_payload = ?,
                        claimed_by = NULL,
                        claim_expires_at = NULL
                    WHERE run_id = ?
                """, (now, json.dumps(payload), run_id))

                # Update task
                conn.execute(f"""
                    UPDATE t_{queue_name}
                    SET state = 'pending'
                    WHERE task_id = ?
                """, (task_id,))

                # Remove wait registration
                conn.execute(f"""
                    DELETE FROM w_{queue_name}
                    WHERE run_id = ? AND step_name = ?
                """, (run_id, step_name))

    def process_task(self, queue_name: str, task_data: Dict[str, Any]) -> bool:
        """
        Process a claimed task. Returns True if completed, False if suspended.
        """
        run_id = task_data['run_id']
        task_id = task_data['task_id']
        task_name = task_data['task_name']
        params = task_data['params']
        attempt = task_data['attempt']

        # Get task handler
        if task_name not in self._tasks:
            self._fail_run(queue_name, run_id, f"Unknown task: {task_name}")
            return False

        task_fn = self._tasks[task_name]

        # Load checkpoints
        checkpoints = self._load_checkpoints(queue_name, task_id)

        # Create context
        ctx = TaskContext(
            task_id=task_id,
            run_id=run_id,
            attempt=attempt,
            params=params,
            _absurd=self,
            _queue_name=queue_name,
            _checkpoints=checkpoints
        )

        try:
            # Execute task
            result = task_fn(params, ctx)

            # Task completed successfully
            self._complete_run(queue_name, run_id, result)
            return True

        except _SleepException:
            # Task suspended (sleep or event wait)
            return False

        except Exception as e:
            # Task failed
            self._fail_run(queue_name, run_id, str(e))
            return False

    def run_worker(self, queue_name: str, worker_id: str = "worker",
                   poll_interval: float = 1.0):
        """
        Run a worker that continuously processes tasks from a queue.
        This is a blocking call.
        """
        while True:
            task = self.claim_task(queue_name, worker_id)
            if task:
                self.process_task(queue_name, task)
            else:
                time.sleep(poll_interval)

    def get_task_status(self, queue_name: str, task_id: str) -> Optional[Dict[str, Any]]:
        """Get the current status of a task"""
        conn = self._get_conn()
        should_close = (self._shared_conn is None)
        try:
            cursor = conn.execute(f"""
                SELECT task_name, params, state, attempts, completed_payload
                FROM t_{queue_name}
                WHERE task_id = ?
            """, (task_id,))
            row = cursor.fetchone()
            if not row:
                return None

            return {
                'task_id': task_id,
                'task_name': row['task_name'],
                'params': json.loads(row['params']),
                'state': row['state'],
                'attempts': row['attempts'],
                'completed_payload': json.loads(row['completed_payload']) if row['completed_payload'] else None
            }
        finally:
            if should_close:
                conn.close()
