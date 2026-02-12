import llm
import json
import pytest
from llm_rlm import RLMToolbox


class TestRLMToolboxRegistration:
    """Test that the toolbox registers correctly with LLM."""

    def test_toolbox_registers(self):
        """RLMToolbox should be importable and be a Toolbox subclass."""
        assert issubclass(RLMToolbox, llm.Toolbox)

    def test_register_tools_hook(self):
        """The register_tools hook should register RLMToolbox."""
        from llm_rlm import register_tools

        registered = []
        register_tools(registered.append)
        assert RLMToolbox in registered


class TestRLMToolboxMethods:
    """Test the individual tool methods on RLMToolbox."""

    def test_has_execute_python(self):
        """Toolbox should have an execute_python method."""
        toolbox = RLMToolbox()
        assert hasattr(toolbox, "execute_python")
        assert callable(toolbox.execute_python)

    def test_has_submit_answer(self):
        """Toolbox should have a submit_answer method."""
        toolbox = RLMToolbox()
        assert hasattr(toolbox, "submit_answer")
        assert callable(toolbox.submit_answer)


class TestExecutePython:
    """Test the execute_python tool with the pyeryx sandbox."""

    def test_simple_print(self):
        """Executing print should return the output."""
        toolbox = RLMToolbox()
        result = toolbox.execute_python(code='print("hello world")')
        assert "hello world" in result

    def test_state_persists_across_calls(self):
        """Variables should persist between execute_python calls."""
        toolbox = RLMToolbox()
        toolbox.execute_python(code="x = 42")
        result = toolbox.execute_python(code="print(x * 2)")
        assert "84" in result

    def test_context_variable_available(self):
        """When context is set, it should be available as a variable."""
        toolbox = RLMToolbox(context="This is the long context data.")
        result = toolbox.execute_python(code="print(len(context))")
        assert "30" in result

    def test_context_variable_content(self):
        """Context variable should contain the exact text passed in."""
        text = "needle in a haystack"
        toolbox = RLMToolbox(context=text)
        result = toolbox.execute_python(code="print(context)")
        assert "needle in a haystack" in result

    def test_error_handling(self):
        """Errors in sandbox code should be caught and returned."""
        toolbox = RLMToolbox()
        result = toolbox.execute_python(code="1/0")
        assert "Error" in result or "error" in result or "ZeroDivision" in result

    def test_output_truncation(self):
        """Long output should be truncated."""
        toolbox = RLMToolbox(max_output_chars=100)
        result = toolbox.execute_python(code='print("x" * 1000)')
        assert len(result) <= 200  # some headroom for truncation message

    def test_multiline_code(self):
        """Multi-line code should execute correctly."""
        toolbox = RLMToolbox()
        code = """
items = [1, 2, 3, 4, 5]
total = sum(items)
print(f"Sum: {total}")
"""
        result = toolbox.execute_python(code=code)
        assert "Sum: 15" in result


class TestSubmitAnswer:
    """Test the submit_answer tool."""

    def test_submit_direct_answer(self):
        """submit_answer with text should return confirmation."""
        toolbox = RLMToolbox()
        result = toolbox.submit_answer(answer="42")
        assert "42" in result

    def test_submit_from_variable(self):
        """submit_answer with variable_name should read from session."""
        toolbox = RLMToolbox()
        toolbox.execute_python(code='final_ans = "the answer is 42"')
        result = toolbox.submit_answer(variable_name="final_ans")
        assert "the answer is 42" in result

    def test_submit_missing_variable(self):
        """submit_answer with nonexistent variable should report error."""
        toolbox = RLMToolbox()
        result = toolbox.submit_answer(variable_name="nonexistent")
        assert "not found" in result.lower() or "error" in result.lower()


class TestLLMQueryCallback:
    """Test that llm_query is available in the sandbox."""

    def test_llm_query_available(self):
        """llm_query should be callable in the sandbox."""
        toolbox = RLMToolbox()
        # Just test that the function exists - it will fail without a real model
        # but the function should be registered
        result = toolbox.execute_python(
            code="print(type(llm_query).__name__)"
        )
        assert "function" in result.lower() or "coroutine" in result.lower()

    def test_llm_batch_available(self):
        """llm_batch should be callable in the sandbox."""
        toolbox = RLMToolbox()
        result = toolbox.execute_python(
            code="print(type(llm_batch).__name__)"
        )
        assert "function" in result.lower() or "coroutine" in result.lower()


class TestContextNeverInPrompt:
    """Verify context is ONLY in the sandbox, never in model context."""

    def test_context_not_in_toolbox_description(self):
        """The toolbox methods' docstrings should not contain any context."""
        toolbox = RLMToolbox(context="SECRET_CONTEXT_DATA_12345")
        # Check that tools descriptions don't leak the context
        for name in dir(toolbox):
            if not name.startswith("_"):
                method = getattr(toolbox, name)
                if callable(method) and hasattr(method, "__doc__") and method.__doc__:
                    assert "SECRET_CONTEXT_DATA_12345" not in method.__doc__

    def test_context_only_in_sandbox(self):
        """Context should only be accessible through sandbox execution."""
        toolbox = RLMToolbox(context="SECRET_CONTEXT_DATA_12345")
        result = toolbox.execute_python(code='print("found" if "SECRET" in context else "missing")')
        assert "found" in result
