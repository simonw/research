"""Record web-edit with Python 3.15 and render its native profiling demos."""

import argparse
import hashlib
import html
import importlib.metadata
import json
import os
import platform
import plistlib
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from datetime import UTC, datetime
from pathlib import Path

from workload import Session, seed

ROOT = Path(__file__).resolve().parent


def profile_python():
    """Give only a private interpreter copy macOS debugging entitlements."""
    if sys.platform != "darwin":
        return Path(sys.executable)
    if not shutil.which("codesign"):
        raise RuntimeError(
            "macOS requires codesign for the local profiling interpreter"
        )
    target = Path(sys.executable).parent / "python-profile"
    # All entry points are run through uv, in this project's disposable venv.
    if Path(sys.prefix).resolve() != (ROOT / ".venv").resolve():
        raise RuntimeError(
            "Run this demo using uv run demo.py so signing stays inside .venv"
        )
    if Path(sys.executable).name == "python-profile":
        return target
    original = Path(sys.executable).resolve()
    fingerprint = hashlib.sha256(original.read_bytes()).hexdigest()
    marker = target.with_suffix(".sha256")
    if target.exists() and marker.exists() and marker.read_text() == fingerprint:
        return target
    target.unlink(missing_ok=True)
    shutil.copy2(original, target)
    entitlements = target.with_suffix(".plist")
    entitlements.write_bytes(
        plistlib.dumps(
            {
                "com.apple.security.get-task-allow": True,
                "com.apple.security.cs.debugger": True,
            }
        )
    )
    subprocess.run(
        [
            "codesign",
            "--force",
            "--sign",
            "-",
            "--entitlements",
            str(entitlements),
            str(target),
        ],
        check=True,
    )
    marker.write_text(fingerprint)
    return target


def command(python, *args):
    return ["uv", "run", "--no-sync", str(python), *map(str, args)]


def stop_process(process):
    if process is None or process.poll() is not None:
        return
    # uv and its child share a dedicated process group on Unix.
    if os.name == "posix":
        os.killpg(process.pid, signal.SIGTERM)
    else:
        process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        else:
            process.kill()
        process.wait()


def record(args, output):
    python = profile_python()
    binary = output / "web-edit.bin"
    with tempfile.TemporaryDirectory(prefix="web-edit-demo-") as temporary:
        state = Path(temporary)
        directory = state / "files"
        seed(directory, args.lines, args.files)
        server = profiler = None
        with (
            (output / "server.log").open("w") as server_log,
            (output / "capture.log").open("w") as capture_log,
        ):
            try:
                server = subprocess.Popen(
                    command(
                        python,
                        ROOT / "serve_demo.py",
                        directory,
                        state,
                        "--timeout",
                        args.seconds + 120,
                    ),
                    cwd=ROOT,
                    stdout=server_log,
                    stderr=subprocess.STDOUT,
                    start_new_session=os.name == "posix",
                )
                deadline = time.monotonic() + 30
                while not (state / "ready.json").exists():
                    if server.poll() is not None or time.monotonic() > deadline:
                        raise RuntimeError(
                            f"Server did not start; see {output / 'server.log'}"
                        )
                    time.sleep(0.05)
                ready = json.loads((state / "ready.json").read_text())
                session = Session(ready["port"])
                session.cycle()  # Warm imports, MIME types, and the worker pool before attaching.
                session.counts.clear()
                session.elapsed.clear()
                session.received_bytes = 0
                session.cycle_number = 0
                capture_command = command(
                    python,
                    "-m",
                    "profiling.sampling",
                    "attach",
                    "-a",
                    "--mode",
                    "cpu",
                    "-r",
                    args.rate,
                    "-d",
                    args.seconds,
                    "--binary",
                    "-o",
                    binary,
                    ready["pid"],
                )
                print(
                    f"Profiling web-edit on 127.0.0.1:{ready['port']} for {args.seconds:g}s ...",
                    flush=True,
                )
                profiler = subprocess.Popen(
                    capture_command,
                    cwd=ROOT,
                    stdout=capture_log,
                    stderr=subprocess.STDOUT,
                    start_new_session=os.name == "posix",
                )
                started = time.monotonic()
                while profiler.poll() is None:
                    session.cycle()
                    if time.monotonic() - started > args.seconds + 60:
                        raise RuntimeError("Profiler exceeded its deadline")
                if profiler.returncode:
                    capture_log.flush()
                    raise RuntimeError((output / "capture.log").read_text())
                summary = {
                    "generated_at": datetime.now(UTC).isoformat(),
                    "python": platform.python_version(),
                    "platform": platform.platform(),
                    "web_edit": importlib.metadata.version("web-edit"),
                    "dependencies": {
                        name: importlib.metadata.version(name)
                        for name in (
                            "starlette",
                            "uvicorn",
                            "anyio",
                            "python-multipart",
                        )
                    },
                    "mode": "cpu",
                    "all_threads": True,
                    "sampling_rate": args.rate,
                    "requested_seconds": args.seconds,
                    "workload_seconds": round(time.monotonic() - started, 3),
                    "fixture": {
                        "text_lines": args.lines,
                        "directory_files": args.files,
                    },
                    "capture_command": capture_command,
                    "workload": session.summary(),
                    "scope": "Warmed HTTP server only; client runs outside the sampled process. Request counts include complete cycles overlapping the capture window.",
                }
                (output / "metadata.json").write_text(
                    json.dumps(summary, indent=2) + "\n"
                )
            finally:
                stop_process(profiler)
                (state / "stop").touch()
                if server is not None:
                    try:
                        server.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        stop_process(server)
    return binary


def replay(output, kind):
    binary = output / "web-edit.bin"
    if not binary.is_file():
        raise RuntimeError(f"No recording at {binary}; run without --reuse first")
    formats = ["flamegraph", "heatmap"] if kind == "both" else [kind]
    for format_name in formats:
        destination = output / (
            "flamegraph.html" if format_name == "flamegraph" else "heatmap"
        )
        subprocess.run(
            command(
                sys.executable,
                "-m",
                "profiling.sampling",
                "replay",
                f"--{format_name}",
                "-o",
                destination,
                binary,
            ),
            cwd=ROOT,
            check=True,
        )
    subprocess.run(
        command(
            sys.executable,
            "-m",
            "profiling.sampling",
            "replay",
            "--collapsed",
            "-o",
            output / "stacks.txt",
            binary,
        ),
        cwd=ROOT,
        check=True,
    )
    with (output / "hotspots.txt").open("w") as stream:
        subprocess.run(
            command(
                sys.executable,
                "-m",
                "profiling.sampling",
                "replay",
                "--sort",
                "tottime",
                "--limit",
                "35",
                binary,
            ),
            cwd=ROOT,
            stdout=stream,
            check=True,
        )
    stacks = (output / "stacks.txt").read_text()
    if "FileStore.save_text" not in stacks or "preserve_endings" not in stacks:
        raise RuntimeError(
            "Capture did not contain the expected web-edit save path; increase --seconds"
        )
    metadata_path = output / "metadata.json"
    metadata = json.loads(metadata_path.read_text())
    metadata["rendered"] = sorted(set(metadata.get("rendered", [])) | set(formats))
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n")


def write_index(output):
    metadata = json.loads((output / "metadata.json").read_text())
    workload = metadata["workload"]
    links = []
    for target, title, description in (
        (
            "flamegraph.html",
            "01 / Flame graph",
            "Follow wide stacks into web_edit → save_text → preserve_endings → difflib.",
        ),
        (
            "heatmap/index.html",
            "02 / Source heatmap",
            "Expand All, open Site Packages → web_edit.files, then inspect preserve_endings and browse.",
        ),
    ):
        format_name = "heatmap" if target.startswith("heatmap/") else "flamegraph"
        if format_name in metadata["rendered"] and (output / target).exists():
            links.append(
                f'<a class="card" href="{target}"><h2>{title} ↗</h2><p>{html.escape(description)}</p></a>'
            )
    page = """<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>web-edit / Python 3.15 profiling</title>
<style>body{margin:0;background:#101719;color:#edf4ec;font:18px/1.6 system-ui}main{max-width:1040px;margin:0 auto;padding:70px 28px}small{color:#a8ce9c;letter-spacing:.15em}h1{font-size:clamp(40px,7vw,76px);line-height:1.05;letter-spacing:-.05em;max-width:850px}p{color:#b6c6c1;max-width:800px}section{display:grid;grid-template-columns:repeat(auto-fit,minmax(290px,1fr));gap:18px;margin:36px 0}.card{border:1px solid #3b5148;border-radius:14px;padding:26px;text-decoration:none;background:#192521}.card:hover{background:#21392c}.card h2{color:#d8fca3;font-size:23px}.card p{font-size:17px}a{color:#d8fca3}code{color:#d8fca3}footer{margin-top:45px;font-size:14px;color:#97aea4}.pill{display:inline-block;border:1px solid #3b5148;border-radius:24px;padding:6px 14px;margin:4px 6px 4px 0;font-size:14px}</style>
<main><small>REAL APP · REAL HTTP REQUESTS · NATIVE PYTHON REPORTS</small><h1>Where does<br>web-edit spend CPU time?</h1><p>Browse a folder. Open a document. Edit its contents. Preserve its line endings. Upload, download, and delete a file. Explore one measured editing session through two views.</p>
"""
    page += f'<div><span class="pill">Python {metadata["python"]}</span><span class="pill">web-edit {metadata["web_edit"]}</span><span class="pill">{metadata["requested_seconds"]:g}s · CPU · all threads</span><span class="pill">{workload["requests"]:,} verified HTTP requests</span></div>'
    page += "<section>" + "".join(links) + "</section>"
    page += "<p><strong>How to read it:</strong> flame width represents recorded stack samples, not a timeline. The heatmap shows sampled source locations. CPU mode filters idle waits; worker threads matter because this app dispatches file operations to a thread pool.</p>"
    page += "<p>This is a synthetic workload with a large text file and a busy folder. Its hotspots describe this scenario, not every production editing session. Both views use the same binary recording.</p>"
    page += '<p><small style="letter-spacing:0">Heatmap replay note: Python 3.15.0 displays zero duration/rate in its heatmap index header. Use the capture log or flame graph for timing; source sample counts remain available.</small></p>'
    page += '<footer><a href="metadata.json">Run metadata</a> · <a href="hotspots.txt">Hotspot table</a> · <a href="capture.log">Capture log</a> · <a href="stacks.txt">Collapsed stacks</a><br>Recreate: <code>uv run demo.py</code> · Research: <a href="https://github.com/simonw/research/tree/main/python-3-15-web-edit-profiling#readme">README report</a>.</footer></main></html>'
    (output / "index.html").write_text(page)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "kind", choices=("both", "flamegraph", "heatmap"), nargs="?", default="both"
    )
    parser.add_argument("--seconds", type=int, default=20)
    parser.add_argument("--lines", type=int, default=12000)
    parser.add_argument("--files", type=int, default=800)
    parser.add_argument("--rate", default="1khz")
    parser.add_argument("--output", type=Path, default=ROOT / "artifacts")
    parser.add_argument(
        "--reuse",
        action="store_true",
        help="Render an existing recording without running the workload",
    )
    args = parser.parse_args()
    if sys.version_info[:2] != (3, 15):
        parser.error("Use Python 3.15: uv run --python 3.15 demo.py")
    if args.seconds <= 0 or args.lines < 10 or args.files < 1:
        parser.error(
            "--seconds must be positive, --lines at least 10, and --files at least 1"
        )
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if not args.reuse:
        record(args, output)
    replay(output, args.kind)
    write_index(output)
    print(f"\nReady: {output / 'index.html'}")


if __name__ == "__main__":
    main()
