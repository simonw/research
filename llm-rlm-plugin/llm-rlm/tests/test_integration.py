"""
Integration test using gpt-5.2 to verify the full RLM pipeline.

Run with: uv run pytest tests/test_integration.py -v -s
Requires OPENAI_API_KEY in environment.
"""

import os
import random

import pytest

import llm
from llm_rlm import RLMToolbox


pytestmark = pytest.mark.skipif(
    not os.environ.get("OPENAI_API_KEY"),
    reason="OPENAI_API_KEY not set",
)


def generate_haystack(num_lines: int = 500, needle_line: int = None) -> tuple[str, str]:
    """Generate a large context with a hidden 'needle' fact."""
    if needle_line is None:
        needle_line = random.randint(100, num_lines - 100)

    secret_number = random.randint(10000, 99999)
    lines = []
    for i in range(num_lines):
        if i == needle_line:
            lines.append(f"Line {i}: The secret magic number is {secret_number}.")
        else:
            lines.append(f"Line {i}: This is filler text with random data value={random.randint(0, 10000)}.")

    return "\n".join(lines), str(secret_number)


def test_needle_in_haystack():
    """Test that gpt-5.2 can find a needle in a haystack using RLM tools."""
    context, expected_answer = generate_haystack(num_lines=500, needle_line=250)

    toolbox = RLMToolbox(context=context, sub_model="gpt-5.2")
    model = llm.get_model("gpt-5.2")

    chain_response = model.chain(
        "Find the secret magic number hidden in the context. "
        "The context is stored in the `context` variable in the Python sandbox. "
        "Use execute_python to search through it. "
        "When you find the number, use submit_answer to report it.",
        tools=[toolbox],
    )

    full_text = ""
    for response in chain_response.responses():
        full_text += response.text()

    assert expected_answer in full_text, (
        f"Expected '{expected_answer}' in response but got: {full_text[:500]}"
    )


def test_counting_task():
    """Test a simple counting/classification task typical of RLM benchmarks."""
    items = []
    cat_count = 0
    dog_count = 0
    for i in range(50):
        if random.random() < 0.4:
            items.append(f"Entry {i}: animal=cat, color={random.choice(['black', 'white', 'orange'])}")
            cat_count += 1
        else:
            items.append(f"Entry {i}: animal=dog, breed={random.choice(['labrador', 'poodle', 'beagle'])}")
            dog_count += 1

    context = "\n".join(items)
    toolbox = RLMToolbox(context=context, sub_model="gpt-5.2")
    model = llm.get_model("gpt-5.2")

    chain_response = model.chain(
        "Count how many entries have animal=cat in the context. "
        "The context is stored in the `context` variable in the Python sandbox. "
        "Use execute_python to search and count. "
        "Use submit_answer with the count as a number.",
        tools=[toolbox],
    )

    full_text = ""
    for response in chain_response.responses():
        full_text += response.text()

    assert str(cat_count) in full_text, (
        f"Expected cat count '{cat_count}' in response but got: {full_text[:500]}"
    )
