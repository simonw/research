# Implementation notes

This is a 169-case pilot of Qwen3.8 27B Q4_K_M with medium reasoning enabled and no explicit generation-token limit.

- One frozen case is used from each ordered digit-length cell (`a_digits` 1–13 × `b_digits` 1–13). Each case is replicate 1 from the established 30-per-cell seeded manifest; no operands were resampled.
- Cases run easiest first: ascending maximum operand length, minimum operand length, carry count, longest carry chain, and result length. Original seeded request order is the final tie-breaker.
- The first scored case is `7 + 9`; the final scored case is `4299366105622 + 6088794067970`.
- The user message and greedy decoding settings are unchanged. Reasoning is explicitly enabled with `enable_thinking=true`, `reasoning_effort=medium`, and `reasoning_format=deepseek`.
- Every request explicitly sends `max_tokens=-1`. In this exact llama.cpp build, `-1` means no prediction-count limit. The server also launches with `--n-predict -1` and `--reasoning-budget -1`.
- The Python HTTP client has no socket deadline. The server has a four-hour transport timeout, which is not a token budget.
- Context shifting is disabled. Generation therefore ends naturally on EOS/stop or, at the latest, at the fixed 32,768-token context boundary. “Unlimited” means no artificial reasoning/completion-token cap, not infinite physical context.
- Reasoning is captured separately. Only the visible answer is graded; hidden reasoning is never used to salvage an answer.
- The server uses llama.cpp build `b1-c398c6e`, one slot, and no speculative decoding.
- The smoke test is excluded from `pairs.csv`, `attempts.jsonl`, and `results.csv`.
- Early progress is intentionally biased toward the easiest cases and must not be interpreted as the pilot's overall accuracy.

The superseded 2,048-token, hardest-first pilot was stopped after five completed cases and preserved separately as `qwen3.8-27b-q4_k_m-reasoning-medium-max2048-one-per-cell-hardest-first-aborted-after-5-seed-20260929`.

Runner SHA-256: `7972eb8fa9d908689967de8a182a92ea1718c05d84f8c1003ae44498dc7fa3d5`

## Postprocessing-only accessibility revision

While inference was running, the grid renderer was changed from a red/yellow/green scale to a colorblind-safe orange/neutral/blue scale using Okabe-Ito endpoints. The orange and blue endpoints also have distinct grayscale lightness, and numeric percentage labels remain printed in every observed cell, so color is not the sole carrier of correctness. Black text on orange and white text on blue both exceed a 4.5:1 contrast ratio. This revision does not alter requests, raw responses, parsing, grading, or inference order.

Postprocessing SHA-256: `c80f6405460e9fe2ac5aa3a94c15c8ba96d5eeb62c00cb0073e05162116bd7cc`
