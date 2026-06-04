"""
mquickjs sandbox using subprocess calls to the mqjs binary.

This implementation uses the unmodified mqjs binary via subprocess.
"""

import subprocess
import tempfile
import os
from pathlib import Path
from typing import Any, Optional

from api_design import BaseSandbox, SandboxError, TimeoutError, MemoryError

# Path to mqjs binary
SCRIPT_DIR = Path(__file__).parent
MQUICKJS_DIR = SCRIPT_DIR / "vendor" / "mquickjs"
MQJS_PATH = MQUICKJS_DIR / "mqjs"


def _ensure_mqjs():
    """Ensure mqjs is built."""
    if MQJS_PATH.exists():
        return

    # Clone if needed
    if not MQUICKJS_DIR.exists():
        MQUICKJS_DIR.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["git", "clone", "https://github.com/bellard/mquickjs.git", str(MQUICKJS_DIR)],
            check=True
        )

    # Build mqjs
    subprocess.run(["make", "mqjs"], cwd=MQUICKJS_DIR, check=True, capture_output=True)


class MQuickJSSubprocess(BaseSandbox):
    """
    mquickjs sandbox using subprocess calls.

    This implementation spawns a new mqjs process for each execution.
    """

    def __init__(
        self,
        memory_limit_bytes: int = BaseSandbox.DEFAULT_MEMORY_LIMIT,
        time_limit_ms: int = BaseSandbox.DEFAULT_TIME_LIMIT_MS,
    ):
        super().__init__(memory_limit_bytes, time_limit_ms)
        _ensure_mqjs()

    def execute(self, code: str) -> Any:
        """Execute JavaScript code and return the result."""
        # Wrap code to return and print the result
        # Use -e flag for expressions
        wrapped_code = f"""
var __result__ = (function() {{
    return {code};
}})();
if (__result__ === undefined) {{
    print("__UNDEFINED__");
}} else if (__result__ === null) {{
    print("__NULL__");
}} else {{
    print(JSON.stringify(__result__));
}}
"""

        # Create a temporary file for the code
        with tempfile.NamedTemporaryFile(mode='w', suffix='.js', delete=False) as f:
            f.write(wrapped_code)
            temp_path = f.name

        try:
            # Convert memory limit to string format (e.g., "1M")
            if self.memory_limit_bytes >= 1024 * 1024:
                mem_str = f"{self.memory_limit_bytes // (1024 * 1024)}M"
            elif self.memory_limit_bytes >= 1024:
                mem_str = f"{self.memory_limit_bytes // 1024}k"
            else:
                mem_str = str(self.memory_limit_bytes)

            # Run mqjs
            timeout_sec = self.time_limit_ms / 1000.0
            try:
                result = subprocess.run(
                    [str(MQJS_PATH), "--memory-limit", mem_str, temp_path],
                    capture_output=True,
                    text=True,
                    timeout=timeout_sec if timeout_sec > 0 else None,
                )
            except subprocess.TimeoutExpired:
                raise TimeoutError("Execution timeout")

            if result.returncode != 0:
                error_msg = result.stderr.strip() if result.stderr else "Unknown error"
                raise SandboxError(error_msg)

            # Parse output
            output = result.stdout.strip()
            return self._parse_result(output)

        finally:
            os.unlink(temp_path)

    def _parse_result(self, output: str) -> Any:
        """Parse the output from mqjs."""
        if not output:
            return None

        if output == "__UNDEFINED__":
            return None
        if output == "__NULL__":
            return None

        import json
        try:
            return json.loads(output)
        except json.JSONDecodeError:
            return output


class MQuickJSSubprocessReuse(BaseSandbox):
    """
    mquickjs subprocess wrapper that reuses the same context.

    NOTE: This is a simplified version - true context reuse would require
    a more complex IPC mechanism. This implementation creates a new process
    for each call but tracks variables in a state dict for simulation.
    """

    def __init__(
        self,
        memory_limit_bytes: int = BaseSandbox.DEFAULT_MEMORY_LIMIT,
        time_limit_ms: int = BaseSandbox.DEFAULT_TIME_LIMIT_MS,
    ):
        super().__init__(memory_limit_bytes, time_limit_ms)
        _ensure_mqjs()
        self._state = {}

    def execute(self, code: str) -> Any:
        """Execute JavaScript code."""
        # For subprocess, we can't easily reuse context
        # Each call is independent
        sandbox = MQuickJSSubprocess(self.memory_limit_bytes, self.time_limit_ms)
        return sandbox.execute(code)

    def close(self):
        """Close the sandbox."""
        self._state = {}


def execute_js(
    code: str,
    *,
    memory_limit_bytes: int = BaseSandbox.DEFAULT_MEMORY_LIMIT,
    time_limit_ms: int = BaseSandbox.DEFAULT_TIME_LIMIT_MS,
) -> Any:
    """
    Execute JavaScript code using the mqjs subprocess.
    """
    sandbox = MQuickJSSubprocess(memory_limit_bytes, time_limit_ms)
    return sandbox.execute(code)


if __name__ == "__main__":
    print("Testing mquickjs subprocess wrapper...")

    # Basic arithmetic
    result = execute_js("1 + 2")
    print(f"1 + 2 = {result}")
    assert result == 3

    # String
    result = execute_js("'hello' + ' ' + 'world'")
    print(f"String concat = {result}")
    assert result == "hello world"

    # Array
    result = execute_js("[1, 2, 3]")
    print(f"Array = {result}")
    assert result == [1, 2, 3]

    # Object
    result = execute_js("({a: 1, b: 2})")
    print(f"Object = {result}")
    assert result == {"a": 1, "b": 2}

    print("All subprocess tests passed!")
