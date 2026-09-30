# Implementation and run lineage

- Official runner SHA-256: `907e2b312b283b88624938c94c1ba7066f67cf5fb11951ef0262ffed24199e7a`
  - Archived as both `benchmark_inference.py` and `benchmark_postprocess.py`.
  - Used for preparation, reasoning-disabled smoke testing, all 5,070 official
    inference requests, regrading, summarization, and heatmap generation.
  - The two archived copies are byte-identical; post-run regrading changed no
    parsed answers or correctness/compliance flags.
- Official llama.cpp fingerprint: `b1-c398c6e`.
  - This is the same binary/build used by the Bonsai reference run.
  - Both official runs used one sequential slot and a 32,768-token context.
- Qwen model SHA-256: `e00082f779fa385cee8c68a3ec8833a75778cc87272240b942f74e0b8243e520`.
- Frozen pairs SHA-256: `a1bbd56ef958aa30c2d76c52d57bac43b4e0e96d43e75ae18f1110c82b4053aa`.
  - The Qwen and Bonsai `pairs.csv` files are byte-identical.
- Append-only attempts SHA-256: `98a929fe389f4565212e4b8ee268d749656de417d9b760f01db808a37ea78f2a`.
- Final results SHA-256: `be4c720fc9b1ec2d21621ed740d2fe9bd7720b83f04f4b46b47cb489870fb213`.

Before the official run, 67 clean requests were made with an older llama.cpp
build (`b1-0d9ceae`). That pilot was deliberately stopped so the official Qwen
and Bonsai comparisons could use the same server build. Its artifacts are
preserved under `runs/qwen3.8-27b-q4_k_m-pilot-0d9ceae-seed-20260929/` and are
excluded from all official summaries and comparisons.

The manifest's top-level `system_fingerprint` remained null because it was
written before inference. Every official response and the final summary record
the stable fingerprint `b1-c398c6e`; provenance is therefore retained.
