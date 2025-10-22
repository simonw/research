"""
Basic pytest scope demonstration.

Pytest fixtures support 5 different scopes:
1. function (default) - runs once per test function
2. class - runs once per test class
3. module - runs once per test module (file)
4. package - runs once per test package (directory)
5. session - runs once per test session (entire pytest run)
"""

import pytest

# Track how many times each fixture is called
call_counts = {
    'function': 0,
    'class': 0,
    'module': 0,
    'session': 0,
}


@pytest.fixture(scope='function')
def function_scope():
    """Default scope - runs once per test function."""
    call_counts['function'] += 1
    print(f"\n[FUNCTION SCOPE] Setup (call #{call_counts['function']})")
    yield f"function-{call_counts['function']}"
    print(f"[FUNCTION SCOPE] Teardown (call #{call_counts['function']})")


@pytest.fixture(scope='class')
def class_scope():
    """Runs once per test class."""
    call_counts['class'] += 1
    print(f"\n[CLASS SCOPE] Setup (call #{call_counts['class']})")
    yield f"class-{call_counts['class']}"
    print(f"[CLASS SCOPE] Teardown (call #{call_counts['class']})")


@pytest.fixture(scope='module')
def module_scope():
    """Runs once per test module."""
    call_counts['module'] += 1
    print(f"\n[MODULE SCOPE] Setup (call #{call_counts['module']})")
    yield f"module-{call_counts['module']}"
    print(f"[MODULE SCOPE] Teardown (call #{call_counts['module']})")


@pytest.fixture(scope='session')
def session_scope():
    """Runs once per entire test session."""
    call_counts['session'] += 1
    print(f"\n[SESSION SCOPE] Setup (call #{call_counts['session']})")
    yield f"session-{call_counts['session']}"
    print(f"[SESSION SCOPE] Teardown (call #{call_counts['session']})")


# Standalone test functions using function scope
def test_function_1(function_scope, module_scope, session_scope):
    """First standalone test."""
    print(f"Test 1: {function_scope}, {module_scope}, {session_scope}")
    assert function_scope == "function-1"
    assert module_scope == "module-1"
    assert session_scope == "session-1"


def test_function_2(function_scope, module_scope, session_scope):
    """Second standalone test - function_scope will be different."""
    print(f"Test 2: {function_scope}, {module_scope}, {session_scope}")
    assert function_scope == "function-2"  # New instance
    assert module_scope == "module-1"      # Same instance as test_function_1
    assert session_scope == "session-1"    # Same instance as test_function_1


# Test class to demonstrate class scope
class TestClassScope:
    """Tests grouped in a class to demonstrate class scope."""

    def test_class_1(self, function_scope, class_scope, module_scope, session_scope):
        """First test in class."""
        print(f"Class Test 1: {function_scope}, {class_scope}, {module_scope}, {session_scope}")
        assert function_scope == "function-3"  # New function scope
        assert class_scope == "class-1"        # New class scope for this class
        assert module_scope == "module-1"      # Same module scope
        assert session_scope == "session-1"    # Same session scope

    def test_class_2(self, function_scope, class_scope, module_scope, session_scope):
        """Second test in class - class_scope will be the same."""
        print(f"Class Test 2: {function_scope}, {class_scope}, {module_scope}, {session_scope}")
        assert function_scope == "function-4"  # New function scope
        assert class_scope == "class-1"        # Same class scope as test_class_1
        assert module_scope == "module-1"      # Same module scope
        assert session_scope == "session-1"    # Same session scope


# Another test class to show class scope creates new instance
class TestAnotherClass:
    """Second class to show class scope creates new instance."""

    def test_another_1(self, class_scope):
        """Test in different class gets new class scope."""
        print(f"Another Class Test: {class_scope}")
        assert class_scope == "class-2"  # New class scope instance
