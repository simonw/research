Designed as a Python C extension, the SQLite Time Limit Extension introduces a function, execute_with_timeout, enabling SQL queries against a SQLite database to be terminated if they exceed a specified millisecond threshold. This is achieved using SQLite's progress handler, ensuring that long-running queries do not block application responsiveness. Usage is simple via standard import, and rigorous tests are provided with pytest to validate both normal operation and timeouts. The project is organized for easy development and rapid testing, making it practical for integration into larger Python projects.

Key features:
- Provides precise control over query execution times.
- Raises TimeoutError automatically on time exceedance.
- Full source and tests available at [GitHub repository](https://github.com/example/sqlite-time-limit-extension) (replace with your real link).
- Built as a minimal package for straightforward integration: [PyPI Listing](https://pypi.org/project/sqlite-time-limit/) (if available).
