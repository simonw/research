#!/usr/bin/env python3
"""Build the concise reasoning-on versus no-thinking Qwen3.8 report."""

from __future__ import annotations

import csv
import datetime as dt
import hashlib
import json
import statistics
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parent
CURRENT_DIR = ROOT / "runs/qwen3.8-27b-q4_k_m-reasoning-medium-unlimited-one-per-cell-easiest-first-seed-20260929"
PREVIOUS_DIR = ROOT / "runs/qwen3.8-27b-q4_k_m-seed-20260929"
REPORT_PATH = ROOT / "QWEN38_27B_REASONING_ON_VS_OFF_SHORT_REPORT.md"
REPORT_MANIFEST_PATH = ROOT / "QWEN38_27B_REASONING_ON_VS_OFF_SHORT_REPORT.manifest.json"
ONE_THIRD_ORDER = 56


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def truth(value: Any) -> bool:
    return str(value).strip().lower() == "true"


def fenced(text: str) -> str:
    longest = 0
    run = 0
    for char in text:
        if char == "`":
            run += 1
            longest = max(longest, run)
        else:
            run = 0
    fence = "`" * max(3, longest + 1)
    return f"{fence}text\n{text}\n{fence}"


def pct(numerator: int, denominator: int) -> str:
    return f"{100 * numerator / denominator:.2f}%"


def metric_transition(
    current: dict[str, dict[str, str]],
    previous: dict[str, dict[str, str]],
    field: str,
) -> dict[str, int]:
    counts = {"both": 0, "current_only": 0, "previous_only": 0, "neither": 0}
    for case_id in current:
        now = truth(current[case_id][field])
        before = truth(previous[case_id][field])
        if now and before:
            counts["both"] += 1
        elif now:
            counts["current_only"] += 1
        elif before:
            counts["previous_only"] += 1
        else:
            counts["neither"] += 1
    return counts


def atomic_write(path: Path, content: str) -> None:
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(content, encoding="utf-8")
    temporary.replace(path)


def main() -> None:
    current_results_path = CURRENT_DIR / "results.csv"
    current_attempts_path = CURRENT_DIR / "attempts.jsonl"
    current_pairs_path = CURRENT_DIR / "pairs.csv"
    current_summary_path = CURRENT_DIR / "summary.json"
    current_manifest_path = CURRENT_DIR / "manifest.json"
    previous_results_path = PREVIOUS_DIR / "results.csv"
    previous_manifest_path = PREVIOUS_DIR / "manifest.json"

    current_rows = read_csv(current_results_path)
    if len(current_rows) != 169:
        raise SystemExit(f"Current run is not complete: expected 169 results, found {len(current_rows)}")
    current = {row["case_id"]: row for row in current_rows}
    if len(current) != 169:
        raise SystemExit("Current results contain duplicate case IDs")

    previous_all = read_csv(previous_results_path)
    previous = {row["case_id"]: row for row in previous_all if row["case_id"] in current}
    if set(previous) != set(current):
        raise SystemExit("Previous run does not contain the exact 169 current case IDs")

    immutable = [
        "case_id",
        "a_digits",
        "b_digits",
        "replicate",
        "a",
        "b",
        "correct_answer",
        "result_digits",
        "carry_count",
        "longest_carry_chain",
        "expected_words",
        "prompt",
    ]
    for case_id, row in current.items():
        for field in immutable:
            if str(row[field]) != str(previous[case_id][field]):
                raise SystemExit(f"Immutable mismatch for {case_id}.{field}")

    attempts: dict[str, dict[str, Any]] = {}
    attempt_count = 0
    with current_attempts_path.open(encoding="utf-8") as handle:
        for line in handle:
            attempt_count += 1
            record = json.loads(line)
            case_id = record["case_id"]
            if case_id in attempts:
                raise SystemExit(f"Duplicate/retry attempt found for {case_id}")
            if record.get("http_status") != 200:
                raise SystemExit(f"Non-200 attempt found for {case_id}")
            choices = (record.get("response_json") or {}).get("choices") or []
            if not choices:
                raise SystemExit(f"Missing response choice for {case_id}")
            attempts[case_id] = record
    if attempt_count != 169 or set(attempts) != set(current):
        raise SystemExit("Raw attempts do not exactly match the 169 final results")

    current_by_order = sorted(current_rows, key=lambda row: int(row["request_order"]))
    if [int(row["request_order"]) for row in current_by_order] != list(range(1, 170)):
        raise SystemExit("Current request order is not exactly 1..169")

    easiest = current_by_order[0]
    one_third = next(row for row in current_by_order if int(row["request_order"]) == ONE_THIRD_ORDER)
    correct_rows = [row for row in current_by_order if truth(row["numeric_correct"])]
    hardest_correct = max(correct_rows, key=lambda row: int(row["request_order"]))
    wrong_rows = [row for row in current_by_order if not truth(row["numeric_correct"])]

    metrics = []
    for label, field in [
        ("Numeric correctness", "numeric_correct"),
        ("Instruction compliance", "instruction_compliant"),
        ("Canonical wording", "canonical_text_correct"),
    ]:
        previous_n = sum(truth(previous[case_id][field]) for case_id in current)
        current_n = sum(truth(row[field]) for row in current_rows)
        transitions = metric_transition(current, previous, field)
        metrics.append((label, field, previous_n, current_n, transitions))

    previous_times = [float(previous[case_id]["request_elapsed_seconds"]) for case_id in current]
    current_times = [float(row["request_elapsed_seconds"]) for row in current_rows]
    previous_tokens = [int(previous[case_id]["completion_tokens"]) for case_id in current]
    current_tokens = [int(row["completion_tokens"]) for row in current_rows]

    numeric = next(item for item in metrics if item[1] == "numeric_correct")
    delta_pp = 100 * (numeric[3] - numeric[2]) / 169

    lines = [
        f"# Medium-reasoning configuration scored {pct(numeric[3], 169)} versus {pct(numeric[2], 169)} without thinking",
        "",
        "This is a like-for-like comparison of the same 169 frozen `r01` additions—one from each ordered 1–13 digit cell. The medium-reasoning run gained "
        f"{delta_pp:.2f} percentage points in visible-answer accuracy, but it also used an unlimited prediction setting instead of the previous 128-token cap. It is therefore a configuration comparison, not an isolated causal estimate of reasoning alone.",
        "",
        "## What changed",
        "",
        "| Setting | Previous run | New run |",
        "|---|---:|---:|",
        "| Reasoning | Disabled | Medium, separated reasoning channel |",
        "| Completion limit | 128 tokens | `max_tokens=-1` (bounded by 32,768-token context) |",
        "| Cases compared | Same 169 `r01` cases | Same 169 `r01` cases |",
        "| Execution order | Seeded shuffle inside the 5,070-case run | Easiest to hardest |",
        "| Decoding | Greedy | Greedy |",
        "",
        "## Paired results",
        "",
        "| Measure | No thinking | Medium reasoning | Change | Paired transitions (both / new only / old only / neither) |",
        "|---|---:|---:|---:|---:|",
    ]
    for label, _field, previous_n, current_n, transitions in metrics:
        lines.append(
            f"| {label} | {previous_n}/169 ({pct(previous_n, 169)}) | "
            f"{current_n}/169 ({pct(current_n, 169)}) | "
            f"{100 * (current_n - previous_n) / 169:+.2f} pp | "
            f"{transitions['both']} / {transitions['current_only']} / "
            f"{transitions['previous_only']} / {transitions['neither']} |"
        )
    lines.extend(
        [
            "",
            f"The new run produced {len(wrong_rows)} wrong visible answer(s). All 169 requests completed without retry or API failure.",
            "",
            "| Runtime measure | No thinking | Medium reasoning |",
            "|---|---:|---:|",
            f"| Median latency | {statistics.median(previous_times):.2f} s | {statistics.median(current_times):.2f} s |",
            f"| Mean latency | {statistics.fmean(previous_times):.2f} s | {statistics.fmean(current_times):.2f} s |",
            f"| Median completion tokens | {statistics.median(previous_tokens):.0f} | {statistics.median(current_tokens):.0f} |",
            f"| Total completion tokens | {sum(previous_tokens):,} | {sum(current_tokens):,} |",
            "",
            "## Requested transcripts",
            "",
            "The reasoning blocks and visible responses below are copied verbatim from the saved API response JSON. Hidden reasoning is shown for inspection only and was never used to grade or rescue an answer.",
            "",
        ]
    )

    def append_transcript(row: dict[str, str], heading: str) -> None:
        case_id = row["case_id"]
        record = attempts[case_id]
        choice = record["response_json"]["choices"][0]
        message = choice.get("message") or {}
        reasoning = str(message.get("reasoning_content") or "")
        response = str(message.get("content") or "")
        prior = previous[case_id]
        lines.extend(
            [
                f"### {heading}: request {row['request_order']} — `{case_id}`",
                "",
                f"- Calculation: `{int(row['a']):,} + {int(row['b']):,} = {int(row['correct_answer']):,}`",
                f"- Current grade: **{'correct' if truth(row['numeric_correct']) else 'wrong'}**; finish `{row['finish_reason']}`; {int(row['completion_tokens']):,} completion tokens; {float(row['request_elapsed_seconds']):.2f} seconds",
                f"- Previous no-thinking grade on the same case: **{'correct' if truth(prior['numeric_correct']) else 'wrong'}**",
                f"- Previous no-thinking visible response: {json.dumps(prior['raw_response'], ensure_ascii=False)}",
                f"- Reasoning SHA-256: `{hashlib.sha256(reasoning.encode('utf-8')).hexdigest()}`",
                f"- Visible-response SHA-256: `{hashlib.sha256(response.encode('utf-8')).hexdigest()}`",
                "",
                "#### Full reasoning transcript",
                "",
                fenced(reasoning),
                "",
                "#### Full visible response",
                "",
                fenced(response),
                "",
            ]
        )

    append_transcript(easiest, "Easiest calculation")
    append_transcript(one_third, "Approximately one-third through the run")
    append_transcript(hardest_correct, "Hardest calculation answered correctly")

    lines.extend(["## Every wrong answer in the medium-reasoning run", ""])
    for index, row in enumerate(wrong_rows, start=1):
        append_transcript(row, f"Wrong answer {index}")

    lines.extend(
        [
            "## Reproducibility",
            "",
            f"- Current raw attempts: `{current_attempts_path.relative_to(ROOT)}`",
            f"- Current graded results: `{current_results_path.relative_to(ROOT)}`",
            f"- Previous graded results: `{previous_results_path.relative_to(ROOT)}`",
            f"- Current pair manifest: `{current_pairs_path.relative_to(ROOT)}`",
            "- The comparison joined on `case_id` and verified operands, answers, prompt, digit features, replicate, and expected wording for all 169 cases.",
            "",
        ]
    )

    content = "\n".join(lines)
    atomic_write(REPORT_PATH, content)
    manifest = {
        "generated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "builder_sha256": sha256(Path(__file__)),
        "report": str(REPORT_PATH.relative_to(ROOT)),
        "report_sha256": sha256(REPORT_PATH),
        "sources": {
            str(current_attempts_path.relative_to(ROOT)): sha256(current_attempts_path),
            str(current_results_path.relative_to(ROOT)): sha256(current_results_path),
            str(current_pairs_path.relative_to(ROOT)): sha256(current_pairs_path),
            str(current_summary_path.relative_to(ROOT)): sha256(current_summary_path),
            str(current_manifest_path.relative_to(ROOT)): sha256(current_manifest_path),
            str(previous_results_path.relative_to(ROOT)): sha256(previous_results_path),
            str(previous_manifest_path.relative_to(ROOT)): sha256(previous_manifest_path),
        },
        "selections": {
            "easiest": easiest["case_id"],
            "one_third_request_order": ONE_THIRD_ORDER,
            "one_third": one_third["case_id"],
            "hardest_correct": hardest_correct["case_id"],
            "wrong_cases": [row["case_id"] for row in wrong_rows],
        },
    }
    atomic_write(REPORT_MANIFEST_PATH, json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(REPORT_PATH)
    print(REPORT_MANIFEST_PATH)


if __name__ == "__main__":
    main()
