"""Evaluate a DatasetteAgent program (baseline or optimized) on the dataset.

Usage:
  python evaluate.py                     # baseline prompt, both splits
  python evaluate.py --program optimized_program.json
  python evaluate.py --split test
"""

import argparse
import json
from pathlib import Path

import dspy

from harness import DatasetteAgent, configure_lm, load_examples, metric

HERE = Path(__file__).parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--program", help="Saved program JSON to load")
    parser.add_argument("--split", choices=["train", "test", "both"], default="both")
    parser.add_argument("--model", default="openai/gpt-4.1-mini")
    parser.add_argument("--threads", type=int, default=8)
    parser.add_argument("--out", help="Write results JSON here")
    args = parser.parse_args()

    configure_lm(args.model)
    agent = DatasetteAgent()
    if args.program:
        agent.load(args.program)
        print(f"Loaded program from {args.program}")

    train, test = load_examples()
    splits = {"train": train, "test": test}
    results = {}
    for name in ("train", "test") if args.split == "both" else (args.split,):
        evaluator = dspy.Evaluate(
            devset=splits[name],
            metric=metric,
            num_threads=args.threads,
            display_progress=True,
            display_table=0,
            provide_traceback=True,
        )
        result = evaluator(agent)
        results[name] = result.score
        print(f"{name}: {result.score}")

    label = args.program or "baseline"
    print(json.dumps({"program": label, "model": args.model, "scores": results}))
    if args.out:
        Path(args.out).write_text(
            json.dumps(
                {"program": label, "model": args.model, "scores": results}, indent=2
            )
        )


if __name__ == "__main__":
    main()
