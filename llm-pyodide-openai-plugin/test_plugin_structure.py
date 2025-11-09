"""
Test the plugin structure without requiring pyodide.

This validates that the plugin code is structured correctly
and would work if pyodide were available.
"""

import ast
import inspect


def test_plugin_has_hookimpl():
    """Verify plugin has hookimpl decorator."""
    with open('llm_pyodide_openai.py', 'r') as f:
        code = f.read()

    assert '@hookimpl' in code, "Plugin should have @hookimpl decorator"
    assert 'def register_models' in code, "Plugin should have register_models function"
    print("✓ Plugin has hookimpl decorator and register_models function")


def test_plugin_imports():
    """Verify plugin has necessary imports."""
    with open('llm_pyodide_openai.py', 'r') as f:
        code = f.read()

    # Check for LLM imports
    assert 'from llm import KeyModel' in code or 'llm.KeyModel' in code, \
        "Plugin should import KeyModel from llm"

    # Check for pyodide fetch imports
    assert 'from js import fetch' in code, \
        "Plugin should import fetch from js"
    assert 'from pyodide.ffi import to_js' in code, \
        "Plugin should import to_js from pyodide.ffi"

    print("✓ Plugin has necessary imports")


def test_plugin_class_structure():
    """Verify plugin has a chat model class."""
    with open('llm_pyodide_openai.py', 'r') as f:
        code = f.read()

    assert 'class PyodideChat' in code, "Plugin should have PyodideChat class"
    assert 'needs_key = "openai"' in code, "Chat class should specify needs_key"
    assert 'def execute' in code, "Chat class should have execute method"
    assert 'def build_messages' in code, "Chat class should have build_messages method"

    print("✓ Plugin has PyodideChat class with required methods")


def test_plugin_uses_fetch_api():
    """Verify plugin uses browser fetch API."""
    with open('llm_pyodide_openai.py', 'r') as f:
        code = f.read()

    assert 'await fetch(' in code or 'fetch(' in code, \
        "Plugin should call fetch() function"
    assert 'api.openai.com' in code, \
        "Plugin should call OpenAI API"
    assert 'chat/completions' in code, \
        "Plugin should use chat completions endpoint"

    print("✓ Plugin uses browser fetch API to call OpenAI")


def test_plugin_registers_models():
    """Verify plugin registers models in hookimpl."""
    with open('llm_pyodide_openai.py', 'r') as f:
        code = f.read()

    # Check that register_models calls register() function
    assert 'register(' in code, "Plugin should call register() to add models"
    assert 'gpt-4o-mini' in code or 'gpt-3.5' in code, \
        "Plugin should register at least one OpenAI model"

    print("✓ Plugin registers models correctly")


def test_plugin_syntax():
    """Verify plugin has valid Python syntax."""
    with open('llm_pyodide_openai.py', 'r') as f:
        code = f.read()

    try:
        ast.parse(code)
        print("✓ Plugin has valid Python syntax")
    except SyntaxError as e:
        raise AssertionError(f"Plugin has syntax error: {e}")


def test_html_structure():
    """Verify HTML test file exists and has key elements."""
    with open('test.html', 'r') as f:
        html = f.read()

    assert '<script src="https://cdn.jsdelivr.net/pyodide/' in html, \
        "HTML should load pyodide from CDN"
    assert 'loadPyodide' in html, \
        "HTML should call loadPyodide"
    assert 'micropip' in html, \
        "HTML should use micropip to install packages"
    assert 'llm_pyodide_openai.py' in html, \
        "HTML should load the plugin file"

    print("✓ HTML test harness is structured correctly")


def run_all_tests():
    """Run all validation tests."""
    print("Running plugin structure validation tests...\n")

    tests = [
        test_plugin_syntax,
        test_plugin_has_hookimpl,
        test_plugin_imports,
        test_plugin_class_structure,
        test_plugin_uses_fetch_api,
        test_plugin_registers_models,
        test_html_structure,
    ]

    for test in tests:
        try:
            test()
        except AssertionError as e:
            print(f"✗ {test.__name__} failed: {e}")
            return False

    print("\n✓ All structure validation tests passed!")
    print("\nNote: These tests validate code structure only.")
    print("Full functional tests require a browser environment with network access.")
    return True


if __name__ == "__main__":
    import sys
    success = run_all_tests()
    sys.exit(0 if success else 1)
