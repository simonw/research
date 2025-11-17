# Research projects carried out by AI tools

Each directory in this repo is a separate research project carried out by an LLM tool - usually [Claude Code](https://www.claude.com/product/claude-code). Every single line of text and code was written by an LLM.

I try to include prompts and links to transcripts in [the PRs](https://github.com/simonw/research/pulls?q=is%3Apr+is%3Aclosed) that added each report, or in [the commits](https://github.com/simonw/research/commits/main/).

<!--[[[cog
import os
import subprocess
import pathlib
from datetime import datetime, timezone

# Model to use for generating summaries
MODEL = "github/gpt-4.1"

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
                subdirs_with_dates.append((d.name, datetime.fromtimestamp(d.stat().st_mtime, tz=timezone.utc)))
        except Exception:
            # Fallback to directory modification time
            subdirs_with_dates.append((d.name, datetime.fromtimestamp(d.stat().st_mtime, tz=timezone.utc)))

# Print the heading with count
print(f"## {len(subdirs_with_dates)} research projects\n")

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
            ['llm', '-m', MODEL, '-s', prompt],
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
## 21 research projects

### [svg-to-png-renderer](https://github.com/simonw/research/tree/main/svg-to-png-renderer) (2025-11-17)

A lightweight SVG to PNG renderer has been developed using Python, leveraging the `xml.etree.ElementTree` and `Pillow` libraries to parse SVG XML data and convert it to raster PNG images. This minimal library supports a range of SVG elements, including paths, basic shapes, and containers, as well as attributes such as colors, styling, and transforms. The renderer can be used as a command-line tool or imported as a library, and has been tested with complex SVG files, including the "Ghostscript Tiger" SVG. For more information on the project, see the [Pillow](https://pillow.readthedocs.io/) documentation or the [SVG specification](https://www.w3.org/TR/SVG2/).

* Key features:
  * Support for SVG paths, basic shapes, and containers
  * Support for colors, styling, and transforms
  * Can be used as a command-line tool or imported as a library
  * Tested with complex SVG files, including the "Ghostscript Tiger" SVG

### [svg-to-png-comparison](https://github.com/simonw/research/tree/main/svg-to-png-comparison) (2025-11-17)

The conversion of SVG files to PNG format in Python can be achieved through various methods

### [absurd-in-sqlite](https://github.com/simonw/research/tree/main/absurd-in-sqlite) (2025-11-12)

Durable execution workflows can be implemented using SQLite, as demonstrated by the Absurd-in-SQLite project, which is inspired by Armin Ronacher's Absurd. This project provides a proof-of-concept implementation of durable execution using SQLite, allowing for reliable and long-running workflows that can survive crashes and network failures. The project utilizes a pull-based model, where workers pull tasks from a queue, and features a replay model that replays the entire function from the beginning when a task resumes. For more information, visit the [Absurd](https://github.com/earendil-works/absurd) and [Absurd Workflows](https://lucumr.pocoo.org/2025/11/3/absurd-workflows/) resources.

* Key findings:
  * Durable execution can be achieved using a database like SQLite
  * The replay model allows for reliable execution of tasks
  * A pull-based model can simplify workflow management
  * SQLite can handle moderate workloads, but Postgres may be more suitable for high-concurrency scenarios

### [yt-dlp-install-report](https://github.com/simonw/research/tree/main/yt-dlp-install-report) (2025-11-12)

A detailed analysis of installing `yt-dlp[default]` via pip on Linux with Python 3.11 reveals that the process brings in six new packages totaling about 39 MB and over 3,000 files, including 44 binary libraries (mainly for cryptography and compression) consuming 8.55 MB. The main package, yt-dlp, is a feature-rich video downloader whose full capabilities rely on its optional dependencies, enabled by the `[default]` extra: Brotli (compression), pycryptodomex (cryptography), websockets (live streaming), mutagen (metadata), and yt-dlp-ejs (JavaScript extractors). The installation is dominated by Python source and bytecode files, with binaries used for performance-critical tasks; all binaries are standard Linux ELF shared objects with typical system dependencies. For downloading encrypted content, handling compression, live streams, or audio metadata, installing with `[default]` is recommended.  
Key tools: [yt-dlp](https://github.com/yt-dlp/yt-dlp), [pycryptodomex](https://www.pycryptodome.org/)

**Key findings:**
- 6 packages installed, major size contributor is yt-dlp itself (~22 MB).
- 44 compiled .so binaries (mostly for crypto/compression, not full executables).
- `[default]` extra adds significant functionality for encrypted, compressed, live, and tagged media.
- Binaries built for standard x86-64 Linux, depending only on common system libraries.

### [uv-run-flow-analysis](https://github.com/simonw/research/tree/main/uv-run-flow-analysis) (2025-11-10)

Running `uv run myscript.py` in a directory with a `pyproject.toml` launches a multi-phase workflow that automates Python script execution within an isolated, dependency-managed environment. uv scans for project metadata, resolves and validates interpreter and package requirements, manages virtual environments, locks dependencies with a TOML-based `uv.lock` file using the PubGrub algorithm, efficiently syncs the environment with parallel downloads and caching, and finally executes the desired command with robust error handling. This process is orchestrated via performant Rust crates, resulting in fast, reliable, and reproducible Python executions superior to traditional tools like pip or poetry. For more details on the tool, see [uv documentation](https://github.com/astral-sh/uv) or the PubGrub [resolution algorithm](https://docs.rs/pubgrub/latest/pubgrub/).

Key findings:
- uv automatically discovers and parses project dependencies from `pyproject.toml`, supporting PEP standards and custom configurations.
- Dependency resolution is lock-file-driven (universal, reproducible) and faster than pip/poetry due to Rust implementation, PubGrub, and aggressive caching.
- Python interpreter management is integrated, with automatic downloads and validation against project constraints.
- Installation of packages occurs in a virtual environment, with fine-grained error handling and atomic operations.
- The architecture enables cross-platform consistency, incremental sync, and seamless user experience with zero configuration required.

### [env86-analysis](https://github.com/simonw/research/tree/main/env86-analysis) (2025-11-09)

env86 is a Go-based management tool that enables users to run x86 Linux virtual machines within browser contexts via the v86 WebAssembly emulator. By combining a native desktop application (embedding a browser), a robust CLI, and an integrated virtual networking stack, env86 provides an easily distributable and reproducible Linux environment that can boot instantly from snapshots, support host-guest communication, and mount host filesystems. Images are efficiently distributed through GitHub releases, and the system can be used interactively or in headless/automation contexts, making it especially suitable for development, education, sandboxing, legacy software execution, and rapid demonstration scenarios. While performance is limited by browser-based emulation, env86 uniquely excels in cross-platform portability and accessibility, allowing VMs to run anywhere a browser or desktop is available.

Key findings/features:
- Instant VM boot from state snapshots, dramatically reducing startup time.
- Secure host-guest communication using CBOR RPC over serial ports.
- User-space networking via [go-netstack](https://github.com/progrium/go-netstack) and transparent port forwarding.
- 9P protocol-based filesystem mounting for host directory access.
- Image distribution and updates handled via GitHub releases, supporting reproducible environments.
- Desktop integration via [tractor.dev/toolkit-go](https://github.com/tractor-dev/toolkit-go) enables GUI and CLI/CI workflows.

See the env86 repo for details: https://github.com/progrium/env86  
Learn more about the v86 emulator: https://github.com/copy/v86

### [llm-pyodide-openai-plugin](https://github.com/simonw/research/tree/main/llm-pyodide-openai-plugin) (2025-11-09)

Leveraging the [LLM Python package](https://llm.datasette.io/) and pyodide, this project successfully adapts LLM’s OpenAI model interface for direct use in browser environments by bypassing the standard openai library (which fails in browsers due to its httpx dependency) and instead using the browser-native fetch API for CORS-compliant API calls. The plugin implements the LLM `KeyModel` interface and registers new models with OpenAI support through custom hooks, allowing prompt execution and chat completions entirely within pyodide’s async event loop, without server-side Python. No changes to LLM’s core were required; all adaptations reside in the plugin, which integrates cleanly with the browser’s JS-Python bridge and achieves dynamic model registration, API calls, and response parsing directly in the browser. For reference, the core plugin implementation is contained in `llm_pyodide_openai.py` while [pyodide](https://pyodide.org/en/stable/) provides the Python-in-browser runtime.

**Key findings:**
- The openai package installs in pyodide but is incompatible with browser HTTP requests, necessitating direct fetch API integration.
- All plugin logic, including API calls and response parsing, operates in Python via pyodide, using JavaScript interop for networking.
- No modifications to the LLM core package are required; plugin flexibility is sufficient for adapting to browser constraints.
- Streaming (partial responses) is not yet implemented, but could be achieved with ReadableStream and async generators.
- CORS restrictions are resolved naturally by using fetch, making production-grade browser-based AI agents feasible.

### [codex-sandbox-investigation](https://github.com/simonw/research/tree/main/codex-sandbox-investigation) (2025-11-09)

OpenAI Codex CLI's sandbox employs strong, platform-specific isolation to securely constrain the behavior of AI-driven code agents. On macOS, it uses Apple's Seatbelt sandbox with finely tuned dynamic policies, while on Linux, it combines Landlock for strict filesystem controls and seccomp for syscall-based network blocking—ensuring that agents can only write to user-approved directories and have no outgoing network by default. Both platforms feature special protection for `.git` repositories, path canonicalization to thwart symlink attacks, and enforce least-privilege principles, all integrated with user-configurable approval policies for flexibility. Key tools include the [OpenAI Codex CLI](https://github.com/openai/codex) and related [sandbox documentation](https://github.com/openai/codex/blob/main/docs/sandbox.md).

**Key findings:**
- Codex's sandbox distinguishes between DangerFullAccess (no sandbox), ReadOnly, and WorkspaceWrite modes, tightly controlling file/network access.
- Version control metadata (e.g., `.git`) is always read-only, preventing AI from corrupting repositories.
- Linux sandboxing requires kernel 5.13+ for Landlock; macOS relies on built-in Seatbelt.
- All attempted network operations are blocked at the syscall level, except for local IPC.
- Windows support is experimental; robust isolation is limited outside macOS/Linux.

### [sqlite-query-linter](https://github.com/simonw/research/tree/main/sqlite-query-linter) (2025-11-04)

The SQLite Query Linter is a lightweight Python library that wraps the standard `sqlite3` module to provide configurable linting and rule-based analysis of SQL queries before execution. Acting as a drop-in replacement, it helps catch common syntax errors and platform incompatibilities—such as invalid types in `CAST`, use of unsupported functions, `SELECT *`, missing `WHERE` clauses, and string quoting mistakes—helping developers avoid runtime errors and improve code quality. Users can choose built-in rules, set severity levels, and easily define custom rules via an extensible API. Designed for flexibility, it can block execution on critical issues or run in permissive/audit-only modes, with zero dependencies other than Python's standard library. Explore code and integration options at [GitHub](https://github.com/yourusername/sqlite-query-linter) or view usage in the included [`demo.py`](demo.py) script.

Key Features & Findings:
- Detects SQL mistakes commonly encountered when migrating between databases or writing raw SQLite queries
- Flexible configuration: Enable/disable rules, set strictness, and use audit-only monitoring
- Easy to extend for custom organizational or project rules
- Applicable to development, automated testing, database migrations, and production monitoring

### [h3-library-benchmark](https://github.com/simonw/research/tree/main/h3-library-benchmark) (2025-11-04)

A systematic performance benchmark was conducted on two prominent Python libraries implementing Uber's H3 geospatial indexing system: [h3-py](https://github.com/uber/h3-py) (official, C-based) and [h3o-python](https://github.com/HydroniumLabs/h3o) (Rust-based). Results show h3o-python consistently outperforms h3-py on core operations, achieving over 2x speedup for coordinate conversions and up to 13x faster neighbor queries, while area calculations remain comparable. The performance advantage holds steady across varied dataset sizes and H3 resolutions, suggesting h3o-python's Rust backend is highly optimized for geospatial workloads. Differences in API coverage and cell representation (string vs. integer) should inform choice based on project requirements.

**Key Findings:**
- h3o-python is 2.2x faster for coordinate-to-cell and 1.8–2x for cell-to-coordinate conversions.
- Neighbor queries with grid_disk are 10–13x faster in h3o-python.
- Both libraries perform similarly for cell area calculations.
- h3-py offers more features and broader API support; h3o-python excels in raw speed for core operations.

### [h3o-python](https://github.com/simonw/research/tree/main/h3o-python) (2025-11-03)

h3o-python delivers efficient Python bindings for the [h3o](https://github.com/HydroniumLabs/h3o) Rust library, enabling fast and convenient access to H3 geospatial indexing from Python. Utilizing [PyO3](https://pyo3.rs/) and packaged with maturin, it allows encoding geographic coordinates into 64-bit H3 cell indexes, decoding indexes, performing neighborhood queries, calculating great-circle distances, and retrieving surface area metrics—all without requiring a separate H3 installation. The module bundles its Rust extension in the distributable wheel for seamless deployment, and the API mirrors the upstream Rust crate for high performance and compatibility.

**Key capabilities:**
- Simple conversion between latitude/longitude and H3 cell indexes
- Neighborhood and adjacency checks, and disk queries
- Accurate area and distance calculations using H3 algorithms
- Lossless string/integer conversions of H3 indexes

### [wazero-python-claude](https://github.com/simonw/research/tree/main/wazero-python-claude) (2025-11-02)

Wazero Python Bindings enable seamless integration of the [wazero](https://wazero.io/) WebAssembly runtime—written in Go—with Python applications, delivering a zero-dependency solution for running WASM modules natively from Python. The project exposes a clean, Pythonic API for instantiating modules, calling exported WASM functions, and managing resources efficiently with context managers. Performance benchmarks demonstrate rapid execution and minimal overhead between Python and WASM. While the library excels at speed and ease of use, current limitations include support only for integer argument and return types, restricted WASI features, and lack of direct memory access.

Key findings:
- Near-native performance for compute-intensive WebAssembly code via [wazero](https://github.com/tetratelabs/wazero).
- Simple Python interface with automatic resource management and no external dependencies.
- Presently limited to i32/i64 arguments/results and basic WASM module features; WASI filesystem and direct memory access are not available yet.

### [datasette-plugin-skill](https://github.com/simonw/research/tree/main/datasette-plugin-skill) (2025-10-24)

Covering every aspect of Datasette plugin development, this project creates a comprehensive skill set for authors—from bootstrapping with cookiecutter to deploying on GitHub and PyPI. It provides precise guides and working code samples for essential plugin hooks like custom SQL functions, authentication, custom views, and output formats. The resource includes an extensive API reference, best practices for configuration, static assets, and templates, plus testing and publishing workflows to ensure reliable plugins. Developers can use this to rapidly build a variety of plugins—custom SQL, visualizations, authentication handlers, data exporters, and more.

Key tools/projects:
- [Datasette documentation](https://docs.datasette.io/)
- [cookiecutter plugin template](https://github.com/simonw/datasette-plugin)

Key findings:
- Covers both sync and async hook design for performance.
- Explains complete request/response and database APIs.
- Provides tested patterns for authentication, authorization, routing, and output customization.

### [blog-tags-scikit-learn](https://github.com/simonw/research/tree/main/blog-tags-scikit-learn) (2025-10-24)

Automatically assigning meaningful tags to historic, untagged blog posts, this project leverages the [Simon Willison blog database](https://datasette.simonwillison.net/simonwillisonblog.db) and scikit-learn to train and compare multi-label text classification models. Four approaches—TF-IDF + Logistic Regression, Multinomial Naive Bayes, Random Forest, and LinearSVC—were tested on posts’ title and body text using the 158 most frequently used tags. LinearSVC, with probability calibration, yielded the best overall performance, striking a balance between precision (85%) and recall (56%) with an F1 score of 68%, proving especially effective for assigning multiple tags to each entry. This [open-source toolkit](https://scikit-learn.org/) not only automates metadata enrichment but facilitates rapid quality assessment and scalable tag prediction for content libraries.

**Key findings:**
- LinearSVC outperformed other models, delivering the highest F1 score (0.6791) and recall.
- Logistic Regression and Random Forest prioritized precision but were more conservative—missing more actual tags.
- Naive Bayes offered a fast, simple solution with a solid balance of metrics.
- TF-IDF features and OneVsRest multi-label strategies proved robust for text classification in high-dimensional spaces.

### [cmarkgfm-in-pyodide](https://github.com/simonw/research/tree/main/cmarkgfm-in-pyodide) (2025-10-22)

By rewriting cmarkgfm's bindings from CFFI to the Python C API, the project successfully ported GitHub's cmark-gfm Markdown parser to Pyodide. The resulting wheel is fully functional, requires no further building, and supports all GitHub Flavored Markdown features with high performance, thanks to direct C code execution via WebAssembly. Users can integrate the package into Pyodide (see [Pyodide documentation](https://pyodide.org/)) and render robust Markdown—including tables, strikethrough, and task lists—directly in the browser. This port demonstrates a practical technique for bringing other CFFI-based packages to WebAssembly/Pyodide environments.

**Key Findings:**
- All GFM features (tables, strikethrough, smart typography, etc.) work accurately.
- Integration and pytest test suites pass 100%.
- The port uses only Python C API bindings, improving compatibility and speed.
- [Project source & wheel available](https://github.com/github/cmark-gfm).

### [python-markdown-comparison](https://github.com/simonw/research/tree/main/python-markdown-comparison) (2025-10-22)

Comparing seven prominent Python markdown libraries, cmarkgfm—bindings to GitHub’s C-based CommonMark/GFM parser—proved dramatically faster (10-50x) than pure Python options such as mistune, Python-Markdown, and marko. The benchmark, spanning small to large markdown documents, consistently found cmarkgfm excels in both speed and stability, making it ideal for high-volume or performance-critical applications. However, cmarkgfm trades extensibility and custom output formats for speed, so libraries like mistune (for fast pure Python and custom rendering) or Python-Markdown (for extension-rich configurability) may be preferable for projects prioritizing flexibility or ease of customization. See [cmarkgfm's repository](https://github.com/theacodes/cmarkgfm) and [mistune](https://github.com/lepture/mistune) for details.

**Key findings:**
- cmarkgfm is 10-50x faster than pure Python markdown libraries, especially for large documents.
- Pure Python options offer greater extensibility, custom output formats, and API access, but at the cost of speed.
- Best library choice depends on project needs: cmarkgfm for raw speed/GFM compatibility, mistune for pure Python speed/customization, Python-Markdown for plugins/extensions.

### [datasette-plugin-alpha-versions](https://github.com/simonw/research/tree/main/datasette-plugin-alpha-versions) (2025-10-20)

Datasette Plugins Analysis presents a systematic evaluation of 44 key plugins from the Datasette ecosystem, focusing on dependencies, permissions hooks, and release patterns as of October 2025. The study finds that 89% of these plugins rely on ALPHA versions of Datasette, with only 8 plugins having stable releases and just 5 supporting stable Datasette while using advanced hooks like `register_permissions()`. The open datasets, such as [`datasette_plugins_analysis.json`](https://github.com/simonw/datasette-plugins-analysis/blob/main/datasette_plugins_analysis.json) and analysis scripts, support deeper inspection and maintenance planning as Datasette nears its 1.0 milestone. This enables maintainers to prioritize updates for plugins with alpha dependencies and track release maturity across the ecosystem.

**Key Findings:**
- 39 plugins depend on Datasette ALPHA versions; 34 of these have no stable releases.
- Only 5 plugins use `register_permissions()` without requiring ALPHA Datasette.
- 8 of the analyzed plugins currently offer at least one stable release.  
- Main analysis and scripts are available [here](https://github.com/simonw/datasette-plugins-analysis) for further plugin and dependency tracking.

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
- If not, it generates a new summary using `llm -m <!--[[[cog
print(MODEL, end='')
]]]-->
github/Llama-3.3-70B-Instruct
<!--[[[end]]]-->` with a prompt that creates engaging descriptions with bullets and links
- Creates markdown links to each project folder on GitHub
- New summaries are saved to `_summary.md` to avoid regenerating them on every run

To regenerate a specific project's description, delete its `_summary.md` file and run `cog -r -P README.md` again.
