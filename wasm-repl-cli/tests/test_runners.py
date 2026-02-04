"""Tests for jsrunner and pyrunner WASM REPL CLIs."""
import json
import subprocess
import os
from pathlib import Path

# Get the project root directory
PROJECT_ROOT = Path(__file__).parent.parent
JSRUNNER = PROJECT_ROOT / "jsrunner"
PYRUNNER = PROJECT_ROOT / "pyrunner"
RUNTIME = PROJECT_ROOT / "runtime"


class TestJSRunner:
    """Tests for the JavaScript WASM runner."""

    def test_simple_eval(self):
        """Test simple JavaScript evaluation with -e flag."""
        result = subprocess.run(
            [str(JSRUNNER), "-e", "console.log(1+2+3)"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        assert result.stdout.strip() == "6"

    def test_variable_assignment(self):
        """Test variable assignment and output."""
        result = subprocess.run(
            [str(JSRUNNER), "-e", "var x = 42; console.log(x)"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        assert result.stdout.strip() == "42"

    def test_function_definition(self):
        """Test function definition and call."""
        code = "function double(n) { return n * 2; } console.log(double(21))"
        result = subprocess.run(
            [str(JSRUNNER), "-e", code],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        assert result.stdout.strip() == "42"

    def test_jsonl_single_request(self):
        """Test JSONL mode with a single request."""
        request = json.dumps({"id": "test-1", "code": "console.log(2**10)"})
        result = subprocess.run(
            [str(JSRUNNER), "--jsonl"],
            input=request + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        response = json.loads(result.stdout.strip())
        assert response["id"] == "test-1"
        assert response["output"].strip() == "1024"

    def test_jsonl_state_persistence(self):
        """Test that state persists between JSONL requests."""
        requests = [
            json.dumps({"id": "a", "code": "var counter = 0"}),
            json.dumps({"id": "b", "code": "counter++; console.log(counter)"}),
            json.dumps({"id": "c", "code": "counter++; console.log(counter)"}),
            json.dumps({"id": "d", "code": "counter++; console.log(counter)"}),
        ]
        result = subprocess.run(
            [str(JSRUNNER), "--jsonl"],
            input="\n".join(requests) + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0

        lines = result.stdout.strip().split("\n")
        assert len(lines) == 4

        responses = [json.loads(line) for line in lines]
        assert responses[0]["id"] == "a"
        assert responses[1]["id"] == "b"
        assert responses[1]["output"].strip() == "1"
        assert responses[2]["id"] == "c"
        assert responses[2]["output"].strip() == "2"
        assert responses[3]["id"] == "d"
        assert responses[3]["output"].strip() == "3"

    def test_jsonl_function_persistence(self):
        """Test that function definitions persist between JSONL requests."""
        requests = [
            json.dumps({"id": "def", "code": "function greet(name) { return 'Hello, ' + name + '!'; }"}),
            json.dumps({"id": "call", "code": "console.log(greet('World'))"}),
        ]
        result = subprocess.run(
            [str(JSRUNNER), "--jsonl"],
            input="\n".join(requests) + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0

        lines = result.stdout.strip().split("\n")
        responses = [json.loads(line) for line in lines]
        assert responses[1]["output"].strip() == "Hello, World!"

    def test_jsonl_error_handling(self):
        """Test that errors are properly reported in JSONL mode."""
        request = json.dumps({"id": "err", "code": "throw new Error('test error')"})
        result = subprocess.run(
            [str(JSRUNNER), "--jsonl"],
            input=request + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        response = json.loads(result.stdout.strip())
        assert response["id"] == "err"
        assert "error" in response


class TestPyRunner:
    """Tests for the Python WASM runner."""

    def test_simple_eval(self):
        """Test simple Python evaluation with -c flag."""
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "-c", "print(1+2+3)"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        assert result.stdout.strip() == "6"

    def test_variable_assignment(self):
        """Test variable assignment and output."""
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "-c", "x = 42; print(x)"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        assert result.stdout.strip() == "42"

    def test_function_definition(self):
        """Test function definition and call."""
        code = "def double(n): return n * 2\nprint(double(21))"
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "-c", code],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        assert result.stdout.strip() == "42"

    def test_import_json(self):
        """Test importing the json module."""
        code = "import json; print(json.dumps({'key': 'value'}))"
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "-c", code],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        assert json.loads(result.stdout.strip()) == {"key": "value"}

    def test_jsonl_single_request(self):
        """Test JSONL mode with a single request."""
        request = json.dumps({"id": "test-1", "code": "print(2**10)"})
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "--jsonl"],
            input=request + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        response = json.loads(result.stdout.strip())
        assert response["id"] == "test-1"
        assert response["output"].strip() == "1024"

    def test_jsonl_state_persistence(self):
        """Test that state persists between JSONL requests."""
        requests = [
            json.dumps({"id": "a", "code": "counter = 0"}),
            json.dumps({"id": "b", "code": "counter += 1; print(counter)"}),
            json.dumps({"id": "c", "code": "counter += 1; print(counter)"}),
            json.dumps({"id": "d", "code": "counter += 1; print(counter)"}),
        ]
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "--jsonl"],
            input="\n".join(requests) + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0

        lines = result.stdout.strip().split("\n")
        assert len(lines) == 4

        responses = [json.loads(line) for line in lines]
        assert responses[0]["id"] == "a"
        assert responses[1]["id"] == "b"
        assert responses[1]["output"].strip() == "1"
        assert responses[2]["id"] == "c"
        assert responses[2]["output"].strip() == "2"
        assert responses[3]["id"] == "d"
        assert responses[3]["output"].strip() == "3"

    def test_jsonl_function_persistence(self):
        """Test that function definitions persist between JSONL requests."""
        requests = [
            json.dumps({"id": "def", "code": "def greet(name): return f'Hello, {name}!'"}),
            json.dumps({"id": "call", "code": "print(greet('World'))"}),
        ]
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "--jsonl"],
            input="\n".join(requests) + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0

        lines = result.stdout.strip().split("\n")
        responses = [json.loads(line) for line in lines]
        assert responses[1]["output"].strip() == "Hello, World!"

    def test_jsonl_list_comprehension(self):
        """Test list comprehension state persistence."""
        requests = [
            json.dumps({"id": "create", "code": "numbers = [x**2 for x in range(5)]"}),
            json.dumps({"id": "print", "code": "print(numbers)"}),
        ]
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "--jsonl"],
            input="\n".join(requests) + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0

        lines = result.stdout.strip().split("\n")
        responses = [json.loads(line) for line in lines]
        assert responses[1]["output"].strip() == "[0, 1, 4, 9, 16]"

    def test_jsonl_error_handling(self):
        """Test that errors are properly reported in JSONL mode."""
        request = json.dumps({"id": "err", "code": "raise ValueError('test error')"})
        result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "--jsonl"],
            input=request + "\n",
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        response = json.loads(result.stdout.strip())
        assert response["id"] == "err"
        assert "error" in response
        assert "ValueError" in response["error"]


class TestIntegration:
    """Integration tests comparing JavaScript and Python behavior."""

    def test_both_runners_handle_math(self):
        """Test that both runners handle basic math correctly."""
        js_result = subprocess.run(
            [str(JSRUNNER), "-e", "console.log(Math.pow(2, 8))"],
            capture_output=True,
            text=True,
        )
        py_result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "-c", "print(2**8)"],
            capture_output=True,
            text=True,
        )
        assert js_result.stdout.strip() == py_result.stdout.strip() == "256"

    def test_jsonl_uuid_tracking(self):
        """Test that UUIDs are properly tracked in responses."""
        import uuid

        # Test JavaScript
        js_uuid = str(uuid.uuid4())
        js_request = json.dumps({"id": js_uuid, "code": "console.log('js')"})
        js_result = subprocess.run(
            [str(JSRUNNER), "--jsonl"],
            input=js_request + "\n",
            capture_output=True,
            text=True,
        )
        js_response = json.loads(js_result.stdout.strip())
        assert js_response["id"] == js_uuid

        # Test Python
        py_uuid = str(uuid.uuid4())
        py_request = json.dumps({"id": py_uuid, "code": "print('py')"})
        py_result = subprocess.run(
            [str(PYRUNNER), "-runtime", str(RUNTIME), "--jsonl"],
            input=py_request + "\n",
            capture_output=True,
            text=True,
        )
        py_response = json.loads(py_result.stdout.strip())
        assert py_response["id"] == py_uuid
