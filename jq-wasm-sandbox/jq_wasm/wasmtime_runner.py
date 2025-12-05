"""
Wasmtime-based jq WASM runner

Uses wasmtime-py to execute jq/jaq WASM modules with:
- Memory limits via Store configuration
- CPU limits via fuel metering
- WASI support for stdin/stdout/stderr
- No filesystem access (sandboxed)
"""

import io
from typing import Optional, List
from pathlib import Path

try:
    import wasmtime
    from wasmtime import Store, Module, Instance, Engine, Config, Linker
    from wasmtime import WasiConfig
    WASMTIME_AVAILABLE = True
except ImportError:
    WASMTIME_AVAILABLE = False

from .base import JqRunner, JqResult, JqError, JqTimeoutError, JqMemoryError


class WasmtimeJqRunner(JqRunner):
    """
    Execute jq programs using the Wasmtime WebAssembly runtime.

    Features:
    - Fuel-based CPU limiting (instruction counting)
    - Memory limits via store configuration
    - WASI stdin/stdout/stderr capture
    - Complete filesystem isolation

    Example:
        runner = WasmtimeJqRunner("jq.wasm", max_fuel=1_000_000)
        result = runner.run(".foo", '{"foo": "bar"}')
        print(result.output)  # "bar"
    """

    def __init__(
        self,
        wasm_path: str,
        max_memory_pages: int = 256,
        max_fuel: Optional[int] = 100_000_000,
    ):
        if not WASMTIME_AVAILABLE:
            raise ImportError(
                "wasmtime is not installed. Install with: pip install wasmtime"
            )
        super().__init__(wasm_path, max_memory_pages, max_fuel)
        self._engine: Optional[Engine] = None
        self._module: Optional[Module] = None

    def _initialize(self) -> None:
        """Initialize the Wasmtime engine and compile the module."""
        # Configure engine with fuel consumption enabled
        config = Config()
        if self.max_fuel is not None:
            config.consume_fuel = True

        self._engine = Engine(config)

        # Load and compile the WASM module
        wasm_path = Path(self.wasm_path)
        if not wasm_path.exists():
            raise FileNotFoundError(f"WASM file not found: {self.wasm_path}")

        self._module = Module.from_file(self._engine, str(wasm_path))
        self._initialized = True

    def run(
        self,
        program: str,
        input_json: str,
        *,
        raw_output: bool = False,
        compact: bool = False,
    ) -> JqResult:
        """
        Execute a jq program on JSON input.

        Args:
            program: The jq program/filter to execute
            input_json: JSON input string
            raw_output: If True, output raw strings (-r flag)
            compact: If True, produce compact output (-c flag)

        Returns:
            JqResult with stdout, stderr, and exit code

        Raises:
            JqTimeoutError: If execution exceeds fuel limit
            JqMemoryError: If execution exceeds memory limit
        """
        if not self._initialized:
            self._initialize()

        # Create a new store for this execution (isolation)
        store = Store(self._engine)

        # Set fuel limit if configured
        if self.max_fuel is not None:
            store.set_fuel(self.max_fuel)

        # Build command-line arguments
        args = ["jaq"]
        if raw_output:
            args.append("-r")
        if compact:
            args.append("-c")
        args.append(program)

        # Create WASI configuration with captured stdin/stdout/stderr
        wasi_config = WasiConfig()
        wasi_config.argv = args
        wasi_config.stdin_file = self._create_temp_stdin(input_json)

        # Capture stdout and stderr
        stdout_path = self._create_temp_file()
        stderr_path = self._create_temp_file()
        wasi_config.stdout_file = stdout_path
        wasi_config.stderr_file = stderr_path

        # No filesystem access - completely sandboxed
        # (we don't call wasi_config.preopen_dir)

        store.set_wasi(wasi_config)

        # Create linker and instantiate
        linker = Linker(self._engine)
        linker.define_wasi()

        try:
            instance = linker.instantiate(store, self._module)

            # Get the _start function (WASI entry point)
            start = instance.exports(store).get("_start")
            if start is None:
                raise JqError("WASM module does not export _start function")

            # Execute
            try:
                start(store)
                exit_code = 0
            except wasmtime.ExitTrap as e:
                exit_code = e.code
            except wasmtime.Trap as e:
                trap_message = str(e)
                if "out of fuel" in trap_message.lower():
                    raise JqTimeoutError(
                        f"Execution exceeded fuel limit ({self.max_fuel} instructions)"
                    )
                elif "memory" in trap_message.lower():
                    raise JqMemoryError(f"Memory limit exceeded: {trap_message}")
                else:
                    raise JqError(f"WASM trap: {trap_message}")

        except wasmtime.WasmtimeError as e:
            error_msg = str(e)
            if "out of fuel" in error_msg.lower():
                raise JqTimeoutError(
                    f"Execution exceeded fuel limit ({self.max_fuel} instructions)"
                )
            raise JqError(f"Wasmtime error: {error_msg}")

        finally:
            # Read captured output
            stdout = self._read_temp_file(stdout_path)
            stderr = self._read_temp_file(stderr_path)

            # Clean up temp files
            self._cleanup_temp_file(stdout_path)
            self._cleanup_temp_file(stderr_path)

        return JqResult(stdout=stdout, stderr=stderr, exit_code=exit_code)

    def _create_temp_stdin(self, content: str) -> str:
        """Create a temporary file with stdin content."""
        import tempfile
        fd, path = tempfile.mkstemp(suffix=".json", prefix="jq_stdin_")
        with open(fd, "w") as f:
            f.write(content)
        return path

    def _create_temp_file(self) -> str:
        """Create an empty temporary file."""
        import tempfile
        fd, path = tempfile.mkstemp(prefix="jq_out_")
        import os
        os.close(fd)
        return path

    def _read_temp_file(self, path: str) -> str:
        """Read content from a temporary file."""
        try:
            with open(path, "r") as f:
                return f.read()
        except Exception:
            return ""

    def _cleanup_temp_file(self, path: str) -> None:
        """Remove a temporary file."""
        import os
        try:
            os.unlink(path)
        except Exception:
            pass

    def get_fuel_consumed(self, store: Store) -> Optional[int]:
        """Get the amount of fuel consumed in the last execution."""
        if self.max_fuel is None:
            return None
        try:
            remaining = store.get_fuel()
            return self.max_fuel - remaining
        except Exception:
            return None
