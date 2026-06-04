"""
sqlite3_wasm - SQLite module using WebAssembly via wasmtime

This module provides an API that is compatible with Python's standard sqlite3 module,
but uses a SQLite library compiled to WebAssembly and executed via wasmtime.
"""

from pathlib import Path
from typing import Any, Callable, Iterator, Optional, Sequence, Union
import wasmtime
from wasmtime import Store, Module, Instance, Linker, WasiConfig

# SQLite constants
SQLITE_OK = 0
SQLITE_ERROR = 1
SQLITE_BUSY = 5
SQLITE_CONSTRAINT = 19
SQLITE_MISMATCH = 20
SQLITE_MISUSE = 21
SQLITE_ROW = 100
SQLITE_DONE = 101

# Column types
SQLITE_INTEGER = 1
SQLITE_FLOAT = 2
SQLITE_TEXT = 3
SQLITE_BLOB = 4
SQLITE_NULL = 5

# Open flags
SQLITE_OPEN_READONLY = 0x00000001
SQLITE_OPEN_READWRITE = 0x00000002
SQLITE_OPEN_CREATE = 0x00000004
SQLITE_OPEN_URI = 0x00000040

# For compatibility with sqlite3 module
PARSE_DECLTYPES = 1
PARSE_COLNAMES = 2

# Version info (will be populated from WASM module)
version = ""
version_info = (0, 0, 0)
sqlite_version = ""
sqlite_version_info = (0, 0, 0)


class Error(Exception):
    """Base class for SQLite exceptions."""
    pass


class DatabaseError(Error):
    """Exception raised for database errors."""
    pass


class IntegrityError(DatabaseError):
    """Exception raised for integrity constraint violations."""
    pass


class OperationalError(DatabaseError):
    """Exception raised for operational errors."""
    pass


class ProgrammingError(DatabaseError):
    """Exception raised for programming errors."""
    pass


class InterfaceError(Error):
    """Exception raised for interface errors."""
    pass


class DataError(DatabaseError):
    """Exception raised for data errors."""
    pass


class NotSupportedError(DatabaseError):
    """Exception raised for not supported features."""
    pass


class InternalError(DatabaseError):
    """Exception raised for internal errors."""
    pass


class Warning(Exception):
    """Exception raised for warnings."""
    pass


class _SQLiteWasm:
    """Wrapper around the SQLite WASM module."""

    _instance = None

    def __new__(cls):
        if cls._instance is None:
            cls._instance = super().__new__(cls)
            cls._instance._initialized = False
        return cls._instance

    def __init__(self):
        if self._initialized:
            return

        wasm_path = Path(__file__).parent / "sqlite3.wasm"
        if not wasm_path.exists():
            raise RuntimeError(f"SQLite WASM file not found at {wasm_path}")

        # Create the wasmtime runtime
        self.store = Store()
        self.linker = Linker(self.store.engine)

        # Configure WASI
        wasi_config = WasiConfig()
        wasi_config.inherit_stdout()
        wasi_config.inherit_stderr()
        self.store.set_wasi(wasi_config)
        self.linker.define_wasi()

        # Load and instantiate the module
        self.module = Module.from_file(self.store.engine, str(wasm_path))
        self.instance = self.linker.instantiate(self.store, self.module)

        # Get memory
        self.memory = self.instance.exports(self.store)["memory"]

        # Get exported functions
        exports = self.instance.exports(self.store)
        self.malloc = exports["malloc"]
        self.free = exports["free"]
        self.strlen = exports["strlen"]

        # SQLite functions
        self.sqlite3_libversion = exports["sqlite3_libversion"]
        self.sqlite3_libversion_number = exports["sqlite3_libversion_number"]
        self.sqlite3_open = exports["sqlite3_open"]
        self.sqlite3_open_v2 = exports["sqlite3_open_v2"]
        self.sqlite3_close = exports["sqlite3_close"]
        self.sqlite3_close_v2 = exports["sqlite3_close_v2"]
        self.sqlite3_exec = exports["sqlite3_exec"]
        self.sqlite3_prepare_v2 = exports["sqlite3_prepare_v2"]
        self.sqlite3_step = exports["sqlite3_step"]
        self.sqlite3_finalize = exports["sqlite3_finalize"]
        self.sqlite3_reset = exports["sqlite3_reset"]
        self.sqlite3_clear_bindings = exports["sqlite3_clear_bindings"]
        self.sqlite3_bind_null = exports["sqlite3_bind_null"]
        self.sqlite3_bind_int = exports["sqlite3_bind_int"]
        self.sqlite3_bind_int64 = exports["sqlite3_bind_int64"]
        self.sqlite3_bind_double = exports["sqlite3_bind_double"]
        self.sqlite3_bind_text = exports["sqlite3_bind_text"]
        self.sqlite3_bind_blob = exports["sqlite3_bind_blob"]
        self.sqlite3_bind_parameter_count = exports["sqlite3_bind_parameter_count"]
        self.sqlite3_bind_parameter_name = exports["sqlite3_bind_parameter_name"]
        self.sqlite3_bind_parameter_index = exports["sqlite3_bind_parameter_index"]
        self.sqlite3_column_count = exports["sqlite3_column_count"]
        self.sqlite3_column_name = exports["sqlite3_column_name"]
        self.sqlite3_column_type = exports["sqlite3_column_type"]
        self.sqlite3_column_int = exports["sqlite3_column_int"]
        self.sqlite3_column_int64 = exports["sqlite3_column_int64"]
        self.sqlite3_column_double = exports["sqlite3_column_double"]
        self.sqlite3_column_text = exports["sqlite3_column_text"]
        self.sqlite3_column_blob = exports["sqlite3_column_blob"]
        self.sqlite3_column_bytes = exports["sqlite3_column_bytes"]
        self.sqlite3_data_count = exports["sqlite3_data_count"]
        self.sqlite3_errmsg = exports["sqlite3_errmsg"]
        self.sqlite3_errcode = exports["sqlite3_errcode"]
        self.sqlite3_errstr = exports["sqlite3_errstr"]
        self.sqlite3_last_insert_rowid = exports["sqlite3_last_insert_rowid"]
        self.sqlite3_changes = exports["sqlite3_changes"]
        self.sqlite3_total_changes = exports["sqlite3_total_changes"]
        self.sqlite3_interrupt = exports["sqlite3_interrupt"]
        self.sqlite3_complete = exports["sqlite3_complete"]
        self.sqlite3_busy_timeout = exports["sqlite3_busy_timeout"]
        self.sqlite3_get_autocommit = exports["sqlite3_get_autocommit"]
        self.sqlite3_column_decltype = exports["sqlite3_column_decltype"]

        self._initialized = True

        # Get version info
        global version, version_info, sqlite_version, sqlite_version_info
        sqlite_version = self.read_string(self.sqlite3_libversion(self.store))
        parts = sqlite_version.split(".")
        sqlite_version_info = tuple(int(p) for p in parts[:3]) if len(parts) >= 3 else (0, 0, 0)
        version = sqlite_version  # Our module version matches SQLite version
        version_info = sqlite_version_info

    def write_string(self, s: str) -> int:
        """Write a string to WASM memory and return its pointer."""
        data = s.encode("utf-8") + b"\x00"
        ptr = self.malloc(self.store, len(data))
        if ptr == 0:
            raise MemoryError("Failed to allocate memory in WASM")

        mem_data = self.memory.data_ptr(self.store)
        mem_len = self.memory.data_len(self.store)

        for i, b in enumerate(data):
            if ptr + i < mem_len:
                mem_data[ptr + i] = b

        return ptr

    def write_bytes(self, data: bytes) -> int:
        """Write bytes to WASM memory and return its pointer."""
        ptr = self.malloc(self.store, len(data))
        if ptr == 0:
            raise MemoryError("Failed to allocate memory in WASM")

        mem_data = self.memory.data_ptr(self.store)
        mem_len = self.memory.data_len(self.store)

        for i, b in enumerate(data):
            if ptr + i < mem_len:
                mem_data[ptr + i] = b

        return ptr

    def read_string(self, ptr: int) -> str:
        """Read a null-terminated string from WASM memory."""
        if ptr == 0:
            return ""

        mem_data = self.memory.data_ptr(self.store)
        mem_len = self.memory.data_len(self.store)

        result = bytearray()
        i = 0
        while ptr + i < mem_len:
            b = mem_data[ptr + i]
            if b == 0:
                break
            result.append(b)
            i += 1

        return result.decode("utf-8", errors="replace")

    def read_bytes(self, ptr: int, length: int) -> bytes:
        """Read bytes from WASM memory."""
        if ptr == 0 or length == 0:
            return b""

        mem_data = self.memory.data_ptr(self.store)
        return bytes(mem_data[ptr:ptr + length])

    def read_int32(self, ptr: int) -> int:
        """Read a 32-bit integer from WASM memory."""
        mem_data = self.memory.data_ptr(self.store)
        return int.from_bytes(mem_data[ptr:ptr + 4], "little", signed=True)

    def write_int32(self, ptr: int, value: int) -> None:
        """Write a 32-bit integer to WASM memory."""
        mem_data = self.memory.data_ptr(self.store)
        data = value.to_bytes(4, "little", signed=True)
        for i, b in enumerate(data):
            mem_data[ptr + i] = b


# Global WASM instance
_wasm: Optional[_SQLiteWasm] = None


def _get_wasm() -> _SQLiteWasm:
    """Get the global WASM instance, initializing if needed."""
    global _wasm
    if _wasm is None:
        _wasm = _SQLiteWasm()
    return _wasm


def _raise_sqlite_error(wasm: _SQLiteWasm, db_ptr: int = 0, message: Optional[str] = None):
    """Raise an appropriate SQLite exception."""
    if message is None and db_ptr:
        msg_ptr = wasm.sqlite3_errmsg(wasm.store, db_ptr)
        message = wasm.read_string(msg_ptr)

    if message is None:
        message = "Unknown SQLite error"

    if db_ptr:
        errcode = wasm.sqlite3_errcode(wasm.store, db_ptr)
    else:
        errcode = SQLITE_ERROR

    if errcode == SQLITE_CONSTRAINT:
        raise IntegrityError(message)
    elif errcode == SQLITE_BUSY:
        raise OperationalError(message)
    elif errcode == SQLITE_MISMATCH:
        raise DataError(message)
    elif errcode == SQLITE_MISUSE:
        raise ProgrammingError(message)
    else:
        raise OperationalError(message)


class Row:
    """A Row object that allows access by column name."""

    def __init__(self, cursor: 'Cursor', values: tuple):
        self._cursor = cursor
        self._values = values
        self._keys = tuple(d[0] for d in cursor.description) if cursor.description else ()

    def __getitem__(self, key):
        if isinstance(key, int):
            return self._values[key]
        elif isinstance(key, str):
            try:
                idx = self._keys.index(key)
                return self._values[idx]
            except ValueError:
                raise IndexError(f"No column named '{key}'")
        else:
            raise TypeError(f"Invalid key type: {type(key)}")

    def __iter__(self):
        return iter(self._values)

    def __len__(self):
        return len(self._values)

    def __repr__(self):
        return f"<Row {self._values}>"

    def keys(self):
        return self._keys


class Cursor:
    """SQLite cursor object."""

    def __init__(self, connection: 'Connection'):
        self._connection = connection
        self._wasm = connection._wasm
        self._stmt_ptr = 0
        self._description: Optional[tuple] = None
        self._rowcount = -1
        self._lastrowid: Optional[int] = None
        self._arraysize = 1
        self._closed = False
        self._row_factory = None

    @property
    def connection(self) -> 'Connection':
        return self._connection

    @property
    def description(self) -> Optional[tuple]:
        return self._description

    @property
    def rowcount(self) -> int:
        return self._rowcount

    @property
    def lastrowid(self) -> Optional[int]:
        return self._lastrowid

    @property
    def arraysize(self) -> int:
        return self._arraysize

    @arraysize.setter
    def arraysize(self, value: int):
        self._arraysize = value

    def _check_closed(self):
        if self._closed:
            raise ProgrammingError("Cannot operate on a closed cursor")
        if self._connection._closed:
            raise ProgrammingError("Cannot operate on a closed database")

    def _finalize_stmt(self):
        """Finalize the current statement if any."""
        if self._stmt_ptr:
            self._wasm.sqlite3_finalize(self._wasm.store, self._stmt_ptr)
            self._stmt_ptr = 0

    def _bind_parameters(self, parameters: Optional[Sequence]) -> None:
        """Bind parameters to the prepared statement."""
        if not parameters:
            return

        if not self._stmt_ptr:
            return

        # Handle dictionary parameters (named parameters)
        if isinstance(parameters, dict):
            for name, value in parameters.items():
                param_name = name if name.startswith(":") or name.startswith("$") or name.startswith("@") else f":{name}"
                name_ptr = self._wasm.write_string(param_name)
                try:
                    idx = self._wasm.sqlite3_bind_parameter_index(self._wasm.store, self._stmt_ptr, name_ptr)
                finally:
                    self._wasm.free(self._wasm.store, name_ptr)

                if idx > 0:
                    self._bind_value(idx, value)
        else:
            # Positional parameters
            for i, value in enumerate(parameters, 1):
                self._bind_value(i, value)

    def _bind_value(self, idx: int, value: Any) -> None:
        """Bind a single value to a parameter index."""
        if value is None:
            self._wasm.sqlite3_bind_null(self._wasm.store, self._stmt_ptr, idx)
        elif isinstance(value, int):
            self._wasm.sqlite3_bind_int64(self._wasm.store, self._stmt_ptr, idx, value)
        elif isinstance(value, float):
            self._wasm.sqlite3_bind_double(self._wasm.store, self._stmt_ptr, idx, value)
        elif isinstance(value, str):
            ptr = self._wasm.write_string(value)
            try:
                # -1 means use strlen, SQLITE_TRANSIENT = -1 means copy the data
                self._wasm.sqlite3_bind_text(self._wasm.store, self._stmt_ptr, idx, ptr, -1, -1)
            except:
                self._wasm.free(self._wasm.store, ptr)
                raise
        elif isinstance(value, bytes):
            ptr = self._wasm.write_bytes(value)
            try:
                self._wasm.sqlite3_bind_blob(self._wasm.store, self._stmt_ptr, idx, ptr, len(value), -1)
            except:
                self._wasm.free(self._wasm.store, ptr)
                raise
        else:
            raise InterfaceError(f"Unsupported parameter type: {type(value)}")

    def _get_column_value(self, col: int) -> Any:
        """Get the value of a column in the current row."""
        col_type = self._wasm.sqlite3_column_type(self._wasm.store, self._stmt_ptr, col)

        if col_type == SQLITE_NULL:
            return None
        elif col_type == SQLITE_INTEGER:
            return self._wasm.sqlite3_column_int64(self._wasm.store, self._stmt_ptr, col)
        elif col_type == SQLITE_FLOAT:
            return self._wasm.sqlite3_column_double(self._wasm.store, self._stmt_ptr, col)
        elif col_type == SQLITE_TEXT:
            ptr = self._wasm.sqlite3_column_text(self._wasm.store, self._stmt_ptr, col)
            return self._wasm.read_string(ptr)
        elif col_type == SQLITE_BLOB:
            ptr = self._wasm.sqlite3_column_blob(self._wasm.store, self._stmt_ptr, col)
            length = self._wasm.sqlite3_column_bytes(self._wasm.store, self._stmt_ptr, col)
            return self._wasm.read_bytes(ptr, length)
        else:
            return None

    def _fetch_row(self) -> Optional[tuple]:
        """Fetch a single row from the result set."""
        if not self._stmt_ptr:
            return None

        result = self._wasm.sqlite3_step(self._wasm.store, self._stmt_ptr)

        if result == SQLITE_ROW:
            col_count = self._wasm.sqlite3_column_count(self._wasm.store, self._stmt_ptr)
            row = tuple(self._get_column_value(i) for i in range(col_count))
            return row
        elif result == SQLITE_DONE:
            return None
        else:
            _raise_sqlite_error(self._wasm, self._connection._db_ptr)
            return None

    def _build_description(self):
        """Build the description tuple for the current statement."""
        if not self._stmt_ptr:
            self._description = None
            return

        col_count = self._wasm.sqlite3_column_count(self._wasm.store, self._stmt_ptr)
        if col_count == 0:
            self._description = None
            return

        desc = []
        for i in range(col_count):
            name_ptr = self._wasm.sqlite3_column_name(self._wasm.store, self._stmt_ptr, i)
            name = self._wasm.read_string(name_ptr)

            # Get declared type if available
            decltype_ptr = self._wasm.sqlite3_column_decltype(self._wasm.store, self._stmt_ptr, i)
            decltype = self._wasm.read_string(decltype_ptr) if decltype_ptr else None

            # (name, type_code, display_size, internal_size, precision, scale, null_ok)
            desc.append((name, decltype, None, None, None, None, None))

        self._description = tuple(desc)

    def execute(self, sql: str, parameters: Optional[Sequence] = None) -> 'Cursor':
        """Execute a SQL statement."""
        self._check_closed()
        self._finalize_stmt()

        # Prepare the statement
        sql_ptr = self._wasm.write_string(sql)
        stmt_ptr_ptr = self._wasm.malloc(self._wasm.store, 4)
        tail_ptr = self._wasm.malloc(self._wasm.store, 4)

        try:
            result = self._wasm.sqlite3_prepare_v2(
                self._wasm.store,
                self._connection._db_ptr,
                sql_ptr,
                -1,
                stmt_ptr_ptr,
                tail_ptr
            )

            if result != SQLITE_OK:
                _raise_sqlite_error(self._wasm, self._connection._db_ptr)

            self._stmt_ptr = self._wasm.read_int32(stmt_ptr_ptr)
        finally:
            self._wasm.free(self._wasm.store, sql_ptr)
            self._wasm.free(self._wasm.store, stmt_ptr_ptr)
            self._wasm.free(self._wasm.store, tail_ptr)

        # Bind parameters
        self._bind_parameters(parameters)

        # Build description for SELECT statements
        self._build_description()

        # For non-SELECT statements, execute immediately
        if self._description is None:
            result = self._wasm.sqlite3_step(self._wasm.store, self._stmt_ptr)
            if result != SQLITE_DONE and result != SQLITE_ROW:
                _raise_sqlite_error(self._wasm, self._connection._db_ptr)

            self._rowcount = self._wasm.sqlite3_changes(self._wasm.store, self._connection._db_ptr)
            self._lastrowid = self._wasm.sqlite3_last_insert_rowid(self._wasm.store, self._connection._db_ptr)
            self._finalize_stmt()
        else:
            self._rowcount = -1

        return self

    def executemany(self, sql: str, seq_of_parameters: Sequence[Sequence]) -> 'Cursor':
        """Execute a SQL statement multiple times with different parameters."""
        self._check_closed()

        for parameters in seq_of_parameters:
            self.execute(sql, parameters)

        return self

    def executescript(self, sql_script: str) -> 'Cursor':
        """Execute multiple SQL statements at once."""
        self._check_closed()

        # Split by semicolons and execute each statement
        for statement in sql_script.split(';'):
            statement = statement.strip()
            if statement:
                self.execute(statement)

        return self

    def fetchone(self) -> Optional[tuple]:
        """Fetch the next row of a query result set."""
        self._check_closed()
        row = self._fetch_row()
        if row is not None:
            row_factory = self._row_factory or self._connection._row_factory
            if row_factory:
                return row_factory(self, row)
        return row

    def fetchmany(self, size: Optional[int] = None) -> list:
        """Fetch the next set of rows of a query result set."""
        self._check_closed()
        if size is None:
            size = self._arraysize

        results = []
        for _ in range(size):
            row = self.fetchone()
            if row is None:
                break
            results.append(row)

        return results

    def fetchall(self) -> list:
        """Fetch all remaining rows of a query result set."""
        self._check_closed()
        results = []
        while True:
            row = self.fetchone()
            if row is None:
                break
            results.append(row)
        return results

    def close(self) -> None:
        """Close the cursor."""
        if not self._closed:
            self._finalize_stmt()
            self._closed = True

    def setinputsizes(self, sizes):
        """Set input sizes (no-op for compatibility)."""
        pass

    def setoutputsize(self, size, column=None):
        """Set output size (no-op for compatibility)."""
        pass

    def __iter__(self) -> Iterator:
        return self

    def __next__(self):
        row = self.fetchone()
        if row is None:
            raise StopIteration
        return row

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
        return False


class Connection:
    """SQLite database connection object."""

    def __init__(self, database: str, timeout: float = 5.0,
                 isolation_level: Optional[str] = "",
                 detect_types: int = 0,
                 check_same_thread: bool = True):
        self._wasm = _get_wasm()
        self._db_ptr = 0
        self._closed = False
        self._isolation_level = isolation_level
        self._row_factory: Optional[Callable] = None
        self._text_factory = str
        self._in_transaction = False

        # Open the database
        db_ptr_ptr = self._wasm.malloc(self._wasm.store, 4)
        db_path_ptr = self._wasm.write_string(database)

        try:
            flags = SQLITE_OPEN_READWRITE | SQLITE_OPEN_CREATE | SQLITE_OPEN_URI
            result = self._wasm.sqlite3_open_v2(
                self._wasm.store,
                db_path_ptr,
                db_ptr_ptr,
                flags,
                0  # No VFS
            )

            self._db_ptr = self._wasm.read_int32(db_ptr_ptr)

            if result != SQLITE_OK:
                if self._db_ptr:
                    msg = self._wasm.read_string(self._wasm.sqlite3_errmsg(self._wasm.store, self._db_ptr))
                    self._wasm.sqlite3_close(self._wasm.store, self._db_ptr)
                    raise OperationalError(f"Unable to open database: {msg}")
                else:
                    raise OperationalError("Unable to open database")
        finally:
            self._wasm.free(self._wasm.store, db_path_ptr)
            self._wasm.free(self._wasm.store, db_ptr_ptr)

        # Set busy timeout
        if timeout:
            self._wasm.sqlite3_busy_timeout(self._wasm.store, self._db_ptr, int(timeout * 1000))

    @property
    def isolation_level(self) -> Optional[str]:
        return self._isolation_level

    @isolation_level.setter
    def isolation_level(self, value: Optional[str]):
        self._isolation_level = value

    @property
    def row_factory(self) -> Optional[Callable]:
        return self._row_factory

    @row_factory.setter
    def row_factory(self, factory: Optional[Callable]):
        self._row_factory = factory

    @property
    def text_factory(self) -> type:
        return self._text_factory

    @text_factory.setter
    def text_factory(self, factory: type):
        self._text_factory = factory

    @property
    def total_changes(self) -> int:
        return self._wasm.sqlite3_total_changes(self._wasm.store, self._db_ptr)

    @property
    def in_transaction(self) -> bool:
        return self._wasm.sqlite3_get_autocommit(self._wasm.store, self._db_ptr) == 0

    def _check_closed(self):
        if self._closed:
            raise ProgrammingError("Cannot operate on a closed database")

    def cursor(self) -> Cursor:
        """Create a new cursor object."""
        self._check_closed()
        return Cursor(self)

    def execute(self, sql: str, parameters: Optional[Sequence] = None) -> Cursor:
        """Execute a SQL statement and return a cursor."""
        cursor = self.cursor()
        cursor.execute(sql, parameters)
        return cursor

    def executemany(self, sql: str, seq_of_parameters: Sequence[Sequence]) -> Cursor:
        """Execute a SQL statement multiple times with different parameters."""
        cursor = self.cursor()
        cursor.executemany(sql, seq_of_parameters)
        return cursor

    def executescript(self, sql_script: str) -> Cursor:
        """Execute multiple SQL statements at once."""
        cursor = self.cursor()
        cursor.executescript(sql_script)
        return cursor

    def commit(self) -> None:
        """Commit the current transaction."""
        self._check_closed()
        if self.in_transaction:
            self.execute("COMMIT")

    def rollback(self) -> None:
        """Roll back the current transaction."""
        self._check_closed()
        if self.in_transaction:
            self.execute("ROLLBACK")

    def close(self) -> None:
        """Close the database connection."""
        if not self._closed and self._db_ptr:
            self._wasm.sqlite3_close_v2(self._wasm.store, self._db_ptr)
            self._db_ptr = 0
            self._closed = True

    def interrupt(self) -> None:
        """Interrupt a long-running query."""
        self._check_closed()
        self._wasm.sqlite3_interrupt(self._wasm.store, self._db_ptr)

    def create_function(self, name: str, narg: int, func: Callable, deterministic: bool = False) -> None:
        """Create a user-defined function (not supported in WASM)."""
        raise NotSupportedError("User-defined functions are not supported in WASM SQLite")

    def create_aggregate(self, name: str, narg: int, aggregate_class: type) -> None:
        """Create a user-defined aggregate function (not supported in WASM)."""
        raise NotSupportedError("User-defined aggregate functions are not supported in WASM SQLite")

    def create_collation(self, name: str, callable: Callable) -> None:
        """Create a user-defined collation (not supported in WASM)."""
        raise NotSupportedError("User-defined collations are not supported in WASM SQLite")

    def set_authorizer(self, authorizer_callback: Optional[Callable]) -> None:
        """Set an authorizer callback (not supported in WASM)."""
        raise NotSupportedError("Authorizer callbacks are not supported in WASM SQLite")

    def set_progress_handler(self, handler: Optional[Callable], n: int = 0) -> None:
        """Set a progress handler (not supported in WASM)."""
        raise NotSupportedError("Progress handlers are not supported in WASM SQLite")

    def set_trace_callback(self, trace_callback: Optional[Callable]) -> None:
        """Set a trace callback (not supported in WASM)."""
        raise NotSupportedError("Trace callbacks are not supported in WASM SQLite")

    def backup(self, target: 'Connection', *, pages: int = -1,
               progress: Optional[Callable] = None, name: str = "main",
               sleep: float = 0.25) -> None:
        """Create a database backup (not supported in WASM)."""
        raise NotSupportedError("Database backup is not supported in WASM SQLite")

    def iterdump(self) -> Iterator[str]:
        """Iterate over SQL statements to recreate the database."""
        self._check_closed()

        # Get list of tables
        cursor = self.cursor()
        cursor.execute("SELECT name, sql FROM sqlite_master WHERE type='table' ORDER BY name")

        yield "BEGIN TRANSACTION;"

        for name, sql in cursor.fetchall():
            if sql:
                yield f"{sql};"

                # Dump table data
                data_cursor = self.cursor()
                data_cursor.execute(f"SELECT * FROM {name}")

                if data_cursor.description:
                    for row in data_cursor:
                        values = []
                        for v in row:
                            if v is None:
                                values.append("NULL")
                            elif isinstance(v, str):
                                escaped = v.replace("'", "''")
                                values.append(f"'{escaped}'")
                            elif isinstance(v, bytes):
                                hex_str = v.hex()
                                values.append(f"X'{hex_str}'")
                            else:
                                values.append(str(v))
                        yield f"INSERT INTO {name} VALUES({','.join(values)});"

        # Get indexes
        cursor.execute("SELECT sql FROM sqlite_master WHERE type='index' AND sql IS NOT NULL ORDER BY name")
        for (sql,) in cursor.fetchall():
            yield f"{sql};"

        yield "COMMIT;"

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        if exc_type is None:
            self.commit()
        else:
            self.rollback()
        self.close()
        return False


def connect(database: str, timeout: float = 5.0,
            detect_types: int = 0,
            isolation_level: Optional[str] = "",
            check_same_thread: bool = True,
            factory: Optional[type] = None,
            cached_statements: int = 128,
            uri: bool = False) -> Connection:
    """Connect to a SQLite database.

    Args:
        database: Path to database file, or ":memory:" for in-memory database
        timeout: Connection timeout in seconds
        detect_types: Type detection flags (PARSE_DECLTYPES | PARSE_COLNAMES)
        isolation_level: Transaction isolation level
        check_same_thread: Check thread safety (ignored)
        factory: Custom connection factory
        cached_statements: Number of cached statements (ignored)
        uri: Interpret database as URI

    Returns:
        A Connection object
    """
    if factory is None:
        factory = Connection

    return factory(database, timeout=timeout, isolation_level=isolation_level,
                   detect_types=detect_types, check_same_thread=check_same_thread)


def complete_statement(sql: str) -> bool:
    """Check if SQL statement is complete."""
    wasm = _get_wasm()
    sql_ptr = wasm.write_string(sql)
    try:
        result = wasm.sqlite3_complete(wasm.store, sql_ptr)
        return result != 0
    finally:
        wasm.free(wasm.store, sql_ptr)


def enable_callback_tracebacks(flag: bool) -> None:
    """Enable or disable callback tracebacks (no-op in WASM)."""
    pass


def register_adapter(type: type, adapter: Callable) -> None:
    """Register an adapter (limited support in WASM)."""
    pass  # No-op for now


def register_converter(typename: str, converter: Callable) -> None:
    """Register a converter (limited support in WASM)."""
    pass  # No-op for now


# Compatibility with sqlite3
threadsafety = 0  # Not thread-safe in WASM
apilevel = "2.0"
paramstyle = "qmark"  # Also supports "named"
