#!/usr/bin/env python3
"""Qwen3.8 one-per-cell addition pilot with uncapped medium reasoning."""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import hashlib
import http.client
import json
import math
import os
import platform
import random
import re
import statistics
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable
from urllib.parse import urlsplit
from urllib.request import urlopen


ENDPOINT = "http://127.0.0.1:8082"
MODEL = "/home/spark/Qwen3.8-27B-Q4_K_M.gguf"
RUN_DIR = Path(
    "runs/qwen3.8-27b-q4_k_m-reasoning-medium-unlimited-one-per-cell-easiest-first-seed-20260929"
)
PAIR_SEED = 20260929
REQUEST_ORDER_SEED = 20260930
DIGIT_MIN = 1
DIGIT_MAX = 13
SOURCE_PAIRS_PER_CELL = 30
PAIRS_PER_CELL = 1
TEMPERATURE = 0
MAX_TOKENS = -1
REQUEST_TIMEOUT_SECONDS = None
MAX_ATTEMPTS = 3
MAX_PARSED_NUMBER = 20_000_000_000_000
ENABLE_THINKING = True
REASONING_EFFORT = "medium"
REASONING_FORMAT = "deepseek"
PROMPT_TEMPLATE = (
    "What is {a} + {b}? Please write your answer in words. "
    "Do not include any other text or information, just the answer in words."
)

SMALL = {
    "zero": 0,
    "one": 1,
    "two": 2,
    "three": 3,
    "four": 4,
    "five": 5,
    "six": 6,
    "seven": 7,
    "eight": 8,
    "nine": 9,
    "ten": 10,
    "eleven": 11,
    "twelve": 12,
    "thirteen": 13,
    "fourteen": 14,
    "fifteen": 15,
    "sixteen": 16,
    "seventeen": 17,
    "eighteen": 18,
    "nineteen": 19,
}
TENS = {
    "twenty": 20,
    "thirty": 30,
    "forty": 40,
    "fifty": 50,
    "sixty": 60,
    "seventy": 70,
    "eighty": 80,
    "ninety": 90,
}
SCALES = {
    "thousand": 1_000,
    "million": 1_000_000,
    "billion": 1_000_000_000,
    "trillion": 1_000_000_000_000,
    "quadrillion": 1_000_000_000_000_000,
}
ONES_WORDS = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
]
TENS_WORDS = [
    "",
    "",
    "twenty",
    "thirty",
    "forty",
    "fifty",
    "sixty",
    "seventy",
    "eighty",
    "ninety",
]
SCALE_WORDS = ["", "thousand", "million", "billion", "trillion", "quadrillion"]
ALLOWED_TOKEN_RE = re.compile(r"^[A-Za-z]+$")
TERMINAL_PUNCTUATION_RE = re.compile(r"[.!?]+$")

RESULT_FIELDS = [
    "case_id",
    "a_digits",
    "b_digits",
    "replicate",
    "request_order",
    "a",
    "b",
    "correct_answer",
    "result_digits",
    "carry_count",
    "longest_carry_chain",
    "expected_words",
    "prompt",
    "raw_response",
    "reasoning_content",
    "parsed_answer",
    "numeric_correct",
    "instruction_compliant",
    "canonical_text_correct",
    "parse_error",
    "finish_reason",
    "completion_tokens",
    "prompt_tokens",
    "total_tokens",
    "prompt_ms",
    "generation_ms",
    "tokens_per_second",
    "request_elapsed_seconds",
    "response_id",
    "created",
    "system_fingerprint",
]


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat()


def atomic_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    with temporary.open("w", encoding="utf-8") as handle:
        json.dump(value, handle, indent=2, ensure_ascii=False, sort_keys=True)
        handle.write("\n")
        handle.flush()
        os.fsync(handle.fileno())
    temporary.replace(path)


def append_jsonl(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(value, ensure_ascii=False, sort_keys=True) + "\n")
        handle.flush()
        os.fsync(handle.fileno())


def fetch_json(url: str, timeout: int = 10) -> Any:
    with urlopen(url, timeout=timeout) as response:
        return json.loads(response.read().decode("utf-8"))


def post_json(url: str, payload: dict[str, Any], timeout: int = 30) -> Any:
    parsed = urlsplit(url)
    connection = http.client.HTTPConnection(
        parsed.hostname or "127.0.0.1", parsed.port or 80, timeout=timeout
    )
    try:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        connection.request(
            "POST",
            parsed.path or "/",
            body=body,
            headers={"Content-Type": "application/json"},
        )
        response = connection.getresponse()
        raw = response.read()
        if not 200 <= response.status < 300:
            raise RuntimeError(
                f"POST {url} returned HTTP {response.status}: "
                f"{raw.decode('utf-8', errors='replace')}"
            )
        return json.loads(raw.decode("utf-8"))
    finally:
        connection.close()


def log(message: str) -> None:
    timestamped = f"{utc_now()} {message}"
    print(timestamped, flush=True)
    RUN_DIR.mkdir(parents=True, exist_ok=True)
    with (RUN_DIR / "run.log").open("a", encoding="utf-8") as handle:
        handle.write(timestamped + "\n")


def under_thousand_to_words(number: int) -> str:
    if not 0 <= number < 1000:
        raise ValueError(number)
    pieces: list[str] = []
    if number >= 100:
        pieces.extend((ONES_WORDS[number // 100], "hundred"))
        number %= 100
    if number >= 20:
        tens_word = TENS_WORDS[number // 10]
        remainder = number % 10
        pieces.append(f"{tens_word}-{ONES_WORDS[remainder]}" if remainder else tens_word)
    elif number:
        pieces.append(ONES_WORDS[number])
    return " ".join(pieces)


def number_to_words(number: int) -> str:
    if number < 0:
        return "minus " + number_to_words(-number)
    if number == 0:
        return "zero"
    chunks: list[str] = []
    scale_index = 0
    while number:
        chunk = number % 1000
        if chunk:
            words = under_thousand_to_words(chunk)
            if SCALE_WORDS[scale_index]:
                words += " " + SCALE_WORDS[scale_index]
            chunks.append(words)
        number //= 1000
        scale_index += 1
    return " ".join(reversed(chunks))


def tokenize_number_words(text: str) -> tuple[list[str] | None, str]:
    if not isinstance(text, str) or not text.strip():
        return None, "empty_response"
    normalized = text.strip().lower()
    normalized = normalized.replace("’", "'").replace("–", "-").replace("—", "-")
    normalized = TERMINAL_PUNCTUATION_RE.sub("", normalized).strip()
    normalized = normalized.replace(",", " ").replace("-", " ")
    tokens = normalized.split()
    if not tokens:
        return None, "empty_response"
    for token in tokens:
        if not ALLOWED_TOKEN_RE.fullmatch(token):
            return None, f"non_word_token:{token}"
        if token not in SMALL and token not in TENS and token not in SCALES and token not in {
            "hundred",
            "and",
        }:
            return None, f"unknown_word:{token}"
    return tokens, ""


def parse_number_words(text: str) -> tuple[int | None, str]:
    tokens, error = tokenize_number_words(text)
    if tokens is None:
        return None, error
    if tokens == ["zero"]:
        return 0, ""
    if "zero" in tokens:
        return None, "zero_in_nonzero_number"

    total = 0
    group = 0
    last_large_scale = math.inf
    saw_value = False
    previous_kind = "start"
    for token_index, token in enumerate(tokens):
        if token == "and":
            if previous_kind not in {"hundred", "scale"}:
                return None, "misplaced_and"
            if token_index + 1 >= len(tokens):
                return None, "incomplete_number"
            next_token = tokens[token_index + 1]
            if next_token == "zero" or (next_token not in SMALL and next_token not in TENS):
                return None, "misplaced_and"
            previous_kind = "and"
            continue
        if token in SMALL:
            value = SMALL[token]
            if previous_kind == "small":
                return None, "invalid_small_number_sequence"
            if previous_kind == "tens" and not 0 < value < 10:
                return None, "invalid_small_number_after_tens"
            group += value
            saw_value = True
            previous_kind = "small"
            continue
        if token in TENS:
            if previous_kind in {"small", "tens"}:
                return None, "invalid_tens_sequence"
            group += TENS[token]
            saw_value = True
            previous_kind = "tens"
            continue
        if token == "hundred":
            if previous_kind != "small" or group <= 0 or group >= 10:
                return None, "invalid_hundred"
            group *= 100
            previous_kind = "hundred"
            continue
        scale = SCALES[token]
        if group <= 0:
            return None, "scale_without_value"
        if scale >= last_large_scale:
            return None, "non_descending_scale"
        total += group * scale
        group = 0
        last_large_scale = scale
        previous_kind = "scale"

    if not saw_value or previous_kind == "and":
        return None, "incomplete_number"
    parsed = total + group
    if parsed > MAX_PARSED_NUMBER:
        return None, "number_above_supported_maximum"
    return parsed, ""


def canonicalize_text(text: str, *, drop_and: bool = False) -> str | None:
    tokens, _ = tokenize_number_words(text)
    if tokens is None:
        return None
    if drop_and:
        tokens = [token for token in tokens if token != "and"]
    return " ".join(tokens)


def exact_digit_integer(rng: random.Random, digits: int) -> int:
    lower = 1 if digits == 1 else 10 ** (digits - 1)
    upper = 10**digits - 1
    return rng.randint(lower, upper)


def carry_features(a: int, b: int) -> tuple[int, int]:
    carry = 0
    carry_count = 0
    current_chain = 0
    longest_chain = 0
    while a or b:
        column_total = (a % 10) + (b % 10) + carry
        carry = int(column_total >= 10)
        if carry:
            carry_count += 1
            current_chain += 1
            longest_chain = max(longest_chain, current_chain)
        else:
            current_chain = 0
        a //= 10
        b //= 10
    return carry_count, longest_chain


def generate_pairs() -> list[dict[str, Any]]:
    rng = random.Random(PAIR_SEED)
    rows: list[dict[str, Any]] = []
    for a_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
        for b_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
            for replicate in range(1, SOURCE_PAIRS_PER_CELL + 1):
                a = exact_digit_integer(rng, a_digits)
                b = exact_digit_integer(rng, b_digits)
                answer = a + b
                carry_count, longest_carry_chain = carry_features(a, b)
                case_id = f"a{a_digits:02d}_b{b_digits:02d}_r{replicate:02d}"
                rows.append(
                    {
                        "case_id": case_id,
                        "a_digits": a_digits,
                        "b_digits": b_digits,
                        "replicate": replicate,
                        "request_order": 0,
                        "a": a,
                        "b": b,
                        "correct_answer": answer,
                        "result_digits": len(str(answer)),
                        "carry_count": carry_count,
                        "longest_carry_chain": longest_carry_chain,
                        "expected_words": number_to_words(answer),
                        "prompt": PROMPT_TEMPLATE.format(a=a, b=b),
                    }
                )
    # Recreate the original 30-per-cell frozen design, including its shuffle,
    # then select replicate 1 from each cell. This preserves the exact operands
    # from the established manifest rather than drawing a new 169-case sample.
    random.Random(REQUEST_ORDER_SEED).shuffle(rows)
    for source_request_order, row in enumerate(rows, start=1):
        row["request_order"] = source_request_order
    rows = [row for row in rows if row["replicate"] <= PAIRS_PER_CELL]

    # Deliberately present easier cells first. Length dominates the ordering;
    # known carry features break ties. The original shuffled order is the final
    # deterministic tie-breaker.
    rows.sort(
        key=lambda row: (
            max(row["a_digits"], row["b_digits"]),
            min(row["a_digits"], row["b_digits"]),
            row["carry_count"],
            row["longest_carry_chain"],
            row["result_digits"],
            row["request_order"],
        )
    )
    for request_order, row in enumerate(rows, start=1):
        row["request_order"] = request_order
    return rows


def write_csv(path: Path, rows: Iterable[dict[str, Any]], fields: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    with temporary.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(rows)
        handle.flush()
        os.fsync(handle.fileno())
    temporary.replace(path)


def read_pairs() -> list[dict[str, Any]]:
    path = RUN_DIR / "pairs.csv"
    if not path.exists():
        raise SystemExit(
            "pairs.csv is missing; run `python3 benchmark_qwen38_reasoning.py prepare` first"
        )
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    integer_fields = [
        "a_digits",
        "b_digits",
        "replicate",
        "request_order",
        "a",
        "b",
        "correct_answer",
        "result_digits",
        "carry_count",
        "longest_carry_chain",
    ]
    for row in rows:
        for field in integer_fields:
            row[field] = int(row[field])
    return rows


def request_payload(case: dict[str, Any], *, thinking: bool, max_tokens: int) -> dict[str, Any]:
    case_seed = int.from_bytes(
        hashlib.sha256(str(case["case_id"]).encode("utf-8")).digest()[:4], "big"
    )
    if case_seed == 0xFFFFFFFF:
        case_seed -= 1
    return {
        "model": MODEL,
        "messages": [{"role": "user", "content": case["prompt"]}],
        "temperature": TEMPERATURE,
        "top_k": 0,
        "top_p": 1,
        "min_p": 0,
        "repeat_penalty": 1,
        "max_tokens": max_tokens,
        "seed": case_seed,
        "stream": False,
        "reasoning_effort": REASONING_EFFORT,
        "reasoning_format": REASONING_FORMAT,
        "chat_template_kwargs": {"enable_thinking": thinking},
    }


class ModelClient:
    def __init__(self, endpoint: str, timeout: float | None) -> None:
        parsed = urlsplit(endpoint)
        if parsed.scheme != "http":
            raise ValueError("This runner currently expects a local http:// endpoint")
        self.host = parsed.hostname or "127.0.0.1"
        self.port = parsed.port or 80
        self.timeout = timeout
        self.connection: http.client.HTTPConnection | None = None

    def close(self) -> None:
        if self.connection is not None:
            self.connection.close()
            self.connection = None

    def post(self, payload: dict[str, Any]) -> tuple[int, dict[str, str], bytes]:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        for connection_attempt in range(2):
            try:
                if self.connection is None:
                    self.connection = http.client.HTTPConnection(
                        self.host, self.port, timeout=self.timeout
                    )
                self.connection.request(
                    "POST",
                    "/v1/chat/completions",
                    body=body,
                    headers={"Content-Type": "application/json"},
                )
                response = self.connection.getresponse()
                raw = response.read()
                headers = {key.lower(): value for key, value in response.getheaders()}
                return response.status, headers, raw
            except (OSError, TimeoutError, http.client.HTTPException):
                self.close()
                if connection_attempt:
                    raise
        raise RuntimeError("unreachable")


def extract_response_text(response_json: dict[str, Any]) -> tuple[str, str, str]:
    choices = response_json.get("choices") or []
    if not choices:
        return "", "", "missing_choices"
    choice = choices[0]
    message = choice.get("message") or {}
    content = message.get("content")
    reasoning = message.get("reasoning_content")
    if isinstance(content, list):
        content = "".join(
            item.get("text", "") for item in content if isinstance(item, dict)
        )
    return str(content or ""), str(reasoning or ""), str(choice.get("finish_reason") or "")


def grade(case: dict[str, Any], response_json: dict[str, Any], elapsed: float) -> dict[str, Any]:
    content, reasoning, finish_reason = extract_response_text(response_json)
    parsed, parse_error = parse_number_words(content)
    instruction_compliant = parsed is not None
    numeric_correct = parsed == case["correct_answer"]
    expected_canonical = canonicalize_text(case["expected_words"], drop_and=False)
    actual_canonical = canonicalize_text(content, drop_and=False)
    canonical_text_correct = actual_canonical == expected_canonical
    usage = response_json.get("usage") or {}
    timings = response_json.get("timings") or {}
    return {
        **case,
        "raw_response": content,
        "reasoning_content": reasoning,
        "parsed_answer": "" if parsed is None else parsed,
        "numeric_correct": numeric_correct,
        "instruction_compliant": instruction_compliant,
        "canonical_text_correct": canonical_text_correct,
        "parse_error": parse_error,
        "finish_reason": finish_reason,
        "completion_tokens": usage.get("completion_tokens", ""),
        "prompt_tokens": usage.get("prompt_tokens", ""),
        "total_tokens": usage.get("total_tokens", ""),
        "prompt_ms": timings.get("prompt_ms", ""),
        "generation_ms": timings.get("predicted_ms", ""),
        "tokens_per_second": timings.get("predicted_per_second", ""),
        "request_elapsed_seconds": round(elapsed, 6),
        "response_id": response_json.get("id", ""),
        "created": response_json.get("created", ""),
        "system_fingerprint": response_json.get("system_fingerprint", ""),
    }


def load_successes(pairs_by_id: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    attempts_path = RUN_DIR / "attempts.jsonl"
    successes: dict[str, dict[str, Any]] = {}
    if not attempts_path.exists():
        return successes
    with attempts_path.open(encoding="utf-8") as handle:
        for line_number, line in enumerate(handle, start=1):
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                log(f"Ignoring malformed attempts.jsonl line {line_number}")
                continue
            case_id = record.get("case_id")
            if (
                case_id in pairs_by_id
                and record.get("http_status") == 200
                and isinstance(record.get("response_json"), dict)
                and (record["response_json"].get("choices") or [])
            ):
                elapsed = float(record.get("elapsed_seconds") or 0)
                successes[case_id] = grade(
                    pairs_by_id[case_id], record["response_json"], elapsed
                )
    return successes


def save_results(results: dict[str, dict[str, Any]]) -> None:
    ordered = [results[key] for key in sorted(results)]
    write_csv(RUN_DIR / "results.csv", ordered, RESULT_FIELDS)


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def command_output(arguments: list[str]) -> dict[str, Any]:
    try:
        completed = subprocess.run(
            arguments,
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
        return {
            "argv": arguments,
            "returncode": completed.returncode,
            "stdout": completed.stdout,
            "stderr": completed.stderr,
        }
    except Exception as exc:
        return {"argv": arguments, "error": f"{type(exc).__name__}: {exc}"}


def snapshot_environment(model_path: Path, props: dict[str, Any]) -> dict[str, Any]:
    os_release: dict[str, str] = {}
    os_release_path = Path("/etc/os-release")
    if os_release_path.exists():
        for line in os_release_path.read_text(encoding="utf-8").splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                os_release[key] = value.strip().strip('"')
    serving_processes: list[dict[str, Any]] = []
    for proc_dir in Path("/proc").glob("[0-9]*"):
        try:
            raw = (proc_dir / "cmdline").read_bytes()
            command = raw.replace(b"\0", b" ").decode("utf-8", errors="replace").strip()
        except (OSError, PermissionError):
            continue
        if str(model_path) in command:
            serving_processes.append({"pid": int(proc_dir.name), "command": command})
    return {
        "captured_at": utc_now(),
        "platform": platform.platform(),
        "uname": list(platform.uname()),
        "machine": platform.machine(),
        "python": sys.version,
        "os_release": os_release,
        "model": {
            "path": str(model_path),
            "size_bytes": model_path.stat().st_size,
            "sha256": file_sha256(model_path),
            "server_reported_format": props.get("model_ftype"),
        },
        "chat_template_sha256": hashlib.sha256(
            str(props.get("chat_template", "")).encode("utf-8")
        ).hexdigest(),
        "serving_processes": serving_processes,
        "nvidia_smi": command_output(
            [
                "nvidia-smi",
                "--query-gpu=name,driver_version,utilization.gpu",
                "--format=csv,noheader",
            ]
        ),
        "memory": command_output(["free", "-b"]),
    }


def prepare() -> None:
    RUN_DIR.mkdir(parents=True, exist_ok=True)
    pairs = generate_pairs()
    pair_fields = [
        "case_id",
        "a_digits",
        "b_digits",
        "replicate",
        "request_order",
        "a",
        "b",
        "correct_answer",
        "result_digits",
        "carry_count",
        "longest_carry_chain",
        "expected_words",
        "prompt",
    ]
    pairs_path = RUN_DIR / "pairs.csv"
    if pairs_path.exists():
        existing = pairs_path.read_bytes()
        temporary = RUN_DIR / "pairs.generated.csv"
        write_csv(temporary, pairs, pair_fields)
        generated = temporary.read_bytes()
        temporary.unlink()
        if existing != generated:
            raise SystemExit("Existing pairs.csv differs; refusing to overwrite immutable inputs")
    else:
        write_csv(pairs_path, pairs, pair_fields)

    props = fetch_json(f"{ENDPOINT}/props")
    models = fetch_json(f"{ENDPOINT}/v1/models")
    atomic_json(RUN_DIR / "server_props.json", props)
    atomic_json(RUN_DIR / "server_models.json", models)
    rendered_prompt = post_json(
        f"{ENDPOINT}/apply-template",
        request_payload(pairs[0], thinking=ENABLE_THINKING, max_tokens=MAX_TOKENS),
    )
    rendered_text = str(rendered_prompt.get("prompt", ""))
    if "Reasoning effort is set to xhigh." in rendered_text:
        raise SystemExit("Rendered prompt unexpectedly selected xhigh reasoning effort")
    if "<|im_start|>system" in rendered_text:
        raise SystemExit("Rendered medium-effort prompt unexpectedly injected a system message")
    if "<|im_start|>assistant\n<think>\n" not in rendered_text:
        raise SystemExit("Rendered prompt did not open an assistant reasoning block")
    if "<|im_start|>assistant\n<think>\n\n</think>" in rendered_text:
        raise SystemExit("Rendered prompt unexpectedly pre-closed the reasoning block")
    atomic_json(RUN_DIR / "rendered_prompt_reasoning.json", rendered_prompt)
    environment = snapshot_environment(Path(MODEL), props)
    atomic_json(RUN_DIR / "environment.json", environment)
    pair_sha256 = hashlib.sha256(pairs_path.read_bytes()).hexdigest()
    script_sha256 = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    manifest = {
        "benchmark": "addition-in-words-one-per-cell-medium-reasoning-unlimited-pilot",
        "benchmark_version": 1,
        "prepared_at": utc_now(),
        "endpoint": ENDPOINT,
        "model": MODEL,
        "model_format": props.get("model_ftype"),
        "server_build": props.get("build_info"),
        "system_fingerprint": None,
        "pair_seed": PAIR_SEED,
        "request_order_seed": REQUEST_ORDER_SEED,
        "digit_range": [DIGIT_MIN, DIGIT_MAX],
        "pairs_per_ordered_cell": PAIRS_PER_CELL,
        "expected_cases": len(pairs),
        "pair_selection": (
            "Replicate 1 selected from each ordered digit-length cell of the "
            "established 30-per-cell frozen design; 169 total cases"
        ),
        "source_pairs_per_ordered_cell": SOURCE_PAIRS_PER_CELL,
        "request_order": (
            "Easiest first: ascending maximum operand digits, minimum operand "
            "digits, carry count, longest carry chain, and result digits; the "
            "original seeded shuffled order is the final tie-breaker"
        ),
        "prompt_template": PROMPT_TEMPLATE,
        "decoding": {
            "temperature": TEMPERATURE,
            "top_k": 0,
            "top_p": 1,
            "min_p": 0,
            "repeat_penalty": 1,
            "max_tokens": MAX_TOKENS,
            "stream": False,
            "chat_template_kwargs": {"enable_thinking": ENABLE_THINKING},
            "reasoning_effort": REASONING_EFFORT,
            "reasoning_format": REASONING_FORMAT,
            "request_timeout_seconds": REQUEST_TIMEOUT_SECONDS,
            "note": (
                "Reasoning enabled both by the server and by per-request "
                "enable_thinking=true. Every request explicitly selects medium "
                "reasoning effort and deepseek-format reasoning extraction. "
                "max_tokens=-1 removes the prediction-count limit in this "
                "llama.cpp build, and the client has no socket timeout. Generation "
                "still ends naturally on EOS/stop or runtime/context constraints."
            ),
        },
        "primary_metric": "whole-response English-number parse equals a+b",
        "grading": {
            "numeric_correct": "Complete visible response parses as number words and equals the integer sum.",
            "instruction_compliant": "Complete visible response contains only valid English-number syntax.",
            "canonical_text_correct": "Normalized visible response exactly matches the canonical US-English rendering.",
        },
        "pairs_csv_sha256": pair_sha256,
        "benchmark_script_sha256": script_sha256,
        "model_sha256": environment["model"]["sha256"],
        "chat_template_sha256": environment["chat_template_sha256"],
    }
    atomic_json(RUN_DIR / "manifest.json", manifest)
    log(f"Prepared {len(pairs)} immutable cases in {RUN_DIR}")


def smoke() -> None:
    pairs = generate_pairs()
    smoke_answer = 93_847_696
    smoke_carries, smoke_longest_chain = carry_features(677, 93_847_019)
    example = {
        **pairs[0],
        "case_id": "smoke_reference_example",
        "a": 677,
        "b": 93_847_019,
        "a_digits": 3,
        "b_digits": 8,
        "correct_answer": smoke_answer,
        "result_digits": len(str(smoke_answer)),
        "carry_count": smoke_carries,
        "longest_carry_chain": smoke_longest_chain,
        "expected_words": number_to_words(smoke_answer),
        "prompt": PROMPT_TEMPLATE.format(a=677, b=93_847_019),
    }
    client = ModelClient(ENDPOINT, REQUEST_TIMEOUT_SECONDS)
    configurations = [
        {
            "name": "benchmark_reasoning_on_medium_unlimited",
            "thinking": ENABLE_THINKING,
            "max_tokens": MAX_TOKENS,
        },
    ]
    for configuration in configurations:
        payload = request_payload(
            example,
            thinking=configuration["thinking"],
            max_tokens=configuration["max_tokens"],
        )
        started = utc_now()
        monotonic_start = time.monotonic()
        try:
            status, headers, raw = client.post(payload)
            elapsed = time.monotonic() - monotonic_start
            try:
                response_json = json.loads(raw.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError):
                response_json = None
            record = {
                "name": configuration["name"],
                "started_at": started,
                "finished_at": utc_now(),
                "elapsed_seconds": elapsed,
                "case": example,
                "request": payload,
                "http_status": status,
                "response_headers": headers,
                "response_body_utf8": raw.decode("utf-8", errors="replace"),
                "response_json": response_json,
            }
            if status == 200 and isinstance(response_json, dict):
                record["grade"] = grade(example, response_json, elapsed)
            append_jsonl(RUN_DIR / "smoke_tests.jsonl", record)
            content = (
                extract_response_text(response_json)[0]
                if isinstance(response_json, dict)
                else ""
            )
            reasoning = (
                extract_response_text(response_json)[1]
                if isinstance(response_json, dict)
                else ""
            )
            log(
                f"Smoke {configuration['name']}: HTTP {status}, "
                f"{elapsed:.2f}s, reasoning_chars={len(reasoning)}, visible={content!r}"
            )
        except Exception as exc:  # diagnostic must still be saved
            elapsed = time.monotonic() - monotonic_start
            append_jsonl(
                RUN_DIR / "smoke_tests.jsonl",
                {
                    "name": configuration["name"],
                    "started_at": started,
                    "finished_at": utc_now(),
                    "elapsed_seconds": elapsed,
                    "case": example,
                    "request": payload,
                    "error_type": type(exc).__name__,
                    "error": str(exc),
                },
            )
            log(f"Smoke {configuration['name']} failed: {type(exc).__name__}: {exc}")
    client.close()


def run_benchmark(limit: int | None = None) -> None:
    pairs = read_pairs()
    pairs_by_id = {row["case_id"]: row for row in pairs}
    results = load_successes(pairs_by_id)
    total_target = len(pairs) if limit is None else min(len(pairs), limit)
    target_pairs = pairs[:total_target]
    pending = [row for row in target_pairs if row["case_id"] not in results]
    log(
        f"Run start: target={total_target}, already_complete="
        f"{total_target - len(pending)}, pending={len(pending)}"
    )
    client = ModelClient(ENDPOINT, REQUEST_TIMEOUT_SECONDS)
    run_started = time.monotonic()
    durations: list[float] = []
    try:
        for pending_index, case in enumerate(pending, start=1):
            completed = False
            for attempt_number in range(1, MAX_ATTEMPTS + 1):
                payload = request_payload(
                    case, thinking=ENABLE_THINKING, max_tokens=MAX_TOKENS
                )
                started_at = utc_now()
                monotonic_start = time.monotonic()
                record: dict[str, Any] = {
                    "case_id": case["case_id"],
                    "attempt": attempt_number,
                    "started_at": started_at,
                    "request": payload,
                }
                try:
                    status, headers, raw = client.post(payload)
                    elapsed = time.monotonic() - monotonic_start
                    record.update(
                        {
                            "finished_at": utc_now(),
                            "elapsed_seconds": elapsed,
                            "http_status": status,
                            "response_headers": headers,
                            "response_body_utf8": raw.decode("utf-8", errors="replace"),
                        }
                    )
                    try:
                        response_json = json.loads(raw.decode("utf-8"))
                        record["response_json"] = response_json
                    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
                        response_json = None
                        record["decode_error"] = f"{type(exc).__name__}: {exc}"
                    append_jsonl(RUN_DIR / "attempts.jsonl", record)
                    if status == 200 and isinstance(response_json, dict) and (
                        response_json.get("choices") or []
                    ):
                        result = grade(case, response_json, elapsed)
                        results[case["case_id"]] = result
                        durations.append(elapsed)
                        completed = True
                        break
                except Exception as exc:
                    elapsed = time.monotonic() - monotonic_start
                    record.update(
                        {
                            "finished_at": utc_now(),
                            "elapsed_seconds": elapsed,
                            "error_type": type(exc).__name__,
                            "error": str(exc),
                        }
                    )
                    append_jsonl(RUN_DIR / "attempts.jsonl", record)
                    client.close()
                if attempt_number < MAX_ATTEMPTS:
                    time.sleep(2 ** (attempt_number - 1))

            target_completed = sum(row["case_id"] in results for row in target_pairs)
            if not completed:
                log(f"Case {case['case_id']} failed after {MAX_ATTEMPTS} attempts")
            if pending_index % 10 == 0 or pending_index == len(pending):
                save_results(results)
                elapsed_run = time.monotonic() - run_started
                mean_duration = statistics.fmean(durations) if durations else None
                remaining = total_target - target_completed
                eta_seconds = mean_duration * remaining if mean_duration is not None else None
                progress = {
                    "updated_at": utc_now(),
                    "target_cases": total_target,
                    "completed_cases": target_completed,
                    "failed_or_pending_cases": remaining,
                    "completion_fraction": target_completed / total_target if total_target else 1,
                    "elapsed_this_invocation_seconds": elapsed_run,
                    "mean_request_seconds_this_invocation": mean_duration,
                    "eta_seconds": eta_seconds,
                }
                atomic_json(RUN_DIR / "progress.json", progress)
                accuracy = (
                    sum(bool(results[row["case_id"]]["numeric_correct"]) for row in target_pairs if row["case_id"] in results)
                    / target_completed
                    if target_completed
                    else 0
                )
                eta_text = (
                    f"{eta_seconds / 60:.1f}m" if eta_seconds is not None else "unknown"
                )
                log(
                    f"Progress {target_completed}/{total_target} "
                    f"({100 * target_completed / total_target:.1f}%), "
                    f"running accuracy={100 * accuracy:.1f}%, ETA={eta_text}"
                )
    except KeyboardInterrupt:
        log("Interrupted; saved progress can be resumed by rerunning the command")
        raise
    finally:
        client.close()
        save_results(results)


def bool_from_csv(value: Any) -> bool:
    return str(value).strip().lower() in {"true", "1", "yes"}


def load_results_csv() -> list[dict[str, Any]]:
    path = RUN_DIR / "results.csv"
    if not path.exists():
        raise SystemExit("results.csv is missing; run the benchmark first")
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    for row in rows:
        for key in [
            "a_digits",
            "b_digits",
            "replicate",
            "request_order",
            "a",
            "b",
            "correct_answer",
            "result_digits",
            "carry_count",
            "longest_carry_chain",
        ]:
            row[key] = int(row[key])
        for key in ["numeric_correct", "instruction_compliant", "canonical_text_correct"]:
            row[key] = bool_from_csv(row[key])
    return rows


def regrade_from_raw() -> None:
    pairs = read_pairs()
    pairs_by_id = {row["case_id"]: row for row in pairs}
    previous_rows = load_results_csv() if (RUN_DIR / "results.csv").exists() else []
    previous_by_id = {str(row["case_id"]): row for row in previous_rows}
    results = load_successes(pairs_by_id)
    if len(results) != len(pairs):
        raise SystemExit(
            f"Cannot finalize regrade: found {len(results)} successful raw responses for {len(pairs)} cases"
        )
    changed_primary = 0
    changed_compliance = 0
    changed_parsed_answer = 0
    for case_id, row in results.items():
        previous = previous_by_id.get(case_id)
        if not previous:
            continue
        changed_primary += bool(previous["numeric_correct"]) != bool(row["numeric_correct"])
        changed_compliance += bool(previous["instruction_compliant"]) != bool(
            row["instruction_compliant"]
        )
        changed_parsed_answer += str(previous.get("parsed_answer", "")) != str(
            row.get("parsed_answer", "")
        )
    save_results(results)
    postprocess = {
        "regraded_at": utc_now(),
        "grading_version": 2,
        "source": "append-only attempts.jsonl",
        "completed_cases": len(results),
        "changed_primary_numeric_correct_flags": changed_primary,
        "changed_instruction_compliance_flags": changed_compliance,
        "changed_parsed_answers": changed_parsed_answer,
        "postprocess_script_sha256": file_sha256(Path(__file__)),
        "attempts_jsonl_sha256": file_sha256(RUN_DIR / "attempts.jsonl"),
        "results_csv_sha256": file_sha256(RUN_DIR / "results.csv"),
        "grammar_change": (
            "Connector 'and' is accepted only after 'hundred' or a large scale; "
            "malformed phrases such as "
            "'one and forty-two billion' are rejected."
        ),
    }
    atomic_json(RUN_DIR / "postprocess_manifest.json", postprocess)
    log(
        f"Regraded {len(results)} raw responses: primary changes={changed_primary}, "
        f"compliance changes={changed_compliance}, parsed-answer changes={changed_parsed_answer}"
    )


def interpolate_color(value: float) -> tuple[int, int, int]:
    """Colorblind-safe orange-to-blue diverging scale (Okabe-Ito endpoints)."""
    value = max(0.0, min(1.0, value))
    # These endpoints remain distinct under common red/green deficiencies and
    # have materially different lightness in grayscale. Printed percentages
    # provide a second, non-color encoding.
    orange = (230, 159, 0)
    neutral = (247, 247, 247)
    blue = (0, 114, 178)
    if value <= 0.5:
        t = value / 0.5
        return tuple(
            round(orange[i] + t * (neutral[i] - orange[i])) for i in range(3)
        )
    t = (value - 0.5) / 0.5
    return tuple(round(neutral[i] + t * (blue[i] - neutral[i])) for i in range(3))


def build_heatmap(cell_rows: list[dict[str, Any]]) -> None:
    try:
        from PIL import Image, ImageDraw, ImageFont
    except ImportError:
        log("Pillow is unavailable; skipped heatmap.png")
        return

    cell_by_key = {
        (int(row["a_digits"]), int(row["b_digits"])): row for row in cell_rows
    }
    width, height = 1720, 1530
    left, top = 185, 170
    cell_size = 92
    image = Image.new("RGB", (width, height), "white")
    draw = ImageDraw.Draw(image)
    font_candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    ]
    bold_candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf",
    ]

    def load_font(candidates: list[str], size: int):
        for candidate in candidates:
            if Path(candidate).exists():
                return ImageFont.truetype(candidate, size=size)
        return ImageFont.load_default()

    title_font = load_font(bold_candidates, 38)
    subtitle_font = load_font(font_candidates, 25)
    label_font = load_font(font_candidates, 23)
    cell_font = load_font(bold_candidates, 21)
    small_font = load_font(font_candidates, 18)

    draw.text(
        (left, 28),
        "Addition in words — Qwen3.8 27B — medium reasoning pilot",
        fill="black",
        font=title_font,
    )
    draw.text(
        (left, 82),
        "1 fixed pair per ordered digit-length cell · easiest first (n = 169)",
        fill=(50, 50, 50),
        font=subtitle_font,
    )
    for a_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
        x = left + (a_digits - DIGIT_MIN) * cell_size
        draw.text((x + 36, top + 13 * cell_size + 12), str(a_digits), fill="black", font=label_font)
    for b_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
        y = top + (DIGIT_MAX - b_digits) * cell_size
        draw.text((left - 42, y + 31), str(b_digits), fill="black", font=label_font)
        for a_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
            x = left + (a_digits - DIGIT_MIN) * cell_size
            row = cell_by_key.get((a_digits, b_digits))
            has_observations = bool(row and str(row.get("numeric_accuracy", "")).strip())
            accuracy = float(row["numeric_accuracy"]) if has_observations else 0.0
            color = interpolate_color(accuracy) if has_observations else (224, 228, 232)
            draw.rectangle(
                (x, y, x + cell_size, y + cell_size),
                fill=color,
                outline=(255, 255, 255),
                width=1,
            )
            text = f"{100 * accuracy:.0f}%" if has_observations else "—"
            bbox = draw.textbbox((0, 0), text, font=cell_font)
            text_width = bbox[2] - bbox[0]
            text_height = bbox[3] - bbox[1]
            luminance = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2]
            text_color = "white" if luminance < 115 else "black"
            draw.text(
                (x + (cell_size - text_width) / 2, y + (cell_size - text_height) / 2 - 4),
                text,
                fill=text_color,
                font=cell_font,
            )

    x_label = "Number of digits in a"
    bbox = draw.textbbox((0, 0), x_label, font=label_font)
    draw.text(
        (left + (13 * cell_size - (bbox[2] - bbox[0])) / 2, top + 13 * cell_size + 56),
        x_label,
        fill="black",
        font=label_font,
    )
    y_label_layer = Image.new("RGBA", (600, 60), (255, 255, 255, 0))
    y_draw = ImageDraw.Draw(y_label_layer)
    y_draw.text((0, 10), "Number of digits in b", fill="black", font=label_font)
    y_label_layer = y_label_layer.rotate(90, expand=True)
    image.paste(y_label_layer, (35, top + 345), y_label_layer)

    legend_x = left + 13 * cell_size + 80
    legend_y = top + 160
    legend_height = 520
    draw.text((legend_x - 5, legend_y - 52), "Accuracy", fill="black", font=label_font)
    for offset in range(legend_height):
        value = 1 - offset / (legend_height - 1)
        draw.line(
            (legend_x, legend_y + offset, legend_x + 38, legend_y + offset),
            fill=interpolate_color(value),
        )
    for value in [1.0, 0.75, 0.5, 0.25, 0.0]:
        y = legend_y + round((1 - value) * (legend_height - 1))
        draw.line((legend_x + 39, y, legend_x + 49, y), fill="black", width=2)
        draw.text((legend_x + 57, y - 10), f"{value:.2f}", fill="black", font=small_font)
    image.save(RUN_DIR / "heatmap.png")


def wilson_interval(successes: int, count: int, z: float = 1.959963984540054) -> tuple[float | None, float | None]:
    if count <= 0:
        return None, None
    proportion = successes / count
    denominator = 1 + z * z / count
    center = (proportion + z * z / (2 * count)) / denominator
    margin = z * math.sqrt(
        proportion * (1 - proportion) / count + z * z / (4 * count * count)
    ) / denominator
    return center - margin, center + margin


def summarize() -> None:
    rows = load_results_csv()
    expected = (DIGIT_MAX - DIGIT_MIN + 1) ** 2 * PAIRS_PER_CELL
    grouped: dict[tuple[int, int], list[dict[str, Any]]] = defaultdict(list)
    for row in rows:
        grouped[(row["a_digits"], row["b_digits"])].append(row)

    cell_rows: list[dict[str, Any]] = []
    matrix: dict[int, dict[int, float | None]] = {
        b: {a: None for a in range(DIGIT_MIN, DIGIT_MAX + 1)}
        for b in range(DIGIT_MIN, DIGIT_MAX + 1)
    }
    for a_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
        for b_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
            cell = grouped.get((a_digits, b_digits), [])
            count = len(cell)
            numeric = sum(row["numeric_correct"] for row in cell)
            compliant = sum(row["instruction_compliant"] for row in cell)
            canonical = sum(row["canonical_text_correct"] for row in cell)
            accuracy = numeric / count if count else None
            wilson_low, wilson_high = wilson_interval(numeric, count)
            matrix[b_digits][a_digits] = accuracy
            cell_rows.append(
                {
                    "a_digits": a_digits,
                    "b_digits": b_digits,
                    "n": count,
                    "numeric_correct": numeric,
                    "numeric_accuracy": "" if accuracy is None else accuracy,
                    "numeric_accuracy_wilson_95_low": "" if wilson_low is None else wilson_low,
                    "numeric_accuracy_wilson_95_high": "" if wilson_high is None else wilson_high,
                    "instruction_compliance_accuracy": "" if not count else compliant / count,
                    "canonical_text_accuracy": "" if not count else canonical / count,
                }
            )

    cell_fields = [
        "a_digits",
        "b_digits",
        "n",
        "numeric_correct",
        "numeric_accuracy",
        "numeric_accuracy_wilson_95_low",
        "numeric_accuracy_wilson_95_high",
        "instruction_compliance_accuracy",
        "canonical_text_accuracy",
    ]
    write_csv(RUN_DIR / "cell_accuracy.csv", cell_rows, cell_fields)
    inline_rows = []
    for b_digits in range(DIGIT_MAX, DIGIT_MIN - 1, -1):
        for a_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
            cell = next(
                row
                for row in cell_rows
                if int(row["a_digits"]) == a_digits and int(row["b_digits"]) == b_digits
            )
            inline_rows.append(
                {
                    "aDigits": str(a_digits),
                    "bDigits": str(b_digits),
                    "correctRate": float(cell["numeric_accuracy"]),
                    "correct": int(cell["numeric_correct"]),
                    "n": int(cell["n"]),
                }
            )
    atomic_json(
        Path("visualizations/qwen38-reasoning-medium-unlimited-one-per-cell-heatmap.json"),
        {
            "schemaVersion": 1,
            "id": "qwen38-reasoning-medium-unlimited-one-per-cell-heatmap",
            "queryId": "local_qwen38_reasoning_medium_unlimited_one_per_cell_pilot",
            "title": "Addition accuracy by operand length",
            "description": (
                "Qwen3.8 27B Q4_K_M · medium reasoning · no generation-token cap "
                "· 1 fixed pair per cell · easiest first"
            ),
            "chart": {
                "type": "heatmap",
                "x": "aDigits",
                "y": "correctRate",
                "series": "bDigits",
                "xLabel": "Digits in a",
                "yLabel": "Digits in b",
                "showXAxisLabel": True,
                "showYAxisLabel": True,
                "showValues": True,
                "valueDecimals": 0,
                "colorDomain": [0, 1],
                "categoryOrder": [str(value) for value in range(DIGIT_MIN, DIGIT_MAX + 1)],
                "seriesOrder": [str(value) for value in range(DIGIT_MAX, DIGIT_MIN - 1, -1)],
                "rowHeight": 32,
                "tooltipFields": [
                    {"field": "aDigits", "label": "Digits in a"},
                    {"field": "bDigits", "label": "Digits in b"},
                    {"field": "correctRate", "label": "Accuracy"},
                    {"field": "correct", "label": "Correct"},
                    {"field": "n", "label": "Cases"},
                ],
                "legend": {"labels": {"correctRate": "Accuracy"}},
            },
            "rows": inline_rows,
            "source": {
                "label": "Local Qwen3.8 27B medium-reasoning addition pilot",
                "files": [
                    {
                        "label": "Reviewed 169-cell summary",
                        "path": str((RUN_DIR / "cell_accuracy.csv").resolve()),
                    }
                ],
                "caveats": [
                    "Each cell contains replicate 1 from the established frozen 30-pair design.",
                    "Cases are deliberately ordered from easiest to hardest; this is a one-per-cell pilot.",
                    "Greedy decoding uses medium reasoning with max_tokens=-1 and no client timeout; natural stop and runtime/context constraints still apply.",
                    "The rendered grid uses a colorblind-safe orange/neutral/blue scale and redundant numeric cell labels.",
                    "A 95% Wilson interval for each cell is retained in cell_accuracy.csv.",
                ],
            },
            "generatedAt": utc_now(),
            "height": 620,
            "theme": "codex-classic",
        },
    )
    matrix_rows: list[dict[str, Any]] = []
    for b_digits in range(DIGIT_MAX, DIGIT_MIN - 1, -1):
        row: dict[str, Any] = {"b_digits": b_digits}
        for a_digits in range(DIGIT_MIN, DIGIT_MAX + 1):
            value = matrix[b_digits][a_digits]
            row[f"a_digits_{a_digits}"] = "" if value is None else value
        matrix_rows.append(row)
    matrix_fields = ["b_digits"] + [
        f"a_digits_{a}" for a in range(DIGIT_MIN, DIGIT_MAX + 1)
    ]
    write_csv(RUN_DIR / "accuracy_matrix.csv", matrix_rows, matrix_fields)

    total = len(rows)
    completion_tokens = [
        int(row["completion_tokens"])
        for row in rows
        if str(row.get("completion_tokens", "")).isdigit()
    ]
    request_seconds = [
        float(row["request_elapsed_seconds"])
        for row in rows
        if str(row.get("request_elapsed_seconds", "")).strip()
    ]
    error_counts: dict[str, int] = defaultdict(int)
    finish_reason_counts: dict[str, int] = defaultdict(int)
    for row in rows:
        if row["parse_error"]:
            error_counts[str(row["parse_error"])] += 1
        finish_reason_counts[str(row["finish_reason"])] += 1
    correct_count = sum(row["numeric_correct"] for row in rows)
    overall_wilson_low, overall_wilson_high = wilson_interval(correct_count, total)
    fingerprints = sorted(
        {str(row["system_fingerprint"]) for row in rows if row.get("system_fingerprint")}
    )
    summary = {
        "generated_at": utc_now(),
        "model": MODEL,
        "completed_cases": total,
        "expected_cases": expected,
        "is_complete": total == expected,
        "numeric_correct": correct_count,
        "numeric_accuracy": correct_count / total if total else None,
        "numeric_accuracy_wilson_95_low": overall_wilson_low,
        "numeric_accuracy_wilson_95_high": overall_wilson_high,
        "instruction_compliant": sum(row["instruction_compliant"] for row in rows),
        "instruction_compliance_accuracy": (
            sum(row["instruction_compliant"] for row in rows) / total if total else None
        ),
        "canonical_text_correct": sum(row["canonical_text_correct"] for row in rows),
        "canonical_text_accuracy": (
            sum(row["canonical_text_correct"] for row in rows) / total if total else None
        ),
        "total_completion_tokens": sum(completion_tokens),
        "total_request_seconds": sum(request_seconds),
        "mean_request_seconds": statistics.fmean(request_seconds) if request_seconds else None,
        "median_request_seconds": statistics.median(request_seconds) if request_seconds else None,
        "parse_error_counts": dict(sorted(error_counts.items())),
        "finish_reason_counts": dict(sorted(finish_reason_counts.items())),
        "reasoning_content_nonempty": sum(
            bool(str(row.get("reasoning_content", "")).strip()) for row in rows
        ),
        "reasoning_content_empty": sum(
            not bool(str(row.get("reasoning_content", "")).strip()) for row in rows
        ),
        "reasoning_characters_total": sum(
            len(str(row.get("reasoning_content", ""))) for row in rows
        ),
        "reasoning_characters_mean": (
            statistics.fmean(
                len(str(row.get("reasoning_content", ""))) for row in rows
            )
            if rows
            else None
        ),
        "visible_response_empty": sum(
            not bool(str(row.get("raw_response", "")).strip()) for row in rows
        ),
        "visible_think_tag_count": sum(
            "<think>" in str(row.get("raw_response", "")).lower()
            or "</think>" in str(row.get("raw_response", "")).lower()
            for row in rows
        ),
        "system_fingerprints": fingerprints,
    }
    atomic_json(RUN_DIR / "summary.json", summary)
    try:
        atomic_json(RUN_DIR / "server_props_after.json", fetch_json(f"{ENDPOINT}/props"))
        atomic_json(RUN_DIR / "server_models_after.json", fetch_json(f"{ENDPOINT}/v1/models"))
    except Exception as exc:
        log(f"Could not save post-run server snapshot: {type(exc).__name__}: {exc}")
    build_heatmap(cell_rows)
    log(
        f"Summary: {total}/{expected} cases, numeric accuracy="
        f"{100 * summary['numeric_accuracy']:.2f}%" if total else "Summary: no completed cases"
    )


def self_test() -> None:
    cases = {
        0: "zero",
        11: "eleven",
        21: "twenty-one",
        105: "one hundred five",
        1_001: "one thousand one",
        93_847_696: "ninety-three million eight hundred forty-seven thousand six hundred ninety-six",
        19_999_999_999_998: (
            "nineteen trillion nine hundred ninety-nine billion nine hundred ninety-nine million "
            "nine hundred ninety-nine thousand nine hundred ninety-eight"
        ),
    }
    for number, expected in cases.items():
        actual = number_to_words(number)
        assert actual == expected, (number, actual, expected)
        parsed, error = parse_number_words(actual)
        assert error == "" and parsed == number, (number, parsed, error)
    parsed, error = parse_number_words(
        "Ninety-three million, eight hundred and forty-seven thousand, six hundred and ninety-six."
    )
    assert parsed == 93_847_696 and not error
    for invalid in [
        "The answer is eleven",
        "11",
        "one one",
        "one million two billion",
        "one hundred and",
        "twenty ten",
        "one hundred hundred",
        "one million million",
        "one and two",
        "one and thousand",
        "one and forty-two billion five hundred million",
        "one hundred and thousand",
        "zero one",
        "one thousand zero",
        "twenty trillion one",
        "",
    ]:
        parsed, _ = parse_number_words(invalid)
        assert parsed is None, (invalid, parsed)
    accepted_variants = {
        "one hundred and one": 101,
        "one thousand and one": 1001,
        "One million, two hundred thirty-four thousand, five hundred and six.": 1_234_506,
        "twenty trillion": 20_000_000_000_000,
    }
    for words, number in accepted_variants.items():
        parsed, error = parse_number_words(words)
        assert parsed == number and not error, (words, parsed, error)
    random_roundtrip = random.Random(8675309)
    for _ in range(10_000):
        number = random_roundtrip.randint(0, MAX_PARSED_NUMBER)
        parsed, error = parse_number_words(number_to_words(number))
        assert parsed == number and not error, (number, parsed, error)
    expected_cases = (DIGIT_MAX - DIGIT_MIN + 1) ** 2 * PAIRS_PER_CELL
    assert len(generate_pairs()) == expected_cases
    assert len({row["case_id"] for row in generate_pairs()}) == expected_cases
    print("self-test passed")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("prepare")
    subparsers.add_parser("smoke")
    run_parser = subparsers.add_parser("run")
    run_parser.add_argument(
        "--limit", type=int, default=None, help="Run only the first N cases (for diagnostics)"
    )
    subparsers.add_parser("summarize")
    subparsers.add_parser("regrade")
    subparsers.add_parser("self-test")
    args = parser.parse_args()

    if args.command == "prepare":
        prepare()
    elif args.command == "smoke":
        smoke()
    elif args.command == "run":
        run_benchmark(limit=args.limit)
    elif args.command == "summarize":
        summarize()
    elif args.command == "regrade":
        regrade_from_raw()
    elif args.command == "self-test":
        self_test()


if __name__ == "__main__":
    main()
