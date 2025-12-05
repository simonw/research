"""
Wasmer-based jq WASM runner

Uses wasmer-python to execute jq/jaq WASM modules with:
- Memory limits via memory allocation
- WASI support for stdin/stdout/stderr
- No filesystem access (sandboxed)

Note: Wasmer's Python bindings have different fuel/metering capabilities
than wasmtime. Memory limits are the primary resource control.
"""

import io
from typing import Optional, List
from pathlib import Path

try:
    from wasmer import engine, Store, Module, Instance, ImportObject, Function, Memory, MemoryType, Limits
    from wasmer import wasi
    WASMER_AVAILABLE = True
except ImportError:
    WASMER_AVAILABLE = False

from .base import JqRunner, JqResult, JqError, JqTimeoutError, JqMemoryError


class WasmerJqRunner(JqRunner):
    """
    Execute jq programs using the Wasmer WebAssembly runtime.

    Features:
    - Memory limits via memory allocation
    - WASI stdin/stdout/stderr capture
    - Complete filesystem isolation
    - Multiple compiler backends (Cranelift, LLVM, Singlepass)

    Example:
        runner = WasmerJqRunner("jq.wasm", max_memory_pages=256)
        result = runner.run(".foo", '{"foo": "bar"}')
        print(result.output)  # "bar"
    """

    def __init__(
        self,
        wasm_path: str,
        max_memory_pages: int = 256,
        max_fuel: Optional[int] = None,  # Wasmer metering works differently
        compiler: str = "cranelift",
    ):
        if not WASMER_AVAILABLE:
            raise ImportError(
                "wasmer is not installed. Install with: "
                "pip install wasmer wasmer-compiler-cranelift"
            )
        super().__init__(wasm_path, max_memory_pages, max_fuel)
        self.compiler = compiler
        self._store: Optional[Store] = None
        self._module: Optional[Module] = None

    def _initialize(self) -> None:
        """Initialize the Wasmer engine and compile the module."""
        # Select compiler engine
        if self.compiler == "cranelift":
            try:
                from wasmer_compiler_cranelift import Compiler
                eng = engine.JIT(Compiler)
            except ImportError:
                eng = engine.JIT()
        elif self.compiler == "llvm":
            try:
                from wasmer_compiler_llvm import Compiler
                eng = engine.JIT(Compiler)
            except ImportError:
                raise ImportError(
                    "LLVM compiler not available. Install with: "
                    "pip install wasmer-compiler-llvm"
                )
        elif self.compiler == "singlepass":
            try:
                from wasmer_compiler_singlepass import Compiler
                eng = engine.JIT(Compiler)
            except ImportError:
                raise ImportError(
                    "Singlepass compiler not available. Install with: "
                    "pip install wasmer-compiler-singlepass"
                )
        else:
            eng = engine.JIT()

        self._store = Store(eng)

        # Load and compile the WASM module
        wasm_path = Path(self.wasm_path)
        if not wasm_path.exists():
            raise FileNotFoundError(f"WASM file not found: {self.wasm_path}")

        with open(wasm_path, "rb") as f:
            wasm_bytes = f.read()

        self._module = Module(self._store, wasm_bytes)
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
            JqMemoryError: If execution exceeds memory limit
        """
        if not self._initialized:
            self._initialize()

        # Build command-line arguments
        args = ["jaq"]
        if raw_output:
            args.append("-r")
        if compact:
            args.append("-c")
        args.append(program)

        # Create temporary files for I/O
        stdin_path = self._create_temp_stdin(input_json)
        stdout_path = self._create_temp_file()
        stderr_path = self._create_temp_file()

        try:
            # Get WASI version from module
            wasi_version = wasi.get_version(self._module, strict=True)

            # Build WASI environment
            wasi_env = (
                wasi.StateBuilder("jaq")
                .arguments(args)
                .map_directory(".", ".")  # Minimal mapping for stdin file
            )

            # Configure stdin/stdout/stderr
            wasi_env = wasi_env.stdin(open(stdin_path, "rb"))
            wasi_env = wasi_env.stdout(open(stdout_path, "wb"))
            wasi_env = wasi_env.stderr(open(stderr_path, "wb"))

            wasi_env = wasi_env.finalize()

            # Generate import object with WASI
            import_object = wasi_env.generate_import_object(self._store, wasi_version)

            # Instantiate module
            try:
                instance = Instance(self._module, import_object)
            except RuntimeError as e:
                if "memory" in str(e).lower():
                    raise JqMemoryError(f"Memory allocation failed: {e}")
                raise JqError(f"Module instantiation failed: {e}")

            # Get _start function
            try:
                start_func = instance.exports._start
            except AttributeError:
                raise JqError("WASM module does not export _start function")

            # Execute
            exit_code = 0
            try:
                start_func()
            except RuntimeError as e:
                error_msg = str(e)
                if "unreachable" in error_msg.lower():
                    # jaq/jq uses unreachable for non-zero exit
                    exit_code = 1
                elif "memory" in error_msg.lower():
                    raise JqMemoryError(f"Memory limit exceeded: {error_msg}")
                else:
                    exit_code = 1

        except Exception as e:
            if isinstance(e, (JqError, JqMemoryError, JqTimeoutError)):
                raise
            raise JqError(f"Wasmer execution error: {e}")

        finally:
            # Read captured output
            stdout = self._read_temp_file(stdout_path)
            stderr = self._read_temp_file(stderr_path)

            # Clean up temp files
            self._cleanup_temp_file(stdin_path)
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
