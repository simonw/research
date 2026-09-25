# Claude Code Setup Analysis: How This Research Environment Works

## Executive Summary

This report analyzes how Claude Code successfully operates in Simon Willison's research repository environment and identifies promising research projects that can be conducted in this setup. The analysis reveals a thoughtfully designed system that leverages AI agents' strengths while providing clear structure, validation mechanisms, and automation.

## Question 1: How Claude Code Works Successfully in This Environment

### The Core Setup

The research repository (https://github.com/simonw/research) is specifically designed for AI-driven research projects. Every subdirectory contains a complete investigation carried out by LLMs, primarily Claude Code, with all text and code written by AI.

### Success Factor #1: Clear Structural Guidance

**AGENTS.md Provides Explicit Instructions:**
- Create a new folder with an appropriate name
- Maintain notes.md tracking investigation progress
- Build README.md report at the end
- Include only created/modified files, not full external codebases
- Save git diffs instead of complete cloned repositories
- Binary files limited to < 2MB

This structure eliminates ambiguity about deliverables and workflow.

### Success Factor #2: Validation Through Code Execution

Simon Willison's key insight from his article:

> "LLMs hallucinate and make mistakes. This is far less important for code research tasks because the code itself doesn't lie: if they write code and execute it and it does the right things then they've demonstrated to both themselves and to you that something really does work."

Research projects involve:
- Running benchmarks with measurable results
- Cloning and analyzing real codebases
- Installing and testing libraries
- Generating data files and charts
- Executing proofs-of-concept

Code either works or throws errors - immediate, objective feedback that prevents hallucination.

### Success Factor #3: Isolated, Permissive Environment

**Full Trust Model:**
- Dedicated repository for research only
- No production code to break
- Full network access for cloning repos, downloading packages, web searches
- Permission to install dependencies via pip, npm, etc.
- Ability to run arbitrary commands for experimentation

This eliminates the cautious behavior needed in production codebases.

### Success Factor #4: Self-Documenting Workflow

**notes.md Pattern:**
Each research project includes notes.md showing:
- Investigation objectives
- Step-by-step discoveries
- What was tried and whether it worked
- Code locations and key findings
- Decision points and reasoning

This creates an audit trail and helps the agent maintain context through long investigations.

### Success Factor #5: Automation Reduces Friction

**GitHub Actions Workflow (.github/workflows/update-readme.yml):**
1. Triggers on push to main
2. Runs cogapp to regenerate README.md index
3. Uses LLM (github/gpt-4.1) to generate project summaries
4. Auto-commits _summary.md files
5. Commits with `[skip ci]` to prevent infinite loops

**Cogapp Integration:**
- README.md contains embedded Python code in cog blocks
- Discovers all project folders
- Extracts git commit dates
- Generates or reuses cached summaries
- Creates GitHub links automatically

This means the research agent never needs to manually update the index or write summaries.

### Success Factor #6: Appropriate Task Scope

Research projects are:
- **Time-boxed investigations** (not ongoing development)
- **Focused questions** with clear endpoints
- **Self-contained** (each folder is independent)
- **Verifiable** (results can be reproduced)

Examples:
- "How does uv resolve dependencies?" (codebase analysis)
- "Which Python markdown library is fastest?" (benchmark)
- "Can cmarkgfm work in Pyodide?" (proof-of-concept)

### Success Factor #7: Essential Tooling

**requirements.txt includes:**
- **cogapp**: Template-based code generation for README automation
- **llm**: CLI access to LLMs for generating summaries
- **llm-github-models**: Integration with GitHub's model API

These tools enable the automation and self-service AI summaries.

### The Async Agent Model

From Simon's article:

> "Async coding agents operate on a 'fire-and-forget' basis. You pose a research question, the agent churns away on a server, and files a pull request when finished."

Claude Code in this environment:
- Works autonomously on feature branches
- Has full bash/git/network access
- Creates complete research reports
- Files PRs when done
- Minimal human supervision required

Simon reports: "firing off 2-3 research projects daily with minimal personal involvement."

## Question 2: Promising Research Project Ideas

Based on analyzing the 19 existing research projects, I've identified patterns and opportunities:

### Category 1: Tool/Library Comparisons & Benchmarks

**Existing examples:** h3-library-benchmark, python-markdown-comparison, minijinja-vs-jinja2

**New project ideas:**

1. **Python HTTP Client Libraries Comparison**
   - Compare requests, httpx, aiohttp, urllib3
   - Benchmark sync vs async performance
   - Feature matrix (HTTP/2, connection pooling, timeouts)
   - Migration guides between libraries

2. **Python ASGI/WSGI Server Benchmarks**
   - Gunicorn vs Uvicorn vs Hypercorn vs Daphne
   - Load testing with realistic workloads
   - Memory usage and startup time
   - WebSocket support comparison

3. **Python Testing Framework Deep Comparison**
   - pytest vs unittest vs nose2 vs ward
   - Fixture systems comparison
   - Plugin ecosystems
   - Performance with large test suites

4. **JSON Parsing Library Performance**
   - json vs orjson vs ujson vs rapidjson
   - Benchmark serialization/deserialization
   - Memory efficiency
   - Compatibility edge cases

### Category 2: Deep Dive Codebase Analysis

**Existing examples:** uv-run-flow-analysis, env86-analysis, codex-sandbox-investigation

**New project ideas:**

5. **How pytest Discovery and Collection Works**
   - Trace through pytest's test discovery algorithm
   - Understand conftest.py loading
   - Fixture resolution internals
   - Plugin hook execution order

6. **pip vs uv Dependency Resolution Comparison**
   - Compare resolution algorithms (pip's vs uv's PubGrub)
   - Lock file format analysis
   - Performance benchmarks
   - Edge case handling

7. **Django ORM Query Builder Internals**
   - How QuerySets build SQL
   - Lazy evaluation mechanism
   - Join optimization strategies
   - Annotation and aggregation internals

8. **How Black Formats Python Code**
   - AST parsing and manipulation
   - Formatting rules engine
   - Line length optimization
   - Comment preservation logic

9. **FastAPI Dependency Injection Deep Dive**
   - How dependencies are discovered
   - Async dependency resolution
   - Caching mechanisms
   - Validation integration with Pydantic

### Category 3: Proof-of-Concept Implementations

**Existing examples:** sqlite-query-linter, sqlite-permissions-poc, llm-pyodide-openai-plugin

**New project ideas:**

10. **Git Conflict Resolution Assistant**
    - Parse git conflict markers
    - Use LLM to suggest resolutions
    - Validate syntax after resolution
    - CLI tool implementation

11. **Python Import Graph Visualizer**
    - Analyze Python project imports
    - Generate dependency graphs
    - Detect circular imports
    - Identify unused imports

12. **SQLite-Based Job Queue Implementation**
    - Minimal dependencies (just stdlib + sqlite)
    - Priority queuing
    - Retry logic with exponential backoff
    - Worker pool management

13. **Python AST-Based Refactoring Tool**
    - Rename variables across files
    - Extract function/method
    - Inline variable
    - Safe refactoring with tests

### Category 4: Integration/Compatibility Research

**Existing examples:** cmarkgfm-in-pyodide, wazero-python-claude, h3o-python

**New project ideas:**

14. **Python 3.14t Free-Threading Compatibility Study**
    - Test top 100 PyPI packages
    - Measure performance with nogil
    - Identify compatibility issues
    - Create compatibility matrix

15. **Rust-Based Python Packages in WebAssembly**
    - Survey Rust+PyO3 packages
    - Test compilation to WASM
    - Performance comparison
    - Migration guide

16. **Python Package Cross-Interpreter Compatibility**
    - Test packages on CPython, PyPy, GraalPython
    - Performance differences
    - C extension compatibility
    - Feature parity analysis

### Category 5: Data Analysis Projects

**Existing examples:** blog-tags-scikit-learn, datasette-plugin-alpha-versions

**New project ideas:**

17. **PyPI Dependency Graph Analysis**
    - Scrape PyPI metadata
    - Build dependency network
    - Identify critical packages (most depended-on)
    - Analyze security implications

18. **Python Version Adoption Analysis**
    - Survey top packages' Python version support
    - Migration timeline patterns
    - Compatibility matrices
    - Predict Python 3.14 adoption curve

19. **LLM API Pricing and Performance Comparison**
    - Compare OpenAI, Anthropic, Google, Cohere
    - Cost per token analysis
    - Latency benchmarks
    - Context window utilization
    - Quality vs cost tradeoffs

20. **Python Standard Library Usage Patterns**
    - Analyze top GitHub Python projects
    - Most/least used stdlib modules
    - Replacement library patterns
    - Evolution over time

### Category 6: Security & Sandboxing

**Existing examples:** codex-sandbox-investigation

**New project ideas:**

21. **Python Sandbox Implementations Comparison**
    - RestrictedPython vs pysandbox vs others
    - Security model analysis
    - Performance overhead
    - Bypass techniques and mitigations

22. **Python Secrets Management Approaches**
    - Environment variables vs keyring vs vault
    - Security best practices
    - Cloud-specific solutions
    - Development vs production patterns

### Top 5 Most Promising Projects

Based on fit for this environment, I recommend:

**1. pip vs uv Dependency Resolution Deep Dive**
- Builds on existing uv-run-flow-analysis
- Highly relevant to Python community
- Clear testable outcomes
- Can compare algorithms, performance, lock files

**2. Python Testing Framework Internals (pytest focus)**
- Widely used tool with complex internals
- Educational value for plugin developers
- Clear codebase to analyze
- Can create diagrams and traces

**3. Python 3.14t Free-Threading Compatibility Study**
- Timely (Python 3.14 is recent)
- Creates valuable compatibility data
- Measurable results
- Helps ecosystem adoption

**4. LLM CLI Tools Comparison**
- Directly relevant to this workflow
- Compare llm, aichat, openai CLI, anthropic CLI
- Benchmark performance and features
- Improve tooling for future research

**5. FastAPI vs Flask Performance & Architecture**
- Very popular frameworks
- Clear comparison points
- Benchmark opportunities
- Architecture analysis + performance testing

## Conclusion

Claude Code succeeds in this environment because it combines:
- **Clear structure** (AGENTS.md guidelines)
- **Objective validation** (code works or doesn't)
- **Appropriate permissions** (full trust, isolated environment)
- **Self-documentation** (notes.md pattern)
- **Automation** (GitHub Actions, cogapp, LLM summaries)
- **Focused scope** (research projects, not ongoing development)

The environment is specifically designed to leverage AI strengths (investigation, code analysis, benchmarking, documentation) while providing guardrails (folder structure, deliverables) and validation (executable code, measurable results).

For future research projects, the most promising areas are:
1. Tool/library comparisons with benchmarks
2. Deep codebase analysis of popular tools
3. Compatibility/integration studies
4. Data-driven ecosystem analysis
5. Proof-of-concept implementations

The key is choosing projects with clear scope, testable outcomes, and value to the broader developer community.
