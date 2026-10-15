//! Panic containment at the FFI boundary (§8.6 rule R4).
//!
//! rusqlite catches panics in scalar and aggregate functions, but its virtual-table
//! trampolines do not: a panic unwinding out of `xFilter` into SQLite's C frames aborts the
//! host process. Every virtual-table method body therefore runs inside [`guard`] (or
//! [`guard_eof`] for `xEof`, which cannot report errors), and every `Drop` that runs from
//! `xClose` / `xDisconnect` inside [`guard_drop`].
use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Text of a panic payload.
pub(crate) fn panic_message(payload: &(dyn Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s
    } else {
        "non-string panic payload"
    }
}

/// Runs a virtual-table method body; a panic becomes an `SQLITE_ERROR` with a message naming
/// the method.
pub(crate) fn guard<T>(method: &'static str, f: impl FnOnce() -> rusqlite::Result<T>) -> rusqlite::Result<T> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(payload) => Err(rusqlite::Error::ModuleError(format!(
            "sparql: internal error: panic in {method}: {}",
            panic_message(&*payload)
        ))),
    }
}

/// `xEof` has no error channel: a panic ends the scan (reports end of data).
pub(crate) fn guard_eof(f: impl FnOnce() -> bool) -> bool {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(true)
}

/// Runs a destructor body that SQLite reaches through `xClose` / `xDisconnect`; a panic is
/// swallowed (there is nobody to report it to).
pub(crate) fn guard_drop(f: impl FnOnce()) {
    let _ = catch_unwind(AssertUnwindSafe(f));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panics_become_errors() {
        let r: rusqlite::Result<()> = guard("xFilter", || panic!("boom"));
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("panic in xFilter: boom"), "{msg}");
        let r: rusqlite::Result<()> = guard("xNext", || panic!("{}", String::from("owned")));
        assert!(r.unwrap_err().to_string().contains("owned"));
        assert_eq!(guard("x", || Ok(3)).unwrap(), 3);
        assert!(guard_eof(|| panic!("eof")));
        assert!(!guard_eof(|| false));
        guard_drop(|| panic!("drop"));
    }
}
