"""DSPy harness around datasette-agent's read-only SQL answering feature.

Faithfulness goals:
- The tools the DSPy agent calls are datasette-agent's REAL tool
  implementations (`_sql_query`, `_describe_table`,
  `_list_databases_and_tables` from datasette_agent.sql_tools), executed
  against a real in-process Datasette instance, with tool output passed
  through the production `prepare_tool_output_for_model()` filter.
- Tool names, descriptions and argument descriptions are pulled from the
  production `get_default_tools()` registrations.
- The DSPy program's instruction text is seeded with the REAL static
  system prompt extracted from `datasette_agent.agent._build_system_prompt`,
  and the dynamic "Available databases and tables" suffix is supplied as
  the `schema_hint` input, mirroring how production appends it per-request.

The user-visible side channel matters: when the model calls sql_query with
display='user' or 'both', production renders a table for the user. Our
metric therefore scores the union of the final answer text and any rows
rendered for the user, so display='user' isn't unfairly penalized.
"""

import asyncio
import json
import re
import threading
from pathlib import Path

import dspy

HERE = Path(__file__).parent
DB_PATH = HERE / "books.db"
DATASET_PATH = HERE / "dataset.json"

# ---------------------------------------------------------------------------
# Background event loop so sync DSPy tools can call async datasette code,
# including from dspy.Evaluate worker threads.
# ---------------------------------------------------------------------------

_loop = asyncio.new_event_loop()
threading.Thread(target=_loop.run_forever, daemon=True).start()


def run_async(coro, timeout=90):
    return asyncio.run_coroutine_threadsafe(coro, _loop).result(timeout=timeout)


# ---------------------------------------------------------------------------
# Real Datasette instance + real datasette-agent tools
# ---------------------------------------------------------------------------

from datasette.app import Datasette  # noqa: E402
from datasette_agent.agent import _build_system_prompt  # noqa: E402
from datasette_agent.messages import prepare_tool_output_for_model  # noqa: E402
from datasette_agent.sql_tools import (  # noqa: E402
    _describe_table,
    _list_databases_and_tables,
    _sql_query,
    get_default_tools,
)

datasette = Datasette([str(DB_PATH)])
run_async(datasette.invoke_startup())

ACTOR = None  # anonymous; default Datasette policy allows read-only SQL

_full_prompt = run_async(_build_system_prompt(datasette, ACTOR))
_marker = "\nAvailable databases and tables:"
if _marker in _full_prompt:
    BASELINE_INSTRUCTIONS, _suffix = _full_prompt.split(_marker, 1)
    SCHEMA_HINT = _marker.strip() + "\n" + _suffix.strip("\n")
else:  # pragma: no cover
    BASELINE_INSTRUCTIONS = _full_prompt
    SCHEMA_HINT = ""
BASELINE_INSTRUCTIONS = BASELINE_INSTRUCTIONS.strip()

# Per-forward capture of rows rendered for the user (display='user'/'both').
_capture = threading.local()


def _record_user_visible(payload):
    tables = getattr(_capture, "tables", None)
    if tables is None:
        return
    rows = payload.get("_rows") or payload.get("rows") or []
    tables.append({"columns": payload.get("columns"), "rows": rows})


def sql_query(database: str, sql: str, display: str = "model") -> str:
    result = run_async(
        _sql_query(datasette, ACTOR, database=database, sql=sql, display=display)
    )
    try:
        payload = json.loads(result)
    except json.JSONDecodeError:
        payload = None
    if payload and display in ("user", "both") and "error" not in payload:
        _record_user_visible(payload)
    return prepare_tool_output_for_model(result)


def describe_table(database: str, table: str) -> str:
    return prepare_tool_output_for_model(
        run_async(_describe_table(datasette, ACTOR, database=database, table=table))
    )


def list_databases_and_tables() -> str:
    return prepare_tool_output_for_model(
        run_async(_list_databases_and_tables(datasette, ACTOR))
    )


def make_dspy_tools():
    """dspy.Tool wrappers carrying the production names/descriptions."""
    impls = {
        "sql_query": sql_query,
        "describe_table": describe_table,
        "list_databases_and_tables": list_databases_and_tables,
    }
    tools = []
    for agent_tool in get_default_tools():
        fn = impls.get(agent_tool.name)
        if fn is None:  # skip execute_write_sql: read-only feature only
            continue
        arg_desc = {
            name: spec["description"]
            for name, spec in agent_tool.input_schema.get("properties", {}).items()
            if "description" in spec
        }
        tools.append(
            dspy.Tool(
                fn,
                name=agent_tool.name,
                desc=agent_tool.description,
                arg_desc=arg_desc,
            )
        )
    return tools


# ---------------------------------------------------------------------------
# The DSPy program under optimization
# ---------------------------------------------------------------------------


class DatasetteAgent(dspy.Module):
    """dspy.ReAct agent whose instructions are datasette-agent's system
    prompt and whose tools are datasette-agent's real tool functions."""

    def __init__(self, instructions=BASELINE_INSTRUCTIONS, max_iters=8):
        super().__init__()
        signature = dspy.Signature(
            {
                "schema_hint": dspy.InputField(
                    desc="Available databases and tables, as provided by the host"
                ),
                "question": dspy.InputField(desc="The user's question about the data"),
                "answer": dspy.OutputField(
                    desc="Your final response to the user, rendered as markdown"
                ),
            },
            instructions,
        )
        self.react = dspy.ReAct(signature, tools=make_dspy_tools(), max_iters=max_iters)

    def forward(self, schema_hint, question):
        _capture.tables = []
        try:
            prediction = self.react(schema_hint=schema_hint, question=question)
        finally:
            tables = getattr(_capture, "tables", [])
            _capture.tables = None
        return dspy.Prediction(
            answer=prediction.answer,
            user_visible_tables=tables,
            trajectory=getattr(prediction, "trajectory", None),
        )


# ---------------------------------------------------------------------------
# Dataset loading
# ---------------------------------------------------------------------------


def load_examples():
    items = json.loads(DATASET_PATH.read_text())
    train, test = [], []
    for item in items:
        example = dspy.Example(
            schema_hint=SCHEMA_HINT,
            question=item["question"],
            checks=item["checks"],
            gold_sql=item["gold_sql"],
            gold_answer=item["gold_answer"],
        ).with_inputs("schema_hint", "question")
        (train if item["split"] == "train" else test).append(example)
    return train, test


# ---------------------------------------------------------------------------
# Metric: every gold check value must be visible to the user, either in the
# answer text or in a table rendered via display='user'/'both'.
# ---------------------------------------------------------------------------

_NUMBER_WORDS = {
    0: "zero", 1: "one", 2: "two", 3: "three", 4: "four", 5: "five",
    6: "six", 7: "seven", 8: "eight", 9: "nine", 10: "ten",
}


def _normalize(text):
    # strip thousands separators inside numbers, collapse markdown escapes
    text = re.sub(r"(?<=\d),(?=\d\d\d)", "", text)
    text = text.replace("\\_", "_").replace("**", "")
    return text.lower()


def _number_present(text, value):
    if float(value).is_integer():
        variants = [str(int(value))]
        if int(value) in _NUMBER_WORDS:
            variants.append(_NUMBER_WORDS[int(value)])
    else:
        variants = [f"{value}", f"{value:.2f}", f"{value:,.2f}".replace(",", "")]
    for variant in variants:
        if re.search(
            rf"(?<![\d.\w]){re.escape(variant)}(?![\d])", text
        ):
            return True
    return False


def check_present(text, value):
    if isinstance(value, dict) and "any" in value:
        return any(check_present(text, alt) for alt in value["any"])
    if isinstance(value, bool):
        value = str(value).lower()
    if isinstance(value, (int, float)):
        return _number_present(text, value)
    return _normalize(str(value)) in text


def user_visible_text(prediction):
    parts = [prediction.answer or ""]
    for table in prediction.user_visible_tables or []:
        parts.append(json.dumps(table, default=str))
    return _normalize("\n".join(parts))


def metric(gold, prediction, trace=None, pred_name=None, pred_trace=None):
    visible = user_visible_text(prediction)
    missing = [c for c in gold.checks if not check_present(visible, c)]
    score = (len(gold.checks) - len(missing)) / len(gold.checks)
    if pred_name is None:
        return score
    if not missing:
        feedback = (
            "Correct. The user-visible output contained every expected value "
            f"({gold.checks!r})."
        )
    else:
        feedback = (
            f"The user asked: {gold.question!r}. The correct answer is "
            f"{gold.gold_answer!r} (one correct SQL query: {gold.gold_sql}). "
            f"These expected values were missing from the user-visible output: "
            f"{missing!r}. Note the user only sees the final answer text plus "
            "any tables rendered with display='user' or display='both' - rows "
            "fetched with display='model' are not shown to the user. Make sure "
            "the final answer states the specific values, uses correct SQL "
            "(watch out for joins, NULL handling, cancelled orders, and "
            "date filtering), and does not stop before answering."
        )
    return dspy.Prediction(score=score, feedback=feedback)


def configure_lm(model="openai/gpt-4.1-mini", **kwargs):
    lm = dspy.LM(model, temperature=0.0, max_tokens=4000, **kwargs)
    dspy.configure(lm=lm)
    return lm


if __name__ == "__main__":
    print("=== Extracted baseline instructions (static system prompt) ===")
    print(BASELINE_INSTRUCTIONS)
    print()
    print("=== Schema hint (dynamic suffix) ===")
    print(SCHEMA_HINT)
    print()
    print("=== Tool smoke test ===")
    print(list_databases_and_tables())
    print(describe_table("books", "authors")[:200])
    print(sql_query("books", "SELECT COUNT(*) AS n FROM books"))
    print(sql_query("books", "SELECT title FROM books LIMIT 2", display="user"))
