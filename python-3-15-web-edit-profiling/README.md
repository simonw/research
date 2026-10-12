# Profiling web-edit with Python 3.15: flame graphs and source heatmaps

Python 3.15's built-in sampling profiler produced useful, interactive profiles of the unmodified `web-edit` app from PyPI. In a deliberately busy editing workload, preserving a large document's line endings appeared in 64.8% of recorded CPU stacks. The most important configuration choice was sampling **all threads**: 95.1% of the recorded stacks came from the app's AnyIO worker thread. A main-thread-only recording would have missed most of the filesystem work.

The experiment also exposed two practical issues: macOS required a locally signed interpreter for process-memory access, and replaying the binary recording to a heatmap lost the collection statistics in the heatmap header. This report explains the workload, results, interpretation, and reproduction steps. The second visualization is a source-code heatmap, not a heap-memory profile.

## Interactive reports

These links target the repository's GitHub Pages deployment:

- **[Open the flame graph](https://simonw.github.io/research/python-3-15-web-edit-profiling/artifacts/flamegraph.html)** — search for `preserve_endings`, then click a frame to zoom into its callees.
- **[Open the source heatmap](https://simonw.github.io/research/python-3-15-web-edit-profiling/artifacts/heatmap/index.html)** — click **Expand All**, then open **Site Packages → web_edit → web_edit.files**.
- **[Jump directly to line-ending preservation](https://simonw.github.io/research/python-3-15-web-edit-profiling/artifacts/heatmap/file_0001.html#line-63)** — switch to **Total Time** to see the expensive call sites and **Hot Only** to hide unsampled lines.
- **[Inspect the matching algorithm in difflib](https://simonw.github.io/research/python-3-15-web-edit-profiling/artifacts/heatmap/file_0003.html#line-305)**.
- **[Open the demo launcher](https://simonw.github.io/research/python-3-15-web-edit-profiling/artifacts/index.html)**.

The flame graph and heatmap come from **the same recording**. They are the native Python reports, with embedded scripts, styles, and source context. The heatmap's index and source pages use relative links, so the entire directory must be published together. These URLs become available after this folder reaches the repository's deployed branch; creating the report locally does not publish it.

## What was profiled

[web-edit](https://github.com/simonw/web-edit) is a browser-based editor for files in a local folder. [The PyPI release metadata](https://pypi.org/pypi/web-edit/0.1a0/json) identifies the tested release as `0.1a0`. Inspection of the installed wheel showed a Starlette application built by `web_edit.app.create_app`, served by Uvicorn. The app's synchronous request handlers and explicit `run_in_threadpool()` calls dispatch file operations to AnyIO worker threads.

The fixture contained one 12,000-line UTF-8 document with a mixture of LF and CRLF endings, plus a folder containing 800 small text files. A separate client drove the app over actual loopback HTTP. Each cycle performed eight requests:

1. Browse the root folder.
2. Browse the folder containing 800 documents.
3. Read the large document and its version token.
4. Change every 500th line and save using that version token.
5. Download the saved document and verify its contents and SHA-256 version.
6. Upload a multipart text file.
7. Browse the upload folder to obtain its deletion version token.
8. Delete the uploaded file and check the response.

The client also checked that normalized saved text matched the requested edit and that CRLF endings remained. Failed HTTP responses or content checks abort the run. The fixture is synthetic and chosen to make useful stacks visible; it is not a trace of a person's typical editing session.

The harness used a temporary directory, bound the server only to `127.0.0.1` on an automatically assigned port, completed one warm-up cycle, and then attached the profiler. Only the server PID was sampled. Client JSON generation, client-side response checks, fixture creation, and report rendering were outside the sampled process. The server and temporary files were cleaned up at the end.

## Environment and collection settings

| Setting | Recorded value |
| --- | --- |
| Run date | 2026-10-12 |
| Interpreter | CPython 3.15.0 |
| Operating system | macOS 26.4, ARM64 |
| App | web-edit 0.1a0 |
| Server stack | Starlette 1.7.0, Uvicorn 0.54.0, AnyIO 4.15.1 |
| Multipart parser | python-multipart 0.0.32 |
| Profiling mode | CPU |
| Thread selection | All threads (`-a`) |
| Requested sampling rate | 1 kHz |
| Recorded duration | 20.0003 seconds |
| Sampling-loop attempts | 20,000 |
| Recorded stack samples | 15,296, across two threads |
| Sampling error rate | 13.40% |
| Reported missed-sample rate | 0.0% |
| Completed workload | 389 cycles / 3,112 successful HTTP requests |
| Source heatmap | 45 source files |

The [capture log](artifacts/capture.log) calls its 20,000 sampling-loop iterations “Captured … samples.” The [binary metadata and recomputed figures](artifacts/analysis.json) distinguish those iterations from the 15,296 stored stack samples. Inspection of the installed profiler's `sample.py` showed the iteration counter advances after failed collection attempts too. CPU filtering and all-thread collection also mean this counter is not interchangeable with the number of stored stacks. A 0% missed-sample rate therefore does not mean every attempted stack read succeeded.

The 13.40% error rate is material: these results describe the successful recorded stacks, with no correction for possible sampling bias. The client ran for 20.133 seconds because a complete editing cycle could finish just after the profiler stopped. Its request count covers cycles overlapping the recording window, rather than an exact number of requests inside the profiler's boundaries. Versions, settings, and individual request counts are preserved in [metadata.json](artifacts/metadata.json).

## What the flame graph showed

These percentages were recomputed from [collapsed stacks](artifacts/stacks.txt), counting a function once if it appeared anywhere in a recorded stack:

| Function | Stacks containing it | Share of 15,296 recorded stacks |
| --- | ---: | ---: |
| `WorkerThread.run` | 14,546 | 95.1% |
| `FileStore.save_text` | 11,780 | 77.0% |
| `preserve_endings` | 9,914 | 64.8% |
| `SequenceMatcher.find_longest_match` | 5,854 | 38.3% |
| `FileStore.browse` | 1,154 | 7.5% |

These are **inclusive, overlapping** percentages. A stack can contain the worker loop, `save_text`, `preserve_endings`, and `find_longest_match` at the same time. Adding these rows would double-count the same samples.

The wide save path led through `FileStore.save_text → preserve_endings → SequenceMatcher.get_matching_blocks → SequenceMatcher.find_longest_match`. This made sense after inspecting the app: saving an existing document preserves the line endings of unchanged lines by comparing the original and edited documents. For this large text fixture, the comparison was substantial work even though each edit changed only a small fraction of the lines.

The leaf-frame counts reinforce that interpretation. `SequenceMatcher.find_longest_match` accounted for 5,854 leaf samples, or 38.3% of the recording. Regular-expression `split`, `search`, and `findall` were also prominent, with 1,964, 1,909, and 994 leaf samples respectively. File hashing, directory/path operations, and JSON encoding were visible as smaller contributors. The source reports disambiguate the module names that collapsed stacks shorten to filenames such as `__init__.py`.

Flame graph width shows how often the stack was observed; horizontal position is not a timeline. Sampling can miss short operations entirely. CPU mode emphasizes execution rather than waiting, so these charts do not measure browser responsiveness, network latency, or CodeMirror's JavaScript work. They also do not establish that line-ending preservation dominates all web-edit workloads.

## What the source heatmap added

The [web_edit.files page](https://simonw.github.io/research/python-3-15-web-edit-profiling/artifacts/heatmap/file_0001.html) turns the flame graph's large rectangles into specific source locations. Around lines 63–79, `preserve_endings` splits the documents and their endings, asks `SequenceMatcher` for matching blocks, copies the endings for unchanged lines, and reconstructs the saved string. The matching-block call at line 71 has much more total work beneath it than direct execution on the call line itself.

Switching **Self Time** to **Total Time** is useful here. Self samples identify where a thread was executing at the sampled leaf; total samples include work below a call site. A line with few self samples can be the entry point to an expensive algorithm. The caller/callee arrows connect these views to the implementation in [difflib](https://simonw.github.io/research/python-3-15-web-edit-profiling/artifacts/heatmap/file_0003.html).

The heatmap's cumulative per-file counts can exceed the total recorded stack count because a single stack can contain several distinct lines from one file. Do not add the file totals or interpret their display percentages as non-overlapping fractions of the session. A line with zero samples is also not proof that it never ran.

### A reproducible heatmap replay limitation

The native heatmap index displayed `0.0` for duration and sampling rate and `N/A` for collection diagnostics. The recording itself contains the real values: approximately 20 seconds, 999.99 attempts per second, and a 13.40% error rate. The flame graph restored those values correctly.

Inspection of the installed CPython 3.15.0 source explains the difference. The replay code calls `set_replay_stats(info)` only when a collector provides it. `FlamegraphCollector` implements that method. `HeatmapCollector` has `set_stats()` but no `set_replay_stats()`, and inherits directly from `StackTraceCollector`, which does not supply it either. Runtime introspection confirmed `hasattr(HeatmapCollector, "set_replay_stats")` is false. The heatmap consequently retains its empty statistics dictionary during binary replay even though it processes the source samples.

The published native reports retain this behavior rather than silently changing their displayed values. Use the flame graph, [capture log](artifacts/capture.log), or [binary metadata](artifacts/analysis.json) for timing and diagnostics. This is an observation about the tested 3.15.0 implementation, not a claim that every Python version has the same limitation. Relevant upstream source is in [the replay CLI](https://github.com/python/cpython/blob/v3.15.0/Lib/profiling/sampling/cli.py), [stack collectors](https://github.com/python/cpython/blob/v3.15.0/Lib/profiling/sampling/stack_collector.py), and [heatmap collector](https://github.com/python/cpython/blob/v3.15.0/Lib/profiling/sampling/heatmap_collector.py).

## Running the profiler on macOS

The first smoke test with the ordinary uv-managed Python failed with a process-memory permission error. The native profiler needs access to another process's memory; starting the target through its `run` command did not remove that requirement on this machine.

The successful setup copied the interpreter into the project's `.venv/bin/python-profile` and ad-hoc signed only that copy with `com.apple.security.get-task-allow` and `com.apple.security.cs.debugger`. Both the server and the sampling process ran with that copy. This left the uv-managed interpreter and operating-system configuration unchanged and worked without `sudo` or disabling SIP. The runner recreates the private copy when the original interpreter's fingerprint changes.

The distinction matters for reproducing the demo: installing Python 3.15 was sufficient to obtain the profiler, but was not sufficient for macOS process inspection. [Python's remote-debugging guide](https://docs.python.org/3.15/howto/remote_debugging.html) describes the target entitlement, and [Apple documents the debugging entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.debugger). These instructions and the automated setup were tested on this Mac; other systems may impose different process-inspection requirements.

## Reproduce the experiment

From this research folder, with `uv` installed:

```sh
uv sync --locked
uv run demo.py --output fresh-run
```

This installs the pinned environment, creates the temporary workload, collects a new 20-second profile, and generates both HTML views. The separate output directory preserves the original evidence included in `artifacts/`. Open `fresh-run/index.html` when it finishes. A rerun's sample counts and percentages will differ.

To change the workload or render one format:

```sh
uv run demo.py flamegraph --output flamegraph-run
uv run demo.py heatmap --output heatmap-run
uv run demo.py --seconds 30 --lines 20000 --files 1200 --output larger-run
uv run demo.py --reuse --output fresh-run
```

The core native commands are:

```sh
uv run .venv/bin/python-profile -m profiling.sampling attach -a --mode cpu -r 1khz -d 20 --binary -o recording.bin SERVER_PID
uv run python -m profiling.sampling replay --flamegraph -o flamegraph.html recording.bin
uv run python -m profiling.sampling replay --heatmap -o heatmap recording.bin
```

The harness supplies a real, warmed server PID. The signed interpreter path is specific to the macOS setup; other systems can use an appropriately configured Python 3.15 interpreter. Full command reference: [Python 3.15 `profiling.sampling`](https://docs.python.org/3.15/library/profiling.sampling.html).

Recompute the figures in this report without running the workload:

```sh
uv run summarize_capture.py
uv run summarize_capture.py fresh-run
```

This reads the binary's stored metadata and independently aggregates its companion collapsed-stack file, checking that their recorded stack counts match. It uses Python 3.15's internal `BinaryReader`, so future Python versions may require adjustments. The [saved analysis output](artifacts/analysis.json) corresponds to the original recording.

Binary files retain the original absolute source paths. Replaying them with source context requires those files to remain at their original locations. The committed HTML already embeds its source context and can be viewed on another computer. For a new checkout, generate a fresh recording to get source paths matching that checkout rather than assuming the old binary can rediscover them.

## What this suggests investigating next

For this workload, an optimization experiment should start with line-ending preservation and text processing. Useful controlled comparisons would include smaller documents, documents with uniform endings, large edits versus small edits, and varying directory sizes. Any change would need to retain mixed-ending correctness and the existing version-token behavior. This investigation did not modify web-edit or demonstrate a speedup.

A separate wall-clock recording would help distinguish computation from waiting on filesystem operations such as `fsync`; a browser profile would be needed for frontend behavior. The native sampler is useful for locating candidate work, but these measurements are not a controlled benchmark and do not establish profiler overhead.

## Evidence and validation

| File | Purpose |
| --- | --- |
| [demo.py](demo.py) | Capture orchestration, private macOS interpreter, replay, demo launcher |
| [serve_demo.py](serve_demo.py) | Unmodified PyPI app under Uvicorn, readiness and shutdown |
| [workload.py](workload.py) | Generated fixtures and validated HTTP editing cycles |
| [summarize_capture.py](summarize_capture.py) | Recompute binary metadata and function counts |
| [uv.lock](uv.lock) | Exact dependency versions and distribution hashes |
| [web-edit.bin](artifacts/web-edit.bin) | Original compressed binary recording, approximately 64 KiB |
| [metadata.json](artifacts/metadata.json) | Runtime, collection command, workload counts and client timings |
| [analysis.json](artifacts/analysis.json) | Stored binary metadata and recomputed sample shares |
| [capture.log](artifacts/capture.log) | Collection diagnostics |
| [stacks.txt](artifacts/stacks.txt) | Collapsed sample stacks |
| [hotspots.txt](artifacts/hotspots.txt) | Native text statistics; these are samples, not exact call counts |
| [notes.md](notes.md) | Experiments, failures, and packaging notes |

Validation of the original run covered the HTTP assertions, the real app's save-path frames, absence of load-generator frames, successful binary replay, local report links, empty server error logs, and server shutdown. Browser inspection checked flame graph search and heatmap source display, total-time coloring, and hot-line filtering. The scripts passed lint and formatting checks. Packaging checks separately verified that every report link into this folder has a target and that the generated Pages paths preserve all heatmap links.

The repository's existing Pages workflow copies static assets, so no profiling, dependency installation, or new build hook is needed to publish these already-rendered reports. The embedded source pages are generated profiling output; no package checkout or virtual environment is included.
