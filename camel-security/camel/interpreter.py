"""
CaMeL Interpreter - Custom Python interpreter with capability tracking.

This module provides:
- DataFlowGraph for tracking variable dependencies
- CaMeLInterpreter for executing restricted Python with security enforcement
- AST validation for language restrictions
"""

import ast
import operator
from dataclasses import dataclass, field
from enum import Enum, auto
from typing import Any, Callable, Dict, List, Optional, Set, Tuple, Type, Union

from pydantic import BaseModel

from .config import InterpreterMode
from .errors import InterpreterError, NotEnoughInformationError, SecurityPolicyViolation, ToolExecutionError
from .policies import PolicyDecision, SecurityPolicy, SecurityPolicyResult
from .types import CaMeLValue, Capability, CapabilityAssigner, DataSource, Public


class DataFlowGraph:
    """
    Tracks dependencies between variables in the interpreter.

    This graph enables tracking data provenance through the program.
    """

    def __init__(self):
        self._nodes: Dict[str, CaMeLValue] = {}
        self._edges: Dict[str, Set[str]] = {}  # node -> dependencies
        self._control_flow_stack: List[Set[str]] = []  # For STRICT mode

    def add_variable(self, name: str, value: CaMeLValue) -> None:
        """
        Register a variable with its value.

        Args:
            name: Variable name
            value: CaMeLValue to associate with the name
        """
        self._nodes[name] = value
        if name not in self._edges:
            self._edges[name] = set()

    def add_dependency(self, var_name: str, depends_on: str) -> None:
        """
        Add a dependency edge.

        Args:
            var_name: Variable that depends on another
            depends_on: Variable being depended upon
        """
        if var_name not in self._edges:
            self._edges[var_name] = set()
        self._edges[var_name].add(depends_on)

    def push_control_flow_context(self, condition_vars: Set[str]) -> None:
        """
        Enter a control flow block (if/for) in STRICT mode.

        Args:
            condition_vars: Variables the control flow depends on
        """
        self._control_flow_stack.append(condition_vars)

    def pop_control_flow_context(self) -> Set[str]:
        """
        Exit a control flow block.

        Returns:
            The condition variables for the exited context
        """
        return self._control_flow_stack.pop() if self._control_flow_stack else set()

    def get_control_flow_dependencies(self) -> Set[str]:
        """
        Get all current control flow dependencies (STRICT mode).

        Returns:
            Union of all condition variables in the current stack
        """
        result: Set[str] = set()
        for deps in self._control_flow_stack:
            result |= deps
        return result

    def get_all_dependencies(self, var_name: str) -> Set[str]:
        """
        Get all transitive dependencies of a variable.

        Args:
            var_name: Variable to get dependencies for

        Returns:
            Set of all variable names this depends on (transitively)
        """
        visited: Set[str] = set()
        to_visit = [var_name]

        while to_visit:
            current = to_visit.pop()
            if current in visited:
                continue
            visited.add(current)
            to_visit.extend(self._edges.get(current, set()))

        visited.discard(var_name)  # Don't include self
        return visited

    def get_merged_capability(self, var_name: str) -> Capability:
        """
        Get the merged capability for a variable including all dependencies.

        Args:
            var_name: Variable name

        Returns:
            Merged Capability from all dependencies

        Raises:
            ValueError: If variable is unknown
        """
        if var_name not in self._nodes:
            raise ValueError(f"Unknown variable: {var_name}")

        all_deps = self.get_all_dependencies(var_name)
        result = self._nodes[var_name].capability

        for dep_name in all_deps:
            if dep_name in self._nodes:
                result = result.merge_with(self._nodes[dep_name].capability)

        return result

    def get_value(self, name: str) -> Optional[CaMeLValue]:
        """Get the value for a variable name."""
        return self._nodes.get(name)


@dataclass
class InterpreterState:
    """Current state of the interpreter."""
    variables: Dict[str, CaMeLValue] = field(default_factory=dict)
    data_flow_graph: DataFlowGraph = field(default_factory=DataFlowGraph)
    execution_trace: List[Tuple[str, Dict[str, Any], Any]] = field(default_factory=list)
    mode: InterpreterMode = InterpreterMode.NORMAL
    # Track defined classes (for Pydantic models)
    classes: Dict[str, Type] = field(default_factory=dict)


class CaMeLInterpreter:
    """
    Custom Python interpreter for CaMeL.

    Executes a restricted subset of Python while tracking data flow
    and enforcing security policies.
    """

    # Allowed built-in functions
    ALLOWED_BUILTINS = {
        'abs', 'any', 'all', 'bool', 'dir', 'divmod', 'enumerate',
        'float', 'hash', 'int', 'len', 'list', 'max', 'min', 'print',
        'range', 'repr', 'reversed', 'set', 'sorted', 'str', 'tuple',
        'type', 'zip', 'sum'
    }

    # Allowed string methods
    ALLOWED_STR_METHODS = {
        'capitalize', 'count', 'endswith', 'find', 'format', 'index',
        'isalnum', 'isalpha', 'isdigit', 'islower', 'isspace', 'istitle',
        'isupper', 'join', 'lower', 'lstrip', 'partition', 'removeprefix',
        'removesuffix', 'replace', 'rfind', 'rindex', 'rpartition',
        'rsplit', 'rstrip', 'split', 'splitlines', 'startswith', 'strip',
        'title', 'upper'
    }

    # Allowed dict methods
    ALLOWED_DICT_METHODS = {'get', 'items', 'keys', 'values'}

    # Allowed list methods
    ALLOWED_LIST_METHODS = {'index', 'count'}

    # Binary operators
    BINARY_OPS = {
        ast.Add: operator.add,
        ast.Sub: operator.sub,
        ast.Mult: operator.mul,
        ast.Div: operator.truediv,
        ast.FloorDiv: operator.floordiv,
        ast.Mod: operator.mod,
        ast.Pow: operator.pow,
        ast.LShift: operator.lshift,
        ast.RShift: operator.rshift,
        ast.BitOr: operator.or_,
        ast.BitXor: operator.xor,
        ast.BitAnd: operator.and_,
    }

    # Comparison operators
    COMPARE_OPS = {
        ast.Eq: operator.eq,
        ast.NotEq: operator.ne,
        ast.Lt: operator.lt,
        ast.LtE: operator.le,
        ast.Gt: operator.gt,
        ast.GtE: operator.ge,
        ast.Is: operator.is_,
        ast.IsNot: operator.is_not,
        ast.In: lambda a, b: a in b,
        ast.NotIn: lambda a, b: a not in b,
    }

    # Unary operators
    UNARY_OPS = {
        ast.UAdd: operator.pos,
        ast.USub: operator.neg,
        ast.Not: operator.not_,
        ast.Invert: operator.invert,
    }

    def __init__(
        self,
        tools: Dict[str, Callable],
        security_policies: List[SecurityPolicy],
        q_llm: Any,  # QuarantinedLLM
        mode: InterpreterMode = InterpreterMode.NORMAL,
        max_iterations: int = 100,
    ):
        """
        Initialize the CaMeL interpreter.

        Args:
            tools: Dictionary mapping tool names to functions
            security_policies: List of security policies to enforce
            q_llm: QuarantinedLLM instance for query_ai_assistant
            mode: Interpreter mode (NORMAL or STRICT)
            max_iterations: Maximum loop iterations
        """
        self._tools = tools
        self._policies = security_policies
        self._q_llm = q_llm
        self._mode = mode
        self._max_iterations = max_iterations
        self._state: Optional[InterpreterState] = None

    def execute(self, code: str) -> Tuple[Any, List[Tuple]]:
        """
        Execute the given Python code.

        Args:
            code: Python source code to execute

        Returns:
            Tuple of (final result, execution trace)

        Raises:
            SecurityPolicyViolation: If a security policy is violated
            InterpreterError: If code violates language restrictions
        """
        # Initialize state
        self._state = InterpreterState(mode=self._mode)

        # Add BaseModel to classes for Pydantic model definitions
        self._state.classes['BaseModel'] = BaseModel

        # Parse AST
        try:
            tree = ast.parse(code)
        except SyntaxError as e:
            raise InterpreterError(f"Syntax error: {e}", line=e.lineno)

        # Validate AST against restrictions
        self._validate_ast(tree)

        # Execute
        result = None
        for node in tree.body:
            result = self._execute_node(node)

        # Unwrap final result if it's a CaMeLValue
        if isinstance(result, CaMeLValue):
            return result.raw, self._state.execution_trace

        return result, self._state.execution_trace

    def _validate_ast(self, tree: ast.AST) -> None:
        """
        Validate AST against language restrictions.

        Args:
            tree: AST to validate

        Raises:
            InterpreterError: If restrictions are violated
        """
        for node in ast.walk(tree):
            # No while loops
            if isinstance(node, ast.While):
                raise InterpreterError("While loops are not allowed", line=getattr(node, 'lineno', None))

            # No generators (comprehensions are OK)
            if isinstance(node, ast.GeneratorExp):
                raise InterpreterError("Generator expressions are not allowed", line=getattr(node, 'lineno', None))

            # No imports
            if isinstance(node, (ast.Import, ast.ImportFrom)):
                raise InterpreterError("Imports are not allowed", line=getattr(node, 'lineno', None))

            # No eval/exec
            if isinstance(node, ast.Call):
                if isinstance(node.func, ast.Name):
                    if node.func.id in ('eval', 'exec'):
                        raise InterpreterError("eval/exec are not allowed", line=getattr(node, 'lineno', None))

            # No break/continue
            if isinstance(node, (ast.Break, ast.Continue)):
                raise InterpreterError("break/continue are not allowed", line=getattr(node, 'lineno', None))

            # No function definitions (except class methods in Pydantic models)
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                # Allow if inside a class definition
                if not self._is_inside_class(tree, node):
                    raise InterpreterError("Function definitions are not allowed", line=getattr(node, 'lineno', None))

            # No lambda
            if isinstance(node, ast.Lambda):
                raise InterpreterError("Lambda expressions are not allowed", line=getattr(node, 'lineno', None))

    def _is_inside_class(self, tree: ast.AST, target: ast.AST) -> bool:
        """Check if a node is inside a class definition."""
        for node in ast.walk(tree):
            if isinstance(node, ast.ClassDef):
                for child in ast.walk(node):
                    if child is target:
                        return True
        return False

    def _execute_node(self, node: ast.AST) -> Optional[CaMeLValue]:
        """
        Execute a single AST node.

        Args:
            node: AST node to execute

        Returns:
            Result of execution (if any)
        """
        if isinstance(node, ast.Assign):
            return self._execute_assign(node)
        elif isinstance(node, ast.AugAssign):
            return self._execute_aug_assign(node)
        elif isinstance(node, ast.AnnAssign):
            return self._execute_ann_assign(node)
        elif isinstance(node, ast.Expr):
            return self._evaluate_expr(node.value)
        elif isinstance(node, ast.If):
            return self._execute_if(node)
        elif isinstance(node, ast.For):
            return self._execute_for(node)
        elif isinstance(node, ast.Raise):
            return self._execute_raise(node)
        elif isinstance(node, ast.Pass):
            return None
        elif isinstance(node, ast.ClassDef):
            return self._execute_class_def(node)
        else:
            raise InterpreterError(f"Unsupported statement: {type(node).__name__}", line=getattr(node, 'lineno', None))

    def _execute_assign(self, node: ast.Assign) -> None:
        """Execute an assignment statement."""
        value = self._evaluate_expr(node.value)

        for target in node.targets:
            self._assign_target(target, value)

    def _execute_aug_assign(self, node: ast.AugAssign) -> None:
        """Execute an augmented assignment (e.g., x += 1)."""
        if not isinstance(node.target, ast.Name):
            raise InterpreterError("Augmented assignment only supported for simple names")

        var_name = node.target.id
        if var_name not in self._state.variables:
            raise InterpreterError(f"Undefined variable: {var_name}")

        current = self._state.variables[var_name]
        operand = self._evaluate_expr(node.value)

        op_func = self.BINARY_OPS.get(type(node.op))
        if op_func is None:
            raise InterpreterError(f"Unsupported operator: {type(node.op).__name__}")

        result_raw = op_func(current.raw, operand.raw)
        result = CapabilityAssigner.from_operation(result_raw, {current, operand})

        self._state.variables[var_name] = result
        self._state.data_flow_graph.add_variable(var_name, result)

    def _execute_ann_assign(self, node: ast.AnnAssign) -> None:
        """Execute an annotated assignment (e.g., x: int = 5)."""
        if node.value is None:
            return  # Just an annotation, no assignment

        value = self._evaluate_expr(node.value)
        self._assign_target(node.target, value)

    def _assign_target(self, target: ast.AST, value: CaMeLValue) -> None:
        """Assign a value to a target (handles simple names and subscripts)."""
        if isinstance(target, ast.Name):
            var_name = target.id

            # Register in data flow graph
            self._state.data_flow_graph.add_variable(var_name, value)

            # Add control flow dependencies in STRICT mode
            if self._mode == InterpreterMode.STRICT:
                for dep in self._state.data_flow_graph.get_control_flow_dependencies():
                    self._state.data_flow_graph.add_dependency(var_name, dep)

            # Store in variables
            self._state.variables[var_name] = value

        elif isinstance(target, ast.Subscript):
            # Handle subscript assignment: a[0] = x
            container = self._evaluate_expr(target.value)
            index = self._evaluate_expr(target.slice)

            # Perform the assignment on the raw value
            container.raw[index.raw] = value.raw

        elif isinstance(target, ast.Tuple) or isinstance(target, ast.List):
            # Handle tuple/list unpacking: a, b = (1, 2)
            if not isinstance(value.raw, (list, tuple)):
                raise InterpreterError("Cannot unpack non-sequence")

            if len(target.elts) != len(value.raw):
                raise InterpreterError(f"Cannot unpack {len(value.raw)} values into {len(target.elts)} targets")

            for i, elt in enumerate(target.elts):
                item_value = CaMeLValue(
                    raw=value.raw[i],
                    capability=Capability(),
                    dependencies={value},
                )
                self._assign_target(elt, item_value)

        else:
            raise InterpreterError(f"Unsupported assignment target: {type(target).__name__}")

    def _evaluate_expr(self, node: ast.expr) -> CaMeLValue:
        """
        Evaluate an expression and return a CaMeLValue.

        Args:
            node: Expression AST node

        Returns:
            CaMeLValue with the result and capability metadata
        """
        if isinstance(node, ast.Constant):
            # Literal value from P-LLM code (trusted)
            return CapabilityAssigner.from_user_literal(node.value)

        elif isinstance(node, ast.Name):
            # Variable reference
            if node.id in self._state.classes:
                # Return the class itself (for Pydantic model instantiation)
                return CaMeLValue(
                    raw=self._state.classes[node.id],
                    capability=Capability(sources={DataSource(source_type="User")}),
                )
            if node.id not in self._state.variables:
                raise InterpreterError(f"Undefined variable: {node.id}")
            return self._state.variables[node.id]

        elif isinstance(node, ast.BinOp):
            return self._evaluate_binop(node)

        elif isinstance(node, ast.Compare):
            return self._evaluate_compare(node)

        elif isinstance(node, ast.BoolOp):
            return self._evaluate_boolop(node)

        elif isinstance(node, ast.UnaryOp):
            return self._evaluate_unaryop(node)

        elif isinstance(node, ast.Call):
            return self._evaluate_call(node)

        elif isinstance(node, ast.Subscript):
            return self._evaluate_subscript(node)

        elif isinstance(node, ast.Attribute):
            return self._evaluate_attribute(node)

        elif isinstance(node, ast.List):
            return self._evaluate_list(node)

        elif isinstance(node, ast.Dict):
            return self._evaluate_dict(node)

        elif isinstance(node, ast.Tuple):
            return self._evaluate_tuple(node)

        elif isinstance(node, ast.Set):
            return self._evaluate_set(node)

        elif isinstance(node, ast.IfExp):
            return self._evaluate_ifexp(node)

        elif isinstance(node, ast.ListComp):
            return self._evaluate_list_comp(node)

        elif isinstance(node, ast.DictComp):
            return self._evaluate_dict_comp(node)

        elif isinstance(node, ast.SetComp):
            return self._evaluate_set_comp(node)

        elif isinstance(node, ast.JoinedStr):
            return self._evaluate_joined_str(node)

        elif isinstance(node, ast.FormattedValue):
            return self._evaluate_formatted_value(node)

        elif isinstance(node, ast.Starred):
            # Handle starred expressions in calls
            inner = self._evaluate_expr(node.value)
            return inner

        else:
            raise InterpreterError(f"Unsupported expression: {type(node).__name__}")

    def _evaluate_binop(self, node: ast.BinOp) -> CaMeLValue:
        """Evaluate a binary operation."""
        left = self._evaluate_expr(node.left)
        right = self._evaluate_expr(node.right)

        op_func = self.BINARY_OPS.get(type(node.op))
        if op_func is None:
            raise InterpreterError(f"Unsupported binary operator: {type(node.op).__name__}")

        result_raw = op_func(left.raw, right.raw)
        return CapabilityAssigner.from_operation(result_raw, {left, right})

    def _evaluate_compare(self, node: ast.Compare) -> CaMeLValue:
        """Evaluate a comparison expression."""
        left = self._evaluate_expr(node.left)
        dependencies = {left}
        result = True

        current_left = left.raw
        for op, comparator_node in zip(node.ops, node.comparators):
            comparator = self._evaluate_expr(comparator_node)
            dependencies.add(comparator)

            op_func = self.COMPARE_OPS.get(type(op))
            if op_func is None:
                raise InterpreterError(f"Unsupported comparison operator: {type(op).__name__}")

            if not op_func(current_left, comparator.raw):
                result = False
                break

            current_left = comparator.raw

        return CapabilityAssigner.from_operation(result, dependencies)

    def _evaluate_boolop(self, node: ast.BoolOp) -> CaMeLValue:
        """Evaluate a boolean operation (and/or)."""
        dependencies: Set[CaMeLValue] = set()

        if isinstance(node.op, ast.And):
            result = True
            for value_node in node.values:
                value = self._evaluate_expr(value_node)
                dependencies.add(value)
                if not value.raw:
                    result = value.raw
                    break
                result = value.raw
        else:  # ast.Or
            result = False
            for value_node in node.values:
                value = self._evaluate_expr(value_node)
                dependencies.add(value)
                if value.raw:
                    result = value.raw
                    break
                result = value.raw

        return CapabilityAssigner.from_operation(result, dependencies)

    def _evaluate_unaryop(self, node: ast.UnaryOp) -> CaMeLValue:
        """Evaluate a unary operation."""
        operand = self._evaluate_expr(node.operand)

        op_func = self.UNARY_OPS.get(type(node.op))
        if op_func is None:
            raise InterpreterError(f"Unsupported unary operator: {type(node.op).__name__}")

        result_raw = op_func(operand.raw)
        return CapabilityAssigner.from_operation(result_raw, {operand})

    def _evaluate_call(self, node: ast.Call) -> CaMeLValue:
        """Evaluate a function call."""
        # Get function name/object
        if isinstance(node.func, ast.Name):
            func_name = node.func.id
            return self._call_by_name(func_name, node.args, node.keywords)
        elif isinstance(node.func, ast.Attribute):
            return self._evaluate_method_call(node)
        else:
            raise InterpreterError(f"Unsupported call target: {type(node.func).__name__}")

    def _call_by_name(
        self,
        func_name: str,
        arg_nodes: List[ast.expr],
        keyword_nodes: List[ast.keyword],
    ) -> CaMeLValue:
        """Call a function by name."""
        # Evaluate arguments
        args = [self._evaluate_expr(arg) for arg in arg_nodes]
        kwargs = {kw.arg: self._evaluate_expr(kw.value) for kw in keyword_nodes if kw.arg}

        # Handle special functions
        if func_name == 'query_ai_assistant':
            return self._call_qllm(args, kwargs)

        # Check if it's a tool
        if func_name in self._tools:
            return self._call_tool(func_name, args, kwargs)

        # Check if it's an allowed builtin
        if func_name in self.ALLOWED_BUILTINS:
            return self._call_builtin(func_name, args, kwargs)

        # Check if it's a class (for Pydantic models)
        if func_name in self._state.classes:
            return self._construct_class(self._state.classes[func_name], args, kwargs)

        # Check if it's a variable containing a class
        if func_name in self._state.variables:
            var = self._state.variables[func_name]
            if isinstance(var.raw, type):
                return self._construct_class(var.raw, args, kwargs)

        raise InterpreterError(f"Unknown function: {func_name}")

    def _evaluate_method_call(self, node: ast.Call) -> CaMeLValue:
        """Evaluate a method call (e.g., obj.method())."""
        attr_node = node.func
        obj = self._evaluate_expr(attr_node.value)
        method_name = attr_node.attr

        # Evaluate arguments
        args = [self._evaluate_expr(arg) for arg in node.args]
        kwargs = {kw.arg: self._evaluate_expr(kw.value) for kw in node.keywords if kw.arg}

        raw_obj = obj.raw
        dependencies = {obj} | set(args) | set(kwargs.values())

        # Check if method is allowed based on object type
        if isinstance(raw_obj, str):
            if method_name not in self.ALLOWED_STR_METHODS:
                raise InterpreterError(f"String method not allowed: {method_name}")
        elif isinstance(raw_obj, dict):
            if method_name not in self.ALLOWED_DICT_METHODS:
                raise InterpreterError(f"Dict method not allowed: {method_name}")
        elif isinstance(raw_obj, list):
            if method_name not in self.ALLOWED_LIST_METHODS:
                raise InterpreterError(f"List method not allowed: {method_name}")

        # Get the method and call it
        method = getattr(raw_obj, method_name)
        raw_args = [a.raw for a in args]
        raw_kwargs = {k: v.raw for k, v in kwargs.items()}

        result_raw = method(*raw_args, **raw_kwargs)
        return CapabilityAssigner.from_operation(result_raw, dependencies)

    def _call_tool(
        self,
        tool_name: str,
        args: List[CaMeLValue],
        kwargs: Dict[str, CaMeLValue],
    ) -> CaMeLValue:
        """Call a tool with security policy checks."""
        import inspect

        tool_func = self._tools[tool_name]

        # Convert args to kwargs based on function signature
        sig = inspect.signature(tool_func)
        param_names = list(sig.parameters.keys())

        full_kwargs = dict(kwargs)
        for i, arg in enumerate(args):
            if i < len(param_names):
                full_kwargs[param_names[i]] = arg

        # Check security policies
        for policy in self._policies:
            decision = policy.check(tool_name, full_kwargs, self._state)

            if decision.result == SecurityPolicyResult.DENIED:
                raise SecurityPolicyViolation(
                    f"Security policy denied {tool_name}: {decision.reason}",
                    policy=policy,
                    tool_name=tool_name,
                )
            elif decision.result == SecurityPolicyResult.REQUIRES_CONFIRMATION:
                raise SecurityPolicyViolation(
                    f"Tool {tool_name} requires user confirmation: {decision.reason}",
                    policy=policy,
                    tool_name=tool_name,
                )

        # Execute tool with raw values
        raw_args = [arg.raw for arg in args]
        raw_kwargs = {k: v.raw for k, v in kwargs.items()}

        try:
            result = tool_func(*raw_args, **raw_kwargs)
        except Exception as e:
            raise ToolExecutionError(str(e), tool_name=tool_name)

        # Wrap result with capability
        result_value = self._annotate_tool_result(tool_name, result, full_kwargs)

        # Record in trace
        self._state.execution_trace.append((
            tool_name,
            raw_kwargs,
            result,
        ))

        return result_value

    def _call_qllm(
        self,
        args: List[CaMeLValue],
        kwargs: Dict[str, CaMeLValue],
    ) -> CaMeLValue:
        """Call the Quarantined LLM."""
        if len(args) < 2:
            raise InterpreterError("query_ai_assistant requires query and output_schema")

        query_value = args[0]
        schema_value = args[1]

        # Get dependencies from query
        query_deps = {query_value}
        query_deps |= query_value.get_full_dependency_graph()

        # Call Q-LLM
        try:
            result = self._q_llm.parse_data(
                query=query_value.raw,
                output_schema=schema_value.raw,
            )
        except NotEnoughInformationError:
            raise  # Re-raise to be handled by caller

        # Wrap result with inherited dependencies
        return CapabilityAssigner.from_qllm_output(result, query_deps)

    def _call_builtin(
        self,
        func_name: str,
        args: List[CaMeLValue],
        kwargs: Dict[str, CaMeLValue],
    ) -> CaMeLValue:
        """Call a built-in function."""
        builtin_func = getattr(__builtins__ if isinstance(__builtins__, dict) else __builtins__.__dict__, func_name, None)

        if builtin_func is None:
            # Try getting from builtins module
            import builtins
            builtin_func = getattr(builtins, func_name, None)

        if builtin_func is None:
            raise InterpreterError(f"Unknown builtin: {func_name}")

        dependencies = set(args) | set(kwargs.values())
        raw_args = [a.raw for a in args]
        raw_kwargs = {k: v.raw for k, v in kwargs.items()}

        result_raw = builtin_func(*raw_args, **raw_kwargs)

        # Special case for print - return None but still track
        if func_name == 'print':
            return CaMeLValue(raw=None, capability=Capability())

        return CapabilityAssigner.from_operation(result_raw, dependencies)

    def _construct_class(
        self,
        cls: Type,
        args: List[CaMeLValue],
        kwargs: Dict[str, CaMeLValue],
    ) -> CaMeLValue:
        """Construct a class instance (for Pydantic models)."""
        dependencies = set(args) | set(kwargs.values())
        raw_args = [a.raw for a in args]
        raw_kwargs = {k: v.raw for k, v in kwargs.items()}

        result = cls(*raw_args, **raw_kwargs)
        return CapabilityAssigner.from_operation(result, dependencies)

    def _evaluate_subscript(self, node: ast.Subscript) -> CaMeLValue:
        """Evaluate a subscript expression (e.g., a[0])."""
        container = self._evaluate_expr(node.value)
        index = self._evaluate_expr(node.slice)

        result_raw = container.raw[index.raw]
        return CapabilityAssigner.from_operation(result_raw, {container, index})

    def _evaluate_attribute(self, node: ast.Attribute) -> CaMeLValue:
        """Evaluate an attribute access (e.g., obj.attr)."""
        obj = self._evaluate_expr(node.value)
        attr_name = node.attr

        result_raw = getattr(obj.raw, attr_name)
        return CapabilityAssigner.from_operation(result_raw, {obj})

    def _evaluate_list(self, node: ast.List) -> CaMeLValue:
        """Evaluate a list literal."""
        elements = [self._evaluate_expr(elt) for elt in node.elts]
        raw_list = [e.raw for e in elements]
        return CapabilityAssigner.from_operation(raw_list, set(elements))

    def _evaluate_dict(self, node: ast.Dict) -> CaMeLValue:
        """Evaluate a dict literal."""
        keys = []
        values = []
        dependencies: Set[CaMeLValue] = set()

        for k, v in zip(node.keys, node.values):
            if k is not None:
                key = self._evaluate_expr(k)
                keys.append(key.raw)
                dependencies.add(key)
            else:
                # Dict unpacking: {**other_dict}
                raise InterpreterError("Dict unpacking not supported")

            value = self._evaluate_expr(v)
            values.append(value.raw)
            dependencies.add(value)

        raw_dict = dict(zip(keys, values))
        return CapabilityAssigner.from_operation(raw_dict, dependencies)

    def _evaluate_tuple(self, node: ast.Tuple) -> CaMeLValue:
        """Evaluate a tuple literal."""
        elements = [self._evaluate_expr(elt) for elt in node.elts]
        raw_tuple = tuple(e.raw for e in elements)
        return CapabilityAssigner.from_operation(raw_tuple, set(elements))

    def _evaluate_set(self, node: ast.Set) -> CaMeLValue:
        """Evaluate a set literal."""
        elements = [self._evaluate_expr(elt) for elt in node.elts]
        raw_set = set(e.raw for e in elements)
        return CapabilityAssigner.from_operation(raw_set, set(elements))

    def _evaluate_ifexp(self, node: ast.IfExp) -> CaMeLValue:
        """Evaluate a ternary expression (x if cond else y)."""
        test = self._evaluate_expr(node.test)

        if test.raw:
            result = self._evaluate_expr(node.body)
        else:
            result = self._evaluate_expr(node.orelse)

        return CapabilityAssigner.from_operation(result.raw, {test, result})

    def _evaluate_list_comp(self, node: ast.ListComp) -> CaMeLValue:
        """Evaluate a list comprehension."""
        return self._evaluate_comprehension(node, list)

    def _evaluate_dict_comp(self, node: ast.DictComp) -> CaMeLValue:
        """Evaluate a dict comprehension."""
        # Dict comprehensions need special handling
        dependencies: Set[CaMeLValue] = set()
        result = {}

        # Get the generator
        gen = node.generators[0]
        iter_value = self._evaluate_expr(gen.iter)
        dependencies.add(iter_value)

        for item in iter_value.raw:
            # Bind loop variable
            item_value = CaMeLValue(raw=item, dependencies={iter_value})
            self._state.variables[gen.target.id] = item_value
            dependencies.add(item_value)

            # Check conditions
            if all(self._evaluate_expr(if_clause).raw for if_clause in gen.ifs):
                key = self._evaluate_expr(node.key)
                value = self._evaluate_expr(node.value)
                dependencies.add(key)
                dependencies.add(value)
                result[key.raw] = value.raw

        return CapabilityAssigner.from_operation(result, dependencies)

    def _evaluate_set_comp(self, node: ast.SetComp) -> CaMeLValue:
        """Evaluate a set comprehension."""
        return self._evaluate_comprehension(node, set)

    def _evaluate_comprehension(self, node: Union[ast.ListComp, ast.SetComp], container_type: Type) -> CaMeLValue:
        """Evaluate a list or set comprehension."""
        dependencies: Set[CaMeLValue] = set()
        result = []

        # Simple case: single generator
        gen = node.generators[0]
        iter_value = self._evaluate_expr(gen.iter)
        dependencies.add(iter_value)

        iteration_count = 0
        for item in iter_value.raw:
            iteration_count += 1
            if iteration_count > self._max_iterations:
                raise InterpreterError(f"Comprehension exceeded {self._max_iterations} iterations")

            # Bind loop variable
            if isinstance(gen.target, ast.Name):
                item_value = CaMeLValue(raw=item, dependencies={iter_value})
                self._state.variables[gen.target.id] = item_value
                dependencies.add(item_value)
            else:
                raise InterpreterError("Complex comprehension targets not supported")

            # Check conditions
            conditions_met = True
            for if_clause in gen.ifs:
                cond = self._evaluate_expr(if_clause)
                dependencies.add(cond)
                if not cond.raw:
                    conditions_met = False
                    break

            if conditions_met:
                element = self._evaluate_expr(node.elt)
                dependencies.add(element)
                result.append(element.raw)

        return CapabilityAssigner.from_operation(container_type(result), dependencies)

    def _evaluate_joined_str(self, node: ast.JoinedStr) -> CaMeLValue:
        """Evaluate an f-string."""
        parts = []
        dependencies: Set[CaMeLValue] = set()

        for value in node.values:
            if isinstance(value, ast.Constant):
                parts.append(str(value.value))
            elif isinstance(value, ast.FormattedValue):
                formatted = self._evaluate_expr(value.value)
                dependencies.add(formatted)

                # Apply conversion if specified
                result = formatted.raw
                if value.conversion == ord('s'):
                    result = str(result)
                elif value.conversion == ord('r'):
                    result = repr(result)
                elif value.conversion == ord('a'):
                    result = ascii(result)

                # Apply format spec if specified
                if value.format_spec:
                    format_spec_value = self._evaluate_expr(value.format_spec)
                    dependencies.add(format_spec_value)
                    result = format(result, format_spec_value.raw)

                parts.append(str(result))
            else:
                part = self._evaluate_expr(value)
                dependencies.add(part)
                parts.append(str(part.raw))

        result = "".join(parts)
        return CapabilityAssigner.from_operation(result, dependencies)

    def _evaluate_formatted_value(self, node: ast.FormattedValue) -> CaMeLValue:
        """Evaluate a formatted value (part of f-string)."""
        return self._evaluate_expr(node.value)

    def _execute_if(self, node: ast.If) -> Optional[CaMeLValue]:
        """Execute an if statement."""
        test_value = self._evaluate_expr(node.test)

        if self._mode == InterpreterMode.STRICT:
            test_deps = self._get_expr_dependencies(node.test)
            self._state.data_flow_graph.push_control_flow_context(test_deps)

        try:
            result = None
            if test_value.raw:
                for stmt in node.body:
                    result = self._execute_node(stmt)
            else:
                for stmt in node.orelse:
                    result = self._execute_node(stmt)
            return result
        finally:
            if self._mode == InterpreterMode.STRICT:
                self._state.data_flow_graph.pop_control_flow_context()

    def _execute_for(self, node: ast.For) -> None:
        """Execute a for loop."""
        iterable_value = self._evaluate_expr(node.iter)

        if self._mode == InterpreterMode.STRICT:
            iter_deps = self._get_expr_dependencies(node.iter)
            self._state.data_flow_graph.push_control_flow_context(iter_deps)

        try:
            if not isinstance(node.target, ast.Name):
                raise InterpreterError("Only simple loop variables supported")

            var_name = node.target.id
            iteration_count = 0

            for item in iterable_value.raw:
                iteration_count += 1
                if iteration_count > self._max_iterations:
                    raise InterpreterError(f"Loop exceeded {self._max_iterations} iterations")

                # Create CaMeLValue for loop variable with dependency on iterable
                item_value = CaMeLValue(
                    raw=item,
                    capability=Capability(),
                    dependencies={iterable_value},
                )

                self._state.variables[var_name] = item_value
                self._state.data_flow_graph.add_variable(var_name, item_value)

                for stmt in node.body:
                    self._execute_node(stmt)
        finally:
            if self._mode == InterpreterMode.STRICT:
                self._state.data_flow_graph.pop_control_flow_context()

    def _execute_raise(self, node: ast.Raise) -> None:
        """Execute a raise statement."""
        if node.exc is None:
            raise InterpreterError("Bare raise not supported")

        exc_value = self._evaluate_expr(node.exc)

        if isinstance(exc_value.raw, Exception):
            raise exc_value.raw
        elif isinstance(exc_value.raw, type) and issubclass(exc_value.raw, Exception):
            raise exc_value.raw()
        else:
            raise InterpreterError(f"Cannot raise non-exception: {type(exc_value.raw)}")

    def _execute_class_def(self, node: ast.ClassDef) -> None:
        """Execute a class definition (for Pydantic models)."""
        class_name = node.name

        # Get base classes
        bases = []
        for base in node.bases:
            if isinstance(base, ast.Name):
                if base.id in self._state.classes:
                    bases.append(self._state.classes[base.id])
                elif base.id == 'BaseModel':
                    bases.append(BaseModel)
                else:
                    raise InterpreterError(f"Unknown base class: {base.id}")
            else:
                raise InterpreterError("Complex base class expressions not supported")

        # Build class body
        namespace: Dict[str, Any] = {}
        annotations: Dict[str, Any] = {}

        for stmt in node.body:
            if isinstance(stmt, ast.AnnAssign):
                # Annotated assignment: field: type = default
                if isinstance(stmt.target, ast.Name):
                    field_name = stmt.target.id
                    # Get type annotation
                    if isinstance(stmt.annotation, ast.Name):
                        type_name = stmt.annotation.id
                        if type_name in self._state.classes:
                            annotations[field_name] = self._state.classes[type_name]
                        else:
                            # Try built-in types
                            annotations[field_name] = eval(type_name)
                    elif isinstance(stmt.annotation, ast.Constant):
                        annotations[field_name] = stmt.annotation.value
                    else:
                        annotations[field_name] = Any

                    if stmt.value is not None:
                        default_value = self._evaluate_expr(stmt.value)
                        namespace[field_name] = default_value.raw

            elif isinstance(stmt, ast.Assign):
                for target in stmt.targets:
                    if isinstance(target, ast.Name):
                        value = self._evaluate_expr(stmt.value)
                        namespace[target.id] = value.raw

            elif isinstance(stmt, ast.Pass):
                pass

            elif isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Constant):
                # Docstring
                if '__doc__' not in namespace:
                    namespace['__doc__'] = stmt.value.value

        namespace['__annotations__'] = annotations

        # Create the class
        if not bases:
            bases = [object]

        new_class = type(class_name, tuple(bases), namespace)

        # Store in classes dict
        self._state.classes[class_name] = new_class

        # Also create a CaMeLValue for the class
        self._state.variables[class_name] = CaMeLValue(
            raw=new_class,
            capability=Capability(sources={DataSource(source_type="User")}),
        )

    def _annotate_tool_result(
        self,
        tool_name: str,
        result: Any,
        args: Dict[str, CaMeLValue],
    ) -> CaMeLValue:
        """
        Annotate a tool result with appropriate capabilities.

        This method determines the readers for tool outputs based on
        the tool type and result content.
        """
        # Check for capability annotator in tool registry
        from .tools import tool_registry

        tool_def = tool_registry.get(tool_name)
        if tool_def and tool_def.capability_annotator:
            capability = tool_def.capability_annotator(result, args)
            return CaMeLValue(raw=result, capability=capability)

        # Default capability annotation based on result type
        readers = Public

        # Try to extract readers from common patterns
        if hasattr(result, 'shared_with'):
            readers = set(result.shared_with.keys())
            if hasattr(result, 'owner'):
                readers.add(result.owner)

        elif hasattr(result, 'recipients'):
            readers = set(result.recipients)
            if hasattr(result, 'sender'):
                readers.add(result.sender)
            if hasattr(result, 'cc'):
                readers.update(result.cc)
            if hasattr(result, 'bcc'):
                readers.update(result.bcc)

        elif hasattr(result, 'participants'):
            readers = set(result.participants)

        return CapabilityAssigner.from_tool_output(
            result,
            tool_name,
            readers=readers if isinstance(readers, set) and readers else Public,
        )

    def _get_expr_dependencies(self, node: ast.expr) -> Set[str]:
        """Get variable names that an expression depends on."""
        deps: Set[str] = set()

        for child in ast.walk(node):
            if isinstance(child, ast.Name):
                deps.add(child.id)

        return deps
