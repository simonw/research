"""Recompute report figures from the original binary and collapsed stacks."""

import argparse
import json
from collections import Counter
from pathlib import Path
from profiling.sampling.binary_reader import BinaryReader


def summarize(directory):
    with BinaryReader(str(directory / "web-edit.bin")) as reader:
        binary_info = reader.get_info()
    inclusive = Counter()
    direct = Counter()
    total = 0
    for row in (directory / "stacks.txt").read_text().splitlines():
        stack, value = row.rsplit(" ", 1)
        count = int(value)
        frames = stack.split(";")[1:]  # The first entry identifies the thread.
        functions = [frame.rsplit(":", 1)[0].partition(":")[2] for frame in frames]
        for function in set(functions):
            inclusive[function] += count
        direct[functions[-1]] += count
        total += count
    if total != binary_info["sample_count"]:
        raise ValueError("Binary and collapsed-stack sample counts differ")
    functions = (
        "WorkerThread.run",
        "FileStore.save_text",
        "preserve_endings",
        "SequenceMatcher.find_longest_match",
        "FileStore.browse",
    )
    return {
        "binary_info": binary_info,
        "recorded_stack_samples": total,
        "inclusive_functions": [
            {
                "function": name,
                "samples": inclusive[name],
                "percent": round(100 * inclusive[name] / total, 2),
            }
            for name in functions
        ],
        "top_leaf_functions": [
            {
                "function": name,
                "samples": count,
                "percent": round(100 * count / total, 2),
            }
            for name, count in direct.most_common(10)
        ],
        "interpretation": "Inclusive counts test whether a function occurs anywhere in each stack, at most once per stack. Inclusive rows overlap. Leaf counts combine source locations of the same function name; basename-only collapsed stacks can conflate names, so inspect the HTML source context.",
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "directory", type=Path, nargs="?", default=Path(__file__).parent / "artifacts"
    )
    args = parser.parse_args()
    print(json.dumps(summarize(args.directory), indent=2))
