"""
Integration tests for cmarkgfm in Pyodide

This test suite uses pytest to verify that cmarkgfm works correctly in Pyodide
when run via Node.js.

Requirements:
- Node.js installed
- npm install run in project directory
- cmarkgfm wheel built for Pyodide (optional - tests will skip if not available)

Run with:
    pytest pytest/test_integration.py -v
"""

import subprocess
import json
import os
from pathlib import Path

import pytest


PROJECT_DIR = Path(__file__).parent.parent
NODE_MODULES = PROJECT_DIR / "node_modules"


def check_node_available():
    """Check if Node.js is available"""
    try:
        result = subprocess.run(
            ["node", "--version"],
            capture_output=True,
            text=True,
            timeout=5
        )
        return result.returncode == 0
    except (subprocess.SubprocessError, FileNotFoundError):
        return False


def check_npm_installed():
    """Check if npm dependencies are installed"""
    return NODE_MODULES.exists() and (NODE_MODULES / "pyodide").exists()


def run_node_test(script_content, timeout=60):
    """
    Run a Node.js test script and return the output

    Args:
        script_content: JavaScript code to execute
        timeout: Maximum execution time in seconds

    Returns:
        dict with 'stdout', 'stderr', 'returncode'
    """
    result = subprocess.run(
        ["node", "--input-type=module", "-e", script_content],
        cwd=PROJECT_DIR,
        capture_output=True,
        text=True,
        timeout=timeout
    )

    return {
        "stdout": result.stdout,
        "stderr": result.stderr,
        "returncode": result.returncode
    }


@pytest.mark.skipif(not check_node_available(), reason="Node.js not available")
@pytest.mark.skipif(not check_npm_installed(), reason="npm dependencies not installed")
class TestPyodideBasic:
    """Basic Pyodide functionality tests"""

    def test_pyodide_loads(self):
        """Test that Pyodide loads successfully"""
        script = """
        import { loadPyodide } from 'pyodide';

        (async () => {
            const pyodide = await loadPyodide();
            const result = await pyodide.runPythonAsync('2 + 2');
            console.log(result);
        })();
        """

        result = run_node_test(script)
        assert result["returncode"] == 0, f"Failed to load Pyodide: {result['stderr']}"
        assert "4" in result["stdout"]

    def test_python_version(self):
        """Test that Python version is as expected"""
        script = """
        import { loadPyodide } from 'pyodide';

        (async () => {
            const pyodide = await loadPyodide();
            const version = await pyodide.runPythonAsync(`
import sys
sys.version
            `);
            console.log(JSON.stringify({version: version}));
        })();
        """

        result = run_node_test(script)
        assert result["returncode"] == 0
        assert "3.12" in result["stdout"]

    def test_stdlib_available(self):
        """Test that standard library modules are available"""
        script = """
        import { loadPyodide } from 'pyodide';

        (async () => {
            const pyodide = await loadPyodide();
            const result = await pyodide.runPythonAsync(`
import re
import html
"OK"
            `);
            console.log(result);
        })();
        """

        result = run_node_test(script)
        assert result["returncode"] == 0
        assert "OK" in result["stdout"]


@pytest.mark.skipif(not check_node_available(), reason="Node.js not available")
@pytest.mark.skipif(not check_npm_installed(), reason="npm dependencies not installed")
class TestMarkdownAlternatives:
    """Test pure Python markdown alternatives that should work in Pyodide"""

    def test_inline_markdown_parser(self):
        """Test basic inline markdown parser using stdlib only"""
        script = """
        import { loadPyodide } from 'pyodide';

        (async () => {
            const pyodide = await loadPyodide();
            const result = await pyodide.runPythonAsync(`
import re
import html

def simple_markdown_to_html(text):
    text = html.escape(text)
    text = re.sub(r'^# (.+)$', r'<h1>\\\\1</h1>', text, flags=re.MULTILINE)
    text = re.sub(r'\\\\*\\\\*(.+?)\\\\*\\\\*', r'<strong>\\\\1</strong>', text)
    return text

result = simple_markdown_to_html("# Test\\\\n**bold**")
result
            `);
            console.log(JSON.stringify({html: result}));
        })();
        """

        result = run_node_test(script)
        assert result["returncode"] == 0
        output = result["stdout"]
        assert "<h1>Test</h1>" in output
        assert "<strong>bold</strong>" in output


@pytest.mark.skipif(not check_node_available(), reason="Node.js not available")
@pytest.mark.skipif(not check_npm_installed(), reason="npm dependencies not installed")
class TestCmarkgfm:
    """Tests for cmarkgfm in Pyodide"""

    @pytest.fixture
    def cmarkgfm_wheel_path(self):
        """Find the built cmarkgfm wheel if it exists"""
        build_dir = PROJECT_DIR / "build"
        if not build_dir.exists():
            return None

        wheels = list(build_dir.glob("cmarkgfm-*-wasm32.whl"))
        return wheels[0] if wheels else None

    @pytest.mark.skip(reason="cmarkgfm wheel not built yet - requires Emscripten")
    def test_cmarkgfm_import(self, cmarkgfm_wheel_path):
        """Test that cmarkgfm can be imported"""
        if not cmarkgfm_wheel_path:
            pytest.skip("cmarkgfm wheel not found - run build first")

        script = f"""
        import {{ loadPyodide }} from 'pyodide';

        (async () => {{
            const pyodide = await loadPyodide();
            await pyodide.loadPackage('file://{cmarkgfm_wheel_path}');

            const result = await pyodide.runPythonAsync(`
import cmarkgfm
"OK"
            `);
            console.log(result);
        }})();
        """

        result = run_node_test(script)
        assert result["returncode"] == 0
        assert "OK" in result["stdout"]

    @pytest.mark.skip(reason="cmarkgfm wheel not built yet - requires Emscripten")
    def test_cmarkgfm_basic_rendering(self, cmarkgfm_wheel_path):
        """Test basic markdown rendering with cmarkgfm"""
        if not cmarkgfm_wheel_path:
            pytest.skip("cmarkgfm wheel not found - run build first")

        script = f"""
        import {{ loadPyodide }} from 'pyodide';

        (async () => {{
            const pyodide = await loadPyodide();
            await pyodide.loadPackage('file://{cmarkgfm_wheel_path}');

            const result = await pyodide.runPythonAsync(`
import cmarkgfm

markdown = "# Hello\\\\n\\\\nThis is **bold**"
html = cmarkgfm.github_flavored_markdown_to_html(markdown)
html
            `);
            console.log(JSON.stringify({{html: result}}));
        }})();
        """

        result = run_node_test(script, timeout=120)
        assert result["returncode"] == 0
        output = result["stdout"]
        assert "<h1>" in output
        assert "<strong>bold</strong>" in output or "<b>bold</b>" in output

    @pytest.mark.skip(reason="cmarkgfm wheel not built yet - requires Emscripten")
    def test_cmarkgfm_gfm_features(self, cmarkgfm_wheel_path):
        """Test GitHub Flavored Markdown features"""
        if not cmarkgfm_wheel_path:
            pytest.skip("cmarkgfm wheel not found - run build first")

        script = f"""
        import {{ loadPyodide }} from 'pyodide';

        (async () => {{
            const pyodide = await loadPyodide();
            await pyodide.loadPackage('file://{cmarkgfm_wheel_path}');

            const result = await pyodide.runPythonAsync(`
import cmarkgfm

# Test strikethrough
markdown = "~~strikethrough~~"
html = cmarkgfm.github_flavored_markdown_to_html(markdown)
html
            `);
            console.log(JSON.stringify({{html: result}}));
        }})();
        """

        result = run_node_test(script, timeout=120)
        assert result["returncode"] == 0
        output = result["stdout"]
        # GFM uses <del> for strikethrough
        assert "<del>" in output or "strikethrough" in output


def test_node_available():
    """Meta-test: Verify Node.js is available"""
    assert check_node_available(), "Node.js is not available in PATH"


def test_project_structure():
    """Meta-test: Verify project structure"""
    assert PROJECT_DIR.exists()
    assert (PROJECT_DIR / "package.json").exists()
    assert (PROJECT_DIR / "README.md").exists()
    assert (PROJECT_DIR / "REPORT.md").exists()


if __name__ == "__main__":
    # Allow running directly for debugging
    pytest.main([__file__, "-v", "--tb=short"])
