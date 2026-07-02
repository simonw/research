# Using DSPy to evaluate and improve Datasette Agent's SQL system prompts

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

**Task:** Install the latest Datasette alpha, `datasette-agent` and `dspy`, then
figure out how to use DSPy to evaluate and improve the main system prompts used
by Datasette Agent for its read-only SQL question answering feature.

**TL;DR:** It works, and the harness is the interesting part: DSPy can evaluate
and rewrite datasette-agent's *actual* production system prompt while calling
datasette-agent's *actual* tool implementations against a real in-process
Datasette. `dspy.GEPA` fixed the exact training failure its feedback showed it
(+5 points train), but with only 20 training questions it also introduced one
regression on the held-out set (−10 points test) — and the regression is a
genuinely instructive one: GEPA's added advice collided with the prompt's own
`display`-mode semantics. Two of the three apparent baseline failures turned
out to be bugs in my metric, which is the other headline lesson: at this scale,
eval quality dominates optimizer quality.

## Versions

- `datasette` 1.0a35 (installed with `pip install --pre datasette`)
- `datasette-agent` 0.3a0
- `dspy` 3.2.1

## Where the prompts live

The read-only SQL answering feature in `datasette-agent` is driven by:

1. **The main system prompt** — `_build_system_prompt(datasette, actor)` in
   `datasette_agent/agent.py`. A static instruction block (role, tool-calling
   rules, markdown underscore escaping, and the rules for picking the
   `display` mode of `sql_query` results) plus a dynamic suffix listing every
   database/table the actor has `execute-sql` permission on.
2. **Tool descriptions** — `get_default_tools()` in
   `datasette_agent/sql_tools.py` registers `list_databases_and_tables`,
   `describe_table` and `sql_query` (read-only, 1,000 row cap, with a
   3-way `display` parameter: `model`, `both`, `user`).

The agent loop itself is `model.chain(...)` from the `llm`/`datasette-llm`
libraries with `system=` set to that prompt and those tools attached. Tool
output is filtered through `prepare_tool_output_for_model()` which strips
`_`-prefixed side-channel keys (like `_html` and `_rows`) before the model
sees it.

## Approach

See `harness.py`. The key idea is to make DSPy evaluate/optimize the *real*
production artifacts rather than a paraphrase of them:

- **Real tools**: the DSPy agent's tools are datasette-agent's actual tool
  implementations (`_sql_query`, `_describe_table`,
  `_list_databases_and_tables`) executed against a real in-process
  `Datasette` instance, with output passed through the production
  `prepare_tool_output_for_model()` filter. Tool names, descriptions, and
  argument descriptions are pulled from the production `get_default_tools()`
  registrations.
- **Real prompt**: the static system prompt text is extracted at runtime from
  `_build_system_prompt()` and used as the instructions of a `dspy.ReAct`
  signature (`schema_hint, question -> answer`). The dynamic
  "Available databases and tables" suffix is passed as the `schema_hint`
  input on every request, mirroring how production injects it per-request.
  This means an optimized instruction string can be pasted straight back
  into `_build_system_prompt()`.
- **Faithful `display` semantics**: when the model picks `display='user'`,
  production shows the user a rendered table while hiding the rows from the
  model. The harness records those user-visible tables, and the metric
  scores the union of the final answer text and any user-visible tables — so
  the prompt's token-saving `display='user'` guidance is not unfairly
  penalized.

### Evaluation dataset

`make_dataset.py` deterministically generates `books.db` (a small bookstore:
`authors`, `books`, `customers`, `orders`, `order_items` with NULLs,
discounts, cancelled orders, and customers who never ordered) and 30
natural-language questions. Gold answers are **computed by executing SQL
against the generated database**, so they are correct by construction. Each
question carries a list of `checks` — values that must appear in the
user-visible output. Questions cover joins, NULL handling, date filtering,
ties, top-N lists, percentages, and edge cases (answer `0`).

Split: 20 train (GEPA rollouts + candidate selection), 10 held-out test.

### Metric

`metric()` in `harness.py` scores the fraction of gold check values present
in the user-visible output (number matching handles thousands separators,
spelled-out small numbers, and markdown escapes). For GEPA it also returns
*textual feedback*: the question, the gold answer, one correct SQL query, and
which values were missing — this is what GEPA's reflection model uses to
rewrite the instructions.

### Optimizer

`optimize.py` runs `dspy.GEPA` (Genetic-Pareto, DSPy 3.x's reflective prompt
optimizer, `auto="light"`) with `gpt-4.1-nano` as the task model and
`gpt-5-mini` as the reflection model. GEPA runs the agent on training
questions, reads the trajectories plus metric feedback, and proposes
rewritten instruction text; candidates are kept on a Pareto front across
training examples.

## Results

Scores are % of gold check values visible to the user (final answer text plus
any `display='user'`/`'both'` tables), averaged over the split. All runs use
temperature 0.

### Model choice

| Task model | Train (20 q) | Test (10 q) |
|---|---|---|
| gpt-4.1-mini, baseline prompt | 95.0 | 96.7 |
| gpt-4.1-nano, baseline prompt | 90.0 | 81.7 |

(Both rows scored with the first-pass metric, before the metric fixes
described below.) gpt-4.1-mini is already near ceiling, so there is little
for a prompt optimizer to do. gpt-4.1-nano has real headroom and represents
a realistic "run the agent on a cheap model" scenario, so that's what I
optimized for.

### GEPA optimization (task = gpt-4.1-nano, reflection = gpt-5-mini, auto=light)

GEPA accepted one mutation: it rewrote the ~2,400-char production system
prompt into an ~8,800-char rulebook (see `baseline_prompt.txt` vs
`optimized_prompt.txt`). After that it plateaued — nearly every training
minibatch already scored 100%, so reflection had nothing to work with.

With the corrected metric (see below):

| Program | Train (20 q) | Held-out test (10 q) |
|---|---|---|
| Baseline production prompt | 90.0 | 95.0 |
| GEPA-optimized prompt | **95.0** | 85.0 |

GEPA fixed exactly what its feedback showed it: the one failing training
question (revenue *including* cancelled orders) now passes, thanks to added
SQL guidance about status filters, `COALESCE`, and using
`order_items.unit_price` rather than list price. But it cost a regression on
a held-out revenue question. Classic small-trainset overfitting: +5 train,
−10 test.

### The regression is the most interesting finding

The optimized prompt added the plausible-sounding advice *"if unsure, query
the distinct statuses first (e.g. `SELECT DISTINCT status FROM orders`)"* —
with an example that used `display='user'`. On the held-out question
gpt-4.1-nano followed it literally: it ran `SELECT DISTINCT status` with
`display='user'`, and since `display='user'` **hides the rows from the
model** (that is its entire purpose — the table renders for the user only),
the agent saw just `row_count: 3`, re-ran the identical query three times,
exhausted its ReAct iteration budget, and finished with "here is the SQL I
would run" instead of the number. The optimizer's new advice interacted
badly with the display-mode semantics defined elsewhere in the same prompt.
Optimized prompts need regression review just like code — and this suggests a
manual hardening of the production prompt: *"never use `display='user'` for
data you yourself need to read."*

### Two of three "failures" were metric bugs

The first-pass test scores (baseline 81.7, optimized 71.7) overstated the
drop because two shared failures were scorer artifacts, not agent errors:

1. **Semantic zero** — gold answer `0`; both programs correctly answered
   "no books have never been ordered", which contains no literal `0`.
2. **A tie** — the top-3-books question has a genuine tie for 3rd place
   (24 copies each); the gold checks accepted only one tiebreak order.

I fixed both with any-of check groups (`{"any": [...]}`) in the dataset and
metric, and re-scored: baseline test rose from 81.7 to 95.0. If GEPA had been
run against the buggy metric *with these questions in its trainset*, it would
have optimized toward the scorer's quirks. **Before optimizing a prompt
against a metric, debug the metric against real transcripts.**

### Remaining known failure

Both programs answer "Which customer spent the most…" with "customer ID 16
($943.82)" — never joining to `customers` for the name (score 0.5). The fix
is a prompt rule like *"prefer human-readable identifiers: join to look up
names for IDs you report."* GEPA couldn't discover it because the failure
only occurs in the held-out split — trainset coverage bounds what the
optimizer can learn.

## Takeaways for datasette-agent

1. This harness (real tools + real prompt extracted from
   `_build_system_prompt`, optimized instructions paste back into
   `parts[0]`) is a workable regression-test suite for prompt changes, not
   just an optimizer input.
2. Concrete prompt-improvement candidates surfaced by the run:
   - "Never use `display='user'` for data you need to read yourself."
   - "Prefer human-readable identifiers — join to fetch names for IDs."
   - The schema listing gives only table names; the "don't call
     describe_table if you already have the information" advice caused
     column-name guessing (`page_count`, `o.order_id`, `first_name`) and
     error-retry loops in baseline traces. Either include column names in
     the prompt's schema listing or soften that advice.
3. GEPA needs a bigger, harder, well-debugged eval set to be trusted here:
   20 questions with a 90% baseline leaves it almost nothing to learn from,
   and a 10-question holdout makes every question worth 10 points.

## Files

| File | Purpose |
|---|---|
| `make_dataset.py` | Deterministic test db + gold QA dataset generator |
| `harness.py` | Datasette instance, real-tool DSPy wrappers, agent module, metric |
| `evaluate.py` | `dspy.Evaluate` runner for baseline/optimized programs |
| `optimize.py` | `dspy.GEPA` optimization run |
| `dataset.json` | 30 questions with gold SQL/answers/checks |
| `books.db` | Generated SQLite database (regenerate with `make_dataset.py`) |
| `baseline_prompt.txt` | The production system prompt (static part), extracted at runtime |
| `optimized_prompt.txt` | GEPA's rewritten instructions |
| `optimized_program.json` | Saved DSPy program (load with `evaluate.py --program`) |
| `baseline_nano_results.json`, `optimized_nano_results.json` | Final scores (corrected metric) |
| `gepa_summary.json` | GEPA candidate scores / best index |

## Reproducing

```bash
python -m venv venv
venv/bin/pip install --pre datasette
venv/bin/pip install datasette-agent dspy
venv/bin/python make_dataset.py
venv/bin/python evaluate.py --model openai/gpt-4.1-nano   # baseline
venv/bin/python optimize.py --task-model openai/gpt-4.1-nano \
    --reflection-model openai/gpt-5-mini --auto light
venv/bin/python evaluate.py --model openai/gpt-4.1-nano \
    --program optimized_program.json --split test
```
