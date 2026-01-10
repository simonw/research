"""Tests for CaMeL interpreter."""

import pytest

from camel.interpreter import CaMeLInterpreter, DataFlowGraph, InterpreterState
from camel.config import InterpreterMode
from camel.errors import InterpreterError, SecurityPolicyViolation
from camel.policies import SecurityPolicy, PolicyDecision, SecurityPolicyResult
from camel.types import CaMeLValue, Capability, DataSource, Public
from camel.llm import MockQLLM


class TestDataFlowGraph:
    """Tests for DataFlowGraph."""

    def test_add_variable(self):
        """Test adding variables to the graph."""
        graph = DataFlowGraph()
        value = CaMeLValue(raw="test")

        graph.add_variable("x", value)

        assert graph.get_value("x") is value

    def test_add_dependency(self):
        """Test adding dependencies between variables."""
        graph = DataFlowGraph()
        v1 = CaMeLValue(raw="v1")
        v2 = CaMeLValue(raw="v2")

        graph.add_variable("x", v1)
        graph.add_variable("y", v2)
        graph.add_dependency("y", "x")

        deps = graph.get_all_dependencies("y")
        assert "x" in deps

    def test_transitive_dependencies(self):
        """Test getting transitive dependencies."""
        graph = DataFlowGraph()

        graph.add_variable("a", CaMeLValue(raw="a"))
        graph.add_variable("b", CaMeLValue(raw="b"))
        graph.add_variable("c", CaMeLValue(raw="c"))

        graph.add_dependency("b", "a")
        graph.add_dependency("c", "b")

        deps = graph.get_all_dependencies("c")
        assert "a" in deps
        assert "b" in deps

    def test_control_flow_context(self):
        """Test control flow context management."""
        graph = DataFlowGraph()

        graph.push_control_flow_context({"x", "y"})
        assert graph.get_control_flow_dependencies() == {"x", "y"}

        graph.push_control_flow_context({"z"})
        assert graph.get_control_flow_dependencies() == {"x", "y", "z"}

        popped = graph.pop_control_flow_context()
        assert popped == {"z"}
        assert graph.get_control_flow_dependencies() == {"x", "y"}


class TestCaMeLInterpreter:
    """Tests for the CaMeL interpreter."""

    def create_interpreter(
        self,
        tools=None,
        policies=None,
        mode=InterpreterMode.NORMAL,
    ):
        """Helper to create an interpreter with defaults."""
        return CaMeLInterpreter(
            tools=tools or {},
            security_policies=policies or [],
            q_llm=MockQLLM(),
            mode=mode,
        )

    # --- Basic Expression Tests ---

    def test_literal_integer(self):
        """Test integer literal evaluation."""
        interp = self.create_interpreter()
        result, _ = interp.execute("x = 42")

        assert interp._state.variables["x"].raw == 42

    def test_literal_string(self):
        """Test string literal evaluation."""
        interp = self.create_interpreter()
        result, _ = interp.execute('x = "hello"')

        assert interp._state.variables["x"].raw == "hello"

    def test_literal_gets_user_source(self):
        """Test that literals are tagged with User source."""
        interp = self.create_interpreter()
        interp.execute('x = "test"')

        cap = interp._state.variables["x"].capability
        assert any(s.source_type == "User" for s in cap.sources)

    def test_binary_operations(self):
        """Test binary operations."""
        interp = self.create_interpreter()

        interp.execute("x = 10 + 5")
        assert interp._state.variables["x"].raw == 15

        interp.execute("y = 10 - 5")
        assert interp._state.variables["y"].raw == 5

        interp.execute("z = 10 * 5")
        assert interp._state.variables["z"].raw == 50

    def test_comparison_operations(self):
        """Test comparison operations."""
        interp = self.create_interpreter()

        interp.execute("x = 10 > 5")
        assert interp._state.variables["x"].raw is True

        interp.execute("y = 10 < 5")
        assert interp._state.variables["y"].raw is False

        interp.execute("z = 10 == 10")
        assert interp._state.variables["z"].raw is True

    def test_boolean_operations(self):
        """Test boolean operations."""
        interp = self.create_interpreter()

        interp.execute("x = True and False")
        assert interp._state.variables["x"].raw is False

        interp.execute("y = True or False")
        assert interp._state.variables["y"].raw is True

    def test_unary_operations(self):
        """Test unary operations."""
        interp = self.create_interpreter()

        interp.execute("x = -5")
        assert interp._state.variables["x"].raw == -5

        interp.execute("y = not True")
        assert interp._state.variables["y"].raw is False

    # --- Container Tests ---

    def test_list_literal(self):
        """Test list literal."""
        interp = self.create_interpreter()
        interp.execute("x = [1, 2, 3]")

        assert interp._state.variables["x"].raw == [1, 2, 3]

    def test_dict_literal(self):
        """Test dict literal."""
        interp = self.create_interpreter()
        interp.execute('x = {"a": 1, "b": 2}')

        assert interp._state.variables["x"].raw == {"a": 1, "b": 2}

    def test_tuple_literal(self):
        """Test tuple literal."""
        interp = self.create_interpreter()
        interp.execute("x = (1, 2, 3)")

        assert interp._state.variables["x"].raw == (1, 2, 3)

    def test_set_literal(self):
        """Test set literal."""
        interp = self.create_interpreter()
        interp.execute("x = {1, 2, 3}")

        assert interp._state.variables["x"].raw == {1, 2, 3}

    def test_subscript_access(self):
        """Test subscript access."""
        interp = self.create_interpreter()
        interp.execute("""
lst = [10, 20, 30]
x = lst[1]
""")

        assert interp._state.variables["x"].raw == 20

    def test_attribute_access(self):
        """Test attribute access."""
        interp = self.create_interpreter()
        interp.execute('''
s = "hello"
x = s.upper()
''')

        assert interp._state.variables["x"].raw == "HELLO"

    # --- Control Flow Tests ---

    def test_if_statement_true(self):
        """Test if statement with true condition."""
        interp = self.create_interpreter()
        code = """
x = 10
if x > 5:
    y = "big"
else:
    y = "small"
"""
        interp.execute(code)

        assert interp._state.variables["y"].raw == "big"

    def test_if_statement_false(self):
        """Test if statement with false condition."""
        interp = self.create_interpreter()
        code = """
x = 3
if x > 5:
    y = "big"
else:
    y = "small"
"""
        interp.execute(code)

        assert interp._state.variables["y"].raw == "small"

    def test_for_loop(self):
        """Test for loop."""
        interp = self.create_interpreter()
        code = """
result = []
for i in [1, 2, 3]:
    result = result + [i * 2]
"""
        interp.execute(code)

        assert interp._state.variables["result"].raw == [2, 4, 6]

    def test_for_loop_max_iterations(self):
        """Test that for loops respect max iterations."""
        interp = CaMeLInterpreter(
            tools={},
            security_policies=[],
            q_llm=MockQLLM(),
            max_iterations=5,
        )

        code = """
result = 0
for i in range(100):
    result = result + 1
"""
        with pytest.raises(InterpreterError, match="exceeded"):
            interp.execute(code)

    # --- Comprehension Tests ---

    def test_list_comprehension(self):
        """Test list comprehension."""
        interp = self.create_interpreter()
        interp.execute("x = [i * 2 for i in [1, 2, 3]]")

        assert interp._state.variables["x"].raw == [2, 4, 6]

    def test_list_comprehension_with_condition(self):
        """Test list comprehension with condition."""
        interp = self.create_interpreter()
        interp.execute("x = [i for i in [1, 2, 3, 4, 5] if i > 2]")

        assert interp._state.variables["x"].raw == [3, 4, 5]

    def test_dict_comprehension(self):
        """Test dict comprehension."""
        interp = self.create_interpreter()
        interp.execute('x = {k: k * 2 for k in ["a", "b"]}')

        assert interp._state.variables["x"].raw == {"a": "aa", "b": "bb"}

    # --- String Tests ---

    def test_fstring(self):
        """Test f-string evaluation."""
        interp = self.create_interpreter()
        interp.execute('''
name = "world"
x = f"hello {name}"
''')

        assert interp._state.variables["x"].raw == "hello world"

    def test_string_methods(self):
        """Test allowed string methods."""
        interp = self.create_interpreter()
        interp.execute('''
s = "hello world"
x = s.upper()
y = s.split()
''')

        assert interp._state.variables["x"].raw == "HELLO WORLD"
        assert interp._state.variables["y"].raw == ["hello", "world"]

    # --- Builtin Tests ---

    def test_len(self):
        """Test len builtin."""
        interp = self.create_interpreter()
        interp.execute("x = len([1, 2, 3])")

        assert interp._state.variables["x"].raw == 3

    def test_range(self):
        """Test range builtin."""
        interp = self.create_interpreter()
        interp.execute("x = list(range(5))")

        assert interp._state.variables["x"].raw == [0, 1, 2, 3, 4]

    def test_sorted(self):
        """Test sorted builtin."""
        interp = self.create_interpreter()
        interp.execute("x = sorted([3, 1, 2])")

        assert interp._state.variables["x"].raw == [1, 2, 3]

    def test_print(self):
        """Test print builtin (should not raise)."""
        interp = self.create_interpreter()
        result, _ = interp.execute('print("hello")')

        # print returns None
        assert result is None

    # --- Restriction Tests ---

    def test_while_loop_forbidden(self):
        """Test that while loops are forbidden."""
        interp = self.create_interpreter()

        with pytest.raises(InterpreterError, match="While loops"):
            interp.execute("while True: pass")

    def test_import_forbidden(self):
        """Test that imports are forbidden."""
        interp = self.create_interpreter()

        with pytest.raises(InterpreterError, match="Imports"):
            interp.execute("import os")

    def test_eval_forbidden(self):
        """Test that eval is forbidden."""
        interp = self.create_interpreter()

        with pytest.raises(InterpreterError, match="eval/exec"):
            interp.execute('eval("1 + 1")')

    def test_exec_forbidden(self):
        """Test that exec is forbidden."""
        interp = self.create_interpreter()

        with pytest.raises(InterpreterError, match="eval/exec"):
            interp.execute('exec("x = 1")')

    def test_break_forbidden(self):
        """Test that break is forbidden."""
        interp = self.create_interpreter()

        with pytest.raises(InterpreterError, match="break/continue"):
            interp.execute("""
for i in [1, 2, 3]:
    break
""")

    def test_lambda_forbidden(self):
        """Test that lambda is forbidden."""
        interp = self.create_interpreter()

        with pytest.raises(InterpreterError, match="Lambda"):
            interp.execute("f = lambda x: x + 1")

    def test_generator_forbidden(self):
        """Test that generator expressions are forbidden."""
        interp = self.create_interpreter()

        with pytest.raises(InterpreterError, match="Generator"):
            interp.execute("x = (i for i in [1, 2, 3])")

    # --- Tool Tests ---

    def test_tool_call(self):
        """Test calling a tool."""
        def mock_tool(x: int) -> int:
            return x * 2

        interp = self.create_interpreter(tools={"double": mock_tool})
        interp.execute("result = double(21)")

        assert interp._state.variables["result"].raw == 42

    def test_tool_call_recorded_in_trace(self):
        """Test that tool calls are recorded in trace."""
        def mock_tool(x: int) -> int:
            return x * 2

        interp = self.create_interpreter(tools={"double": mock_tool})
        _, trace = interp.execute("result = double(21)")

        assert len(trace) == 1
        assert trace[0][0] == "double"
        assert trace[0][2] == 42

    # --- Dependency Tracking Tests ---

    def test_dependency_propagation(self):
        """Test that dependencies propagate through operations."""
        interp = self.create_interpreter()
        interp.execute("""
a = 10
b = 20
c = a + b
""")

        c_value = interp._state.variables["c"]
        assert len(c_value.dependencies) == 2

    def test_strict_mode_control_flow_deps(self):
        """Test that STRICT mode adds control flow dependencies."""
        interp = self.create_interpreter(mode=InterpreterMode.STRICT)

        code = """
condition = True
if condition:
    result = "yes"
"""
        interp.execute(code)

        # In STRICT mode, result depends on condition through control flow
        result_deps = interp._state.data_flow_graph.get_all_dependencies("result")
        assert "condition" in result_deps


class TestSecurityPolicyEnforcement:
    """Tests for security policy enforcement in interpreter."""

    def test_policy_blocks_violation(self):
        """Test that security policy violations are blocked."""

        class BlockAllPolicy(SecurityPolicy):
            def check(self, tool_name, kwargs, memory_state):
                return PolicyDecision(
                    SecurityPolicyResult.DENIED,
                    reason="Blocked for testing",
                )

        def mock_tool():
            return "result"

        interp = CaMeLInterpreter(
            tools={"blocked_tool": mock_tool},
            security_policies=[BlockAllPolicy()],
            q_llm=MockQLLM(),
        )

        with pytest.raises(SecurityPolicyViolation, match="Blocked for testing"):
            interp.execute("result = blocked_tool()")

    def test_policy_allows_valid_call(self):
        """Test that valid calls are allowed."""

        class AllowAllPolicy(SecurityPolicy):
            def check(self, tool_name, kwargs, memory_state):
                return PolicyDecision(SecurityPolicyResult.ALLOWED)

        def mock_tool():
            return "result"

        interp = CaMeLInterpreter(
            tools={"allowed_tool": mock_tool},
            security_policies=[AllowAllPolicy()],
            q_llm=MockQLLM(),
        )

        interp.execute("result = allowed_tool()")
        assert interp._state.variables["result"].raw == "result"
