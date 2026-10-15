//! `timeout_ms` for the statements SQLite steps on our behalf (the baseline engine's SQL).
//!
//! The native engine checks its deadline between pieces of its own work (R6), and so does
//! the baseline engine between batches. But one `sqlite3_step` of a statement SQLite runs can
//! take unbounded time: a selective FILTER over a large join yields no row for a long while,
//! and `sparql_count_sql` steps a single `SELECT count(*)`. A [`Watchdog`] bounds those steps:
//! a helper thread waits for the deadline and calls `sqlite3_interrupt` (on the §1.6
//! allow-list; the progress-handler slot belongs to the user) if the run's statement is
//! stepping at that moment. The interrupted step fails with `SQLITE_INTERRUPT`, which the run
//! reports as `Error::Timeout`; the storage layer's scopes already survive interrupts.
//!
//! **Safety.** The thread touches the connection only while the watchdog is
//! [armed](Watchdog::arm), and only under the lock that disarming takes. A run arms it around
//! one call that steps its statement, inside a SQLite callback of the connection, so the
//! connection is open whenever it is armed. The lock is never held while a statement steps
//! (helper functions run inside those steps), so it cannot deadlock (R1). The thread is
//! joined when the watchdog is dropped: none outlives its run, nor the extension's code
//! (SQLite unloads extensions when their connection closes).
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

use sq_core::{Error, Result};
use sq_sqlite::RawDb;

/// A deadline that interrupts the connection's running step when it passes.
pub(crate) struct Watchdog {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

struct Shared {
    deadline: Instant,
    db: Db,
    state: Mutex<State>,
    wake: Condvar,
}

#[derive(Default)]
struct State {
    /// A step of the run's statement is in progress.
    armed: bool,
    /// The deadline passed while armed: the connection was interrupted.
    fired: bool,
    /// The watchdog is dropped: the thread ends.
    stop: bool,
}

/// The connection handle, used from the watchdog thread only for `sqlite3_interrupt`, which
/// SQLite allows from any thread while the connection is open (see the module docs).
struct Db(RawDb);

// SAFETY: see `Db` and the module docs: the handle is only passed to sqlite3_interrupt, while
// the connection is known to be open.
unsafe impl Send for Db {}
unsafe impl Sync for Db {}

impl Watchdog {
    /// Starts the watchdog thread for `deadline` on the connection `db`.
    pub(crate) fn start(db: RawDb, deadline: Instant) -> Result<Watchdog> {
        let shared =
            Arc::new(Shared { deadline, db: Db(db), state: Mutex::new(State::default()), wake: Condvar::new() });
        let worker = shared.clone();
        let thread = std::thread::Builder::new()
            .name("sparqlite-timeout".into())
            .stack_size(64 << 10)
            .spawn(move || worker.run())
            .map_err(|e| Error::internal(format!("cannot start the timeout_ms watchdog thread: {e}")))?;
        Ok(Watchdog { shared, thread: Some(thread) })
    }

    /// Arms the watchdog for as long as the guard lives: around one call that steps the run's
    /// statement, inside a SQLite callback of the connection.
    pub(crate) fn arm(&self) -> Armed<'_> {
        self.shared.lock().armed = true;
        if Instant::now() >= self.shared.deadline {
            // Past the deadline the thread sleeps until woken.
            self.shared.wake.notify_one();
        }
        Armed(self)
    }

    /// Whether the watchdog interrupted the connection.
    pub(crate) fn fired(&self) -> bool {
        self.shared.lock().fired
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.shared.lock().stop = true;
        self.shared.wake.notify_one();
        if let Some(thread) = self.thread.take() {
            // The thread never panics; a failed join leaves nothing to clean up.
            let _ = thread.join();
        }
    }
}

/// Keeps the watchdog armed; disarms it (under the lock) when dropped.
pub(crate) struct Armed<'a>(&'a Watchdog);

impl Drop for Armed<'_> {
    fn drop(&mut self) {
        self.0.shared.lock().armed = false;
    }
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // The state is plain flags: a poisoned lock holds a usable value.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn run(&self) {
        let mut state = self.lock();
        loop {
            if state.stop {
                return;
            }
            let now = Instant::now();
            if now < self.deadline {
                state = self.wake.wait_timeout(state, self.deadline - now).unwrap_or_else(PoisonError::into_inner).0;
                continue;
            }
            if state.armed && !state.fired {
                // Armed: the connection is open, and stays so while this lock is held.
                self.db.0.interrupt();
                state.fired = true;
            }
            state = self.wake.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn db(conn: &rusqlite::Connection) -> RawDb {
        // SAFETY: `conn` outlives every use in these tests.
        unsafe { RawDb::from_ptr(conn.handle()) }
    }

    /// A statement that runs for a long time inside one `sqlite3_step`.
    const ENDLESS: &str = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT count(*) FROM c";

    #[test]
    fn an_armed_watchdog_interrupts_the_step_at_the_deadline() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let start = Instant::now();
        let dog = Watchdog::start(db(&conn), start + Duration::from_millis(50)).unwrap();
        let r = {
            let _armed = dog.arm();
            conn.query_row(ENDLESS, [], |r| r.get::<_, i64>(0))
        };
        let interrupted = rusqlite::ErrorCode::OperationInterrupted;
        assert!(matches!(r, Err(rusqlite::Error::SqliteFailure(e, _)) if e.code == interrupted), "{r:?}");
        assert!(dog.fired());
        assert!(start.elapsed() < Duration::from_secs(5));
        drop(dog);
        // The flag is gone once no statement runs.
        assert_eq!(conn.query_row("SELECT 1", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
    }

    #[test]
    fn a_disarmed_watchdog_leaves_the_connection_alone() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let dog = Watchdog::start(db(&conn), Instant::now() + Duration::from_millis(20)).unwrap();
        drop(dog.arm());
        std::thread::sleep(Duration::from_millis(60));
        assert!(!dog.fired());
        let mut st = conn.prepare("SELECT 1 UNION ALL SELECT 2").unwrap();
        let mut rows = st.query([]).unwrap();
        rows.next().unwrap();
        // Armed after the deadline (between steps): the thread wakes and interrupts at once.
        let _armed = dog.arm();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !dog.fired() && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(dog.fired());
    }

    #[test]
    fn dropping_an_unexpired_watchdog_joins_its_thread() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let start = Instant::now();
        let dog = Watchdog::start(db(&conn), start + Duration::from_secs(3600)).unwrap();
        drop(dog);
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}
