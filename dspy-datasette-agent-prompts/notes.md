# Notes: Using DSPy to evaluate and improve Datasette Agent's SQL system prompts

## Goal
Install latest Datasette alpha + datasette-agent + dspy, then figure out how to use
DSPy to evaluate and improve the main system prompts datasette-agent uses for its
read-only-SQL question answering feature.

## Setup
- Created venv (not committed), installed:
  - datasette 1.0a35 (via `pip install --pre datasette`)
  - datasette-agent 0.3a0
  - dspy 3.2.1
- OPENAI_API_KEY is available in this environment; DSPy can use it via LiteLLM.

## Where the prompts live in datasette-agent 0.3a0

- `datasette_agent/agent.py` → `_build_system_prompt(datasette, actor)`:
  the MAIN system prompt. Static instruction text covering:
  - role ("helpful data analysis assistant")
  - don't re-list tables you already know
  - flat JSON tool args, always pass `database`, no `kwargs` wrapper
  - markdown underscore escaping
  - the `display` mode selection rules for sql_query (model/both/user)
  - don't repeat rendered tables in text
  Plus a dynamic suffix: "Available databases and tables:" listing every
  db the actor has `execute-sql` permission on.
- `datasette_agent/sql_tools.py` → `get_default_tools()`: tool
  descriptions + JSON schema for `list_databases_and_tables`,
  `describe_table`, `sql_query` (read-only, 1000 row limit, `display`
  param description), `execute_write_sql`.
- The agent loop is `model.chain(...)` from datasette-llm/llm with
  `system=system_prompt` and those tools.

The read-only SQL answering feature = system prompt + the three read-only
tools (`list_databases_and_tables`, `describe_table`, `sql_query`), where
`sql_query`'s `_sql_query()` enforces `execute-sql` permission and
Datasette itself only allows read-only SQL through `db.execute()`.

## Plan for DSPy

1. Build a small in-process Datasette with a test SQLite db; reuse the REAL
   tool implementations (`_sql_query`, `_describe_table`,
   `_list_databases_and_tables`) as DSPy tools so tool behavior is faithful.
2. Wrap the agent as `dspy.ReAct` whose signature instructions are seeded
   with the REAL `_build_system_prompt` static text (so DSPy is evaluating
   and rewriting the actual production prompt).
3. Create a question/gold-answer dataset over the test db (train/dev/test).
4. Metric: programmatic check that gold values appear in the answer +
   feedback text (GEPA wants textual feedback).
5. Evaluate baseline with `dspy.Evaluate`, then optimize with `dspy.GEPA`
   (light budget), re-evaluate on held-out test, diff the prompts.

## Baseline results (dspy.Evaluate, metric = fraction of gold values visible to user)

- Task model gpt-4.1-mini: train 95.0, test 96.7 — near ceiling already
- Task model gpt-4.1-nano: train 90.0, test 81.7 — real headroom, and this
  is a realistic scenario (running the agent on a cheap model)

Observation from traces: the agent frequently GUESSES column names
(page_count, publication_year, o.order_id, b.book_id, first_name) and only
recovers via SQL error messages. The production prompt actively discourages
calling describe_table ("do not call ... if you already have the
information"), but the schema listing only includes table names, not
columns — so the discouragement backfires into error-retry loops.

Decision: optimize for gpt-4.1-nano as task LM, GEPA auto=light,
reflection LM gpt-5-mini.

## GEPA run (auto=light, task=gpt-4.1-nano, reflection=gpt-5-mini)

- Budget computed by auto=light: 850 rollouts.
- Iteration 0: base program valset (train) score 0.9.
- Early iterations often skipped ("all subsample scores perfect") because
  most minibatches score 1.0 already — headroom is concentrated in a few
  hard questions.
- Iteration 3: first accepted mutation of react.react instructions
  (subsample 2.0 -> 3.0), promoted to full eval.

## GEPA outcome and analysis

GEPA (auto=light, 850 rollouts budget) accepted ONE mutation (iteration 3):
a rewrite of the react.react instructions from the 2,400-char production
prompt to an 8,800-char rulebook. Train (=valset) went 0.90 -> 0.95. It then
plateaued: nearly every subsequent minibatch scored perfect, so reflective
mutation had nothing to chew on ("All subsample scores perfect. Skipping.").

Held-out test (10 questions, never seen by GEPA):
- FIRST scoring: baseline 81.7 vs optimized 71.7 - looked like overfitting.
- Per-question diff showed 2 of the 3 shared "failures" were METRIC BUGS:
  1. gold answer 0: both programs correctly said "no books have never been
     ordered" but the checker demanded a literal 0/zero.
  2. top-3 books: 3rd place is a genuine TIE (24 copies each for Songs of
     River and The Last Harbor); gold accepted only one tiebreak order.
- Fixed the metric/dataset (any-of check groups), re-scored:
  baseline train 90.0 / test 95.0; optimized train 95.0 / test 85.0.

The one real regression is instructive. GEPA added: "if unsure, query the
distinct statuses first (e.g. SELECT DISTINCT status FROM orders)" and its
example used display='user'. nano followed it literally: ran SELECT DISTINCT
status with display='user' - which HIDES THE ROWS FROM THE MODEL (that's
what display='user' means!) - saw only row_count=3, re-ran the same query
3x, burned its 8 ReAct iterations, and finished with "here is the SQL I
would run" instead of the number. The optimizer's advice collided with the
display-mode semantics defined elsewhere in the same prompt.

The remaining shared failure (both programs, 0.5): "Which customer spent
the most..." - both answer "customer ID 16 ($943.82)" without joining to
customers for the name. A prompt fix ("prefer human-readable identifiers -
join to get names") would address it; GEPA never saw it because it's in the
test split.

## Lessons

1. The harness works: DSPy can evaluate and rewrite datasette-agent's real
   system prompt against its real tools. GEPA did fix the exact failure its
   feedback showed it (revenue-incl-cancelled train question).
2. With 20 train examples and a strong baseline, light-budget GEPA
   overfits: +5 train, -10 test.
3. Metric quality dominates at this scale: 2 of 3 apparent baseline
   failures were scorer bugs (semantic zero, ties). Fix the metric before
   trusting - let alone optimizing against - the numbers.
4. Optimized prompts need regression review like code: the distinct-status
   advice was plausible but interacted badly with display-mode semantics.
5. Tool-visibility semantics (display='user' hiding rows) are a subtle trap
   for both models and prompt optimizers; the prompt should say "never use
   display='user' for data you yourself need".

## Cost

Whole investigation ~US$2-3 of OpenAI API usage (nano/mini task runs,
gpt-5-mini reflection).
