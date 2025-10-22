# Research

Each directory is a separate research project. Most of these will be produced using LLM tools.

## Research projects

<!--[[[cog
import os
import subprocess
import pathlib
from datetime import datetime

# Get all subdirectories with their first commit dates
research_dir = pathlib.Path.cwd()
subdirs_with_dates = []

for d in research_dir.iterdir():
    if d.is_dir() and not d.name.startswith('.'):
        # Get the date of the first commit that touched this directory
        try:
            result = subprocess.run(
                ['git', 'log', '--diff-filter=A', '--follow', '--format=%aI', '--reverse', '--', d.name],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0 and result.stdout.strip():
                # Parse first line (oldest commit)
                date_str = result.stdout.strip().split('\n')[0]
                commit_date = datetime.fromisoformat(date_str.replace('Z', '+00:00'))
                subdirs_with_dates.append((d.name, commit_date))
            else:
                # No git history, use directory modification time
                subdirs_with_dates.append((d.name, datetime.fromtimestamp(d.stat().st_mtime)))
        except Exception:
            # Fallback to directory modification time
            subdirs_with_dates.append((d.name, datetime.fromtimestamp(d.stat().st_mtime)))

# Sort by date, most recent first
subdirs_with_dates.sort(key=lambda x: x[1], reverse=True)

for dirname, commit_date in subdirs_with_dates:
    folder_path = research_dir / dirname
    readme_path = folder_path / "README.md"
    summary_path = folder_path / "_summary.md"

    date_formatted = commit_date.strftime('%Y-%m-%d')

    # Get GitHub repo URL
    github_url = None
    try:
        result = subprocess.run(
            ['git', 'remote', 'get-url', 'origin'],
            capture_output=True,
            text=True,
            timeout=2
        )
        if result.returncode == 0 and result.stdout.strip():
            origin = result.stdout.strip()
            # Convert SSH URL to HTTPS URL for GitHub
            if origin.startswith('git@github.com:'):
                origin = origin.replace('git@github.com:', 'https://github.com/')
            if origin.endswith('.git'):
                origin = origin[:-4]
            github_url = f"{origin}/tree/main/{dirname}"
    except Exception:
        pass

    if github_url:
        print(f"### [{dirname}]({github_url}) ({date_formatted})\n")
    else:
        print(f"### {dirname} ({date_formatted})\n")

    # Check if summary already exists
    if summary_path.exists():
        # Use cached summary
        with open(summary_path, 'r') as f:
            description = f.read().strip()
            if description:
                print(description)
            else:
                print("*No description available.*")
    elif readme_path.exists():
        # Generate new summary using llm command
        prompt = """Summarize this research project concisely. Write just 1 paragraph (3-5 sentences) followed by an optional short bullet list if there are key findings. Vary your opening - don't start with "This report" or "This research". Include 1-2 links to key tools/projects. Be specific but brief. No emoji."""
        result = subprocess.run(
            ['llm', '-m', 'github/gpt-5-mini', '-s', prompt],
            stdin=open(readme_path),
            capture_output=True,
            text=True,
            timeout=60
        )
        if result.returncode != 0:
            error_msg = f"LLM command failed for {dirname} with return code {result.returncode}"
            if result.stderr:
                error_msg += f"\nStderr: {result.stderr}"
            raise RuntimeError(error_msg)
        if result.stdout.strip():
            description = result.stdout.strip()
            print(description)
            # Save to cache file
            with open(summary_path, 'w') as f:
                f.write(description + '\n')
        else:
            raise RuntimeError(f"LLM command returned no output for {dirname}")
    else:
        print("*No description available.*")

    print()  # Add blank line between entries

]]]-->
### [datasette-plugin-alpha-versions](https://github.com/simonw/research/tree/main/datasette-plugin-alpha-versions) (2025-10-20)

A complete audit of 44 Datasette plugins (from 213 repos cloned across the datasette org and simonw) evaluated version constraints, hook usage, and release tags to identify which plugins need updating for a stable Datasette 1.0. The analysis shows 39 of the 44 plugins depend on Datasette ALPHA releases, 16 use register_permissions(), and only 8 plugins have at least one stable release while the rest are alpha-only or unreleased. Results and per-plugin metadata are available in datasette_plugins_analysis.json and were produced by the analyze_with_releases.py script; key references: https://datasette.io and https://github.com/simonw. Use the JSON and scripts to prioritize plugin updates and track permission-hook compatibility.

- Total repositories cloned: 213; plugins analyzed in depth: 44  
- ALPHA-dependent plugins: 39 (34 alpha-only)  
- register_permissions() usage: 16 (5 without ALPHA dependency)  
- Plugins with stable releases: 8

### [deepseek-ocr-nvidia-spark](https://github.com/simonw/research/tree/main/deepseek-ocr-nvidia-spark) (2025-10-20)

Successfully deployed DeepSeek-OCR on an NVIDIA GB10 (ARM64, sm_121) by upgrading to PyTorch 2.9.0+cu130 so CUDA 13.0 wheels could be used instead of building from source. The repo includes automated scripts (setup.sh, run_ocr.py) that load the 6.3GB safetensors model (~34s) and run GPU inference (~58s for a 3503×1668 image), producing annotated images, markdown/text outputs and bounding boxes with validated multi-column accuracy. Flash-attn failed to compile on ARM64 and the pipeline falls back to eager attention, but overall accuracy and production readiness were confirmed. Reproducible instructions, logs and scripts are provided in the DeepSeek-OCR repo and the PyTorch cu130 wheel index linked below.  

- Key findings: PyTorch 2.9.0+cu130 provides forward compatibility for sm_121 (no source build needed).  
- Performance: model load ≈34s, inference ≈58s; detected 2257 text tokens / 921 vision tokens.  
- Artifacts & links: DeepSeek-OCR code/model (https://github.com/deepseek-ai/DeepSeek-OCR) and PyTorch cu130 wheel index (https://download.pytorch.org/whl/cu130).

### [sqlite-permissions-poc](https://github.com/simonw/research/tree/main/sqlite-permissions-poc) (2025-10-20)

A proof-of-concept implements a fully SQLite-based hierarchical permission system that computes allowed database/table pairs by cascading rules across child (table), parent (database), and global levels with DENY-over-ALLOW semantics; it uses only plain SQL (CTEs + SQLite JSON functions) and is built on SQLite (https://sqlite.org). Actor and token inputs are JSON-parsed inside the query so a single CTE-based SQL statement resolves per-resource decisions (child → parent → global) and then intersects results with optional token scope, ensuring tokens can only restrict, not grant, access; behavior is validated with a pytest test suite (https://pytest.org). The demo includes a minimal schema, multiple simulated “hook” rule sources, example data, and 11 test scenarios that show child-level ALLOW overriding parent DENY, child-level DENY blocking parent ALLOW, default-deny behavior, and token intersection semantics.

Key findings:
- Pure-SQL implementation (no UDFs/extensions) using CTEs and sqlite JSON helpers.
- Cascading precedence: child > parent > global; at the same level DENY beats ALLOW.
- Token scoping applied via INTERSECT; tokens cannot elevate permissions.
- Single-query engine returns final db/table pairs; schema and tests are compact and extensible.
- 11 pytest scenarios confirm intended conflict-resolution rules and edge cases.

### [minijinja-vs-jinja2](https://github.com/simonw/research/tree/main/minijinja-vs-jinja2) (2025-10-19)

Benchmarking the Python bindings for minijinja (https://github.com/mitsuhiko/minijinja) against Jinja2 (https://palletsprojects.com/p/jinja/) on Python 3.14 and 3.14t measured template render performance using a realistic e-commerce template with inheritance, loops, and ~65KB HTML output. The suite runs 200 iterations per scenario, captures mean/median/std/min/max, and provides reproducible scripts (run_benchmark.sh, benchmark.py) plus matplotlib charts to visualize results. Jinja2 is faster on stock Python 3.14, while minijinja gains more from the free-threaded 3.14t build, indicating minijinja may be better positioned for free-threaded Python even though it’s currently slower in absolute terms. Everything needed to reproduce the 15–20 minute benchmark and view detailed analysis is included in the repository.

- Jinja2 (3.14): 0.990 ms mean vs Minijinja: 1.528 ms mean — Jinja2 ≈ 1.54× faster on 3.14  
- Jinja2 slows ~14% on 3.14t (1.127 ms); Minijinja speeds up ~13% on 3.14t (1.336 ms)  
- Artifacts: JSON results, comparison/distribution/speedup/timeline charts, and BENCHMARK_RESULTS.md with full analysis

### [node-pyodide](https://github.com/simonw/research/tree/main/node-pyodide) (2025-10-19)

A compact demo shows how to run Python scripts inside a WebAssembly sandbox from Node.js using Pyodide: after npm install, launching node server-simple.js executes example-simple.py and writes generated files to the output/ directory. The project demonstrates a minimal server-side integration pattern for Pyodide (https://pyodide.org/) under Node.js (https://nodejs.org/) and is aimed at quick experimentation with sandboxed Python execution. It requires Node.js v16 or later and provides a simple starting point for extending Python-in-WASM workflows in Node applications.

- Executes Python in WebAssembly via Pyodide and writes outputs to output/
- Minimal commands: npm install; node server-simple.js
- Recommended Node.js v16+ for best compatibility

<!--[[[end]]]-->

---

## Updating this README

This README uses [cogapp](https://nedbatchelder.com/code/cog/) to automatically generate project descriptions.

### Automatic updates

A GitHub Action automatically runs `cog -r -P README.md` on every push to main and commits any changes to the README or new `_summary.md` files.

### Manual updates

To update locally:

```bash
# Run cogapp to regenerate the project list
cog -r -P README.md
```

The script automatically:
- Discovers all subdirectories in this folder
- Gets the first commit date for each folder and sorts by most recent first
- For each folder, checks if a `_summary.md` file exists
- If the summary exists, it uses the cached version
- If not, it generates a new summary using `llm -m github/gpt-5-mini` with a prompt that creates engaging descriptions with bullets and links
- Creates markdown links to each project folder on GitHub
- New summaries are saved to `_summary.md` to avoid regenerating them on every run

To regenerate a specific project's description, delete its `_summary.md` file and run `cog -r -P README.md` again.
