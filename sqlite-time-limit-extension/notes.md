Notes
====

- Initialized project folder and notes.md.
- Added pytest coverage for execute_with_timeout behavior (rows, timeout, invalid timeout).
- Added stub C extension so pytest can import before implementation.
- Ran pytest (expected failures due to NotImplementedError stub).
- Implemented execute_with_timeout in C using sqlite3_progress_handler and monotonic timing.
- Rebuilt extension and pytest now passes.
- Wrote README report for the extension.
