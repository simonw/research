"""JSONL runner for Python WASM - maintains persistent state across requests."""
import sys
import json
import traceback
from io import StringIO

# Global namespace for persistent state
_globals = {}
_locals = {}

def execute_code(code):
    """Execute Python code and return stdout output."""
    old_stdout = sys.stdout
    old_stderr = sys.stderr

    stdout_capture = StringIO()
    stderr_capture = StringIO()

    sys.stdout = stdout_capture
    sys.stderr = stderr_capture

    error = None
    try:
        # Try to evaluate as expression first (for things like "1+1")
        try:
            result = eval(code, _globals, _locals)
            if result is not None:
                print(repr(result))
        except SyntaxError:
            # Fall back to exec for statements
            exec(code, _globals, _locals)
    except Exception:
        error = traceback.format_exc()
    finally:
        sys.stdout = old_stdout
        sys.stderr = old_stderr

    return stdout_capture.getvalue(), stderr_capture.getvalue(), error

def main():
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as e:
            response = {"id": "", "error": f"Invalid JSON: {e}"}
            print(json.dumps(response), flush=True)
            continue

        req_id = request.get("id", "")
        code = request.get("code", "")

        stdout_out, stderr_out, error = execute_code(code)

        response = {"id": req_id}

        output = stdout_out
        if stderr_out:
            output += stderr_out

        if output:
            response["output"] = output

        if error:
            response["error"] = error

        print(json.dumps(response), flush=True)

if __name__ == "__main__":
    main()
