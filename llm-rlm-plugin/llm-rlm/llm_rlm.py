import json
import pathlib
from typing import Optional

import eryx
import llm


class RLMToolbox(llm.Toolbox):
    """
    Recursive Language Model toolbox.

    Provides a sandboxed Python REPL environment (via pyeryx) for exploring
    and analyzing large contexts. The context is stored as a Python variable
    in the sandbox and is never exposed directly to the model's context window.

    Within the sandbox, `llm_query` and `llm_batch` callbacks are available
    to spawn sub-LLM calls for recursive reasoning.
    """

    def __init__(
        self,
        context: Optional[str] = None,
        context_file: Optional[str] = None,
        sub_model: Optional[str] = None,
        max_output_chars: int = 8192,
        execution_timeout_ms: int = 120_000,
    ):
        if context_file is not None and context is None:
            context = pathlib.Path(context_file).read_text()
        self._context = context
        self._sub_model_id = sub_model
        self._max_output_chars = max_output_chars
        self._execution_timeout_ms = execution_timeout_ms
        self._session: Optional[eryx.Session] = None
        self._sub_model: Optional[llm.Model] = None
        self._total_input_tokens = 0
        self._total_output_tokens = 0
        self._sub_llm_call_count = 0

    def _get_sub_model(self) -> llm.Model:
        if self._sub_model is None:
            model_id = self._sub_model_id or "gpt-5.2"
            self._sub_model = llm.get_model(model_id)
        return self._sub_model

    def _track_usage(self, response):
        """Accumulate token usage from a sub-LLM response."""
        self._sub_llm_call_count += 1
        usage = response.usage()
        if usage.input is not None:
            self._total_input_tokens += usage.input
        if usage.output is not None:
            self._total_output_tokens += usage.output

    def _llm_query(self, prompt: str) -> str:
        """Host-side callback: query a sub-LLM with a prompt."""
        model = self._get_sub_model()
        response = model.prompt(prompt)
        text = response.text()
        self._track_usage(response)
        return text

    def _llm_batch(self, prompts: list) -> list:
        """Host-side callback: query a sub-LLM with multiple prompts."""
        model = self._get_sub_model()
        responses = []
        for prompt in prompts:
            response = model.prompt(prompt)
            text = response.text()
            self._track_usage(response)
            responses.append(text)
        return responses

    def _ensure_session(self) -> eryx.Session:
        if self._session is None:
            self._session = eryx.Session(
                execution_timeout_ms=self._execution_timeout_ms,
                callbacks=[
                    {
                        "name": "llm_query",
                        "fn": self._llm_query,
                        "description": (
                            "Query a sub-LLM. Call with top-level await: "
                            "result = await llm_query(prompt='...') "
                            "Returns a plain string. Do NOT use asyncio.run()."
                        ),
                    },
                    {
                        "name": "llm_batch",
                        "fn": self._llm_batch,
                        "description": (
                            "Query a sub-LLM with multiple prompts. Call with top-level await: "
                            "results = await llm_batch(prompts=['...', '...']) "
                            "Returns a list of strings. Do NOT use asyncio.run()."
                        ),
                    },
                ],
            )
            if self._context is not None:
                self._load_context()
        return self._session

    def _safe_execute(self, code: str):
        """Execute code in the session, working around pyeryx triple-quote wrapping."""
        # pyeryx wraps code in triple double quotes; ensure code doesn't
        # end with a double quote character to avoid syntax errors.
        if not code.endswith("\n"):
            code = code + "\n"
        return self._session.execute(code)

    def _load_context(self):
        """Load context into the sandbox as the `context` variable."""
        # Use JSON serialization to safely pass arbitrary string content
        context_json = json.dumps(self._context)
        self._safe_execute(
            f"import json as _json\n"
            f"context = _json.loads({context_json!r})\n"
            f"del _json"
        )

    def _read_variable(self, variable_name: str) -> str:
        """Read a variable's value from the sandbox session."""
        self._ensure_session()
        safe_name = variable_name.strip().strip("'\"")
        try:
            result = self._safe_execute(
                f"import json as _json\n"
                f"print(_json.dumps({{'__rlm_var__': str({safe_name})}}))\n"
                f"del _json"
            )
            parsed = json.loads(result.stdout)
            return parsed["__rlm_var__"]
        except Exception:
            return None

    def _usage_summary(self) -> str:
        """Return a token usage summary string."""
        return (
            f"Sub-LLM token usage: "
            f"{self._total_input_tokens:,} input, "
            f"{self._total_output_tokens:,} output "
            f"({self._sub_llm_call_count} calls)"
        )

    def execute_python(self, code: str) -> str:
        """Execute Python code in the RLM sandbox environment.

        The sandbox maintains state across calls. If context was provided,
        it is available as the `context` variable (a string).

        Sub-LLM calls: use top-level `await` directly (do NOT use asyncio.run):
          result = await llm_query(prompt="Summarize: " + chunk)
          print(result)  # result is a plain string
          results = await llm_batch(prompts=["Classify: " + c for c in chunks])
          print(results)  # results is a list of strings

        IMPORTANT: Use print() to see output. Bare expressions produce no output.
        Example: print(context[:500]) to peek at the first 500 chars of context.
        """
        self._ensure_session()
        try:
            result = self._safe_execute(code)
            output = result.stdout or ""
        except eryx.TimeoutError:
            output = "Error: execution timed out"
        except eryx.ExecutionError as e:
            output = f"Error: {e}"
        except Exception as e:
            output = f"Error: {type(e).__name__}: {e}"

        if len(output) > self._max_output_chars:
            truncated = output[: self._max_output_chars]
            output = (
                f"{truncated}\n\n... (output truncated to "
                f"{self._max_output_chars} chars, "
                f"{len(output)} total)"
            )
        return output

    def submit_answer(
        self,
        answer: Optional[str] = None,
        variable_name: Optional[str] = None,
    ) -> str:
        """Submit the final answer after analysis is complete.

        Provide either `answer` with the text directly, or `variable_name`
        to read the answer from a variable in the sandbox session.
        """
        if variable_name is not None:
            value = self._read_variable(variable_name)
            if value is None:
                return f"Error: variable '{variable_name}' not found in session"
            return f"FINAL ANSWER: {value}\n\n{self._usage_summary()}"
        elif answer is not None:
            return f"FINAL ANSWER: {answer}\n\n{self._usage_summary()}"
        else:
            return "Error: provide either 'answer' or 'variable_name'"


@llm.hookimpl
def register_tools(register):
    register(RLMToolbox)
