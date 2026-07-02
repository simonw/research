"""Optimize the datasette-agent system prompt with dspy.GEPA.

GEPA (Genetic-Pareto) reflects on execution traces plus the metric's
textual feedback and proposes rewritten instructions for the program's
predictors. Here that means rewriting the datasette-agent system prompt
(carried as the ReAct signature instructions).

Usage:
  python optimize.py [--auto light] [--task-model openai/gpt-4.1-mini]
                     [--reflection-model openai/gpt-5-mini]
"""

import argparse
import json
from pathlib import Path

import dspy

from harness import (
    BASELINE_INSTRUCTIONS,
    DatasetteAgent,
    configure_lm,
    load_examples,
    metric,
)

HERE = Path(__file__).parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--auto", default="light", choices=["light", "medium", "heavy"])
    parser.add_argument("--task-model", default="openai/gpt-4.1-mini")
    parser.add_argument("--reflection-model", default="openai/gpt-5-mini")
    parser.add_argument("--out", default=str(HERE / "optimized_program.json"))
    args = parser.parse_args()

    configure_lm(args.task_model)
    reflection_lm = dspy.LM(
        args.reflection_model, temperature=1.0, max_tokens=32000
    )

    train, _test = load_examples()
    # GEPA uses trainset for rollouts and valset for candidate selection.
    # With only 20 training questions, reuse the full trainset as valset
    # rather than shrinking rollout data further; the real holdout is the
    # 10-question test split which GEPA never sees.
    optimizer = dspy.GEPA(
        metric=metric,
        auto=args.auto,
        reflection_lm=reflection_lm,
        num_threads=8,
        track_stats=True,
        log_dir=str(HERE / "gepa_logs"),
    )
    student = DatasetteAgent()
    optimized = optimizer.compile(student, trainset=train, valset=train)

    optimized.save(args.out)
    print(f"Saved optimized program to {args.out}")

    print("\n=== BASELINE INSTRUCTIONS ===\n")
    print(BASELINE_INSTRUCTIONS)
    for name, predictor in optimized.named_predictors():
        print(f"\n=== OPTIMIZED INSTRUCTIONS for {name} ===\n")
        print(predictor.signature.instructions)

    detailed = getattr(optimized, "detailed_results", None)
    if detailed is not None:
        summary = {
            "best_idx": getattr(detailed, "best_idx", None),
            "val_aggregate_scores": getattr(detailed, "val_aggregate_scores", None),
        }
        (HERE / "gepa_summary.json").write_text(
            json.dumps(summary, indent=2, default=str)
        )
        print(json.dumps(summary, default=str))


if __name__ == "__main__":
    main()
