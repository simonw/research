#!/usr/bin/env python3
"""Run every example in tutorial.md and check that it prints what the tutorial shows.

Python 3.8+ standard library only.  Run it from the project folder:

    python3 tutorial/verify_tutorial.py --ext ext/target/release/libsparqlite.so

It builds a scratch copy of the project layout (tutorial/make_tutorial_data.py, the
files in tutorial/data/ and the extension at ext/target/release/), runs the
tutorial's setup commands there in order, then replays the SQL examples through the
sqlite3 shell exactly as a reader would type them, and compares the output.  Nothing
in the real project folder is modified (unless you pass --update, which rewrites
the output blocks of tutorial.md).  The expected outputs were recorded with SQLite
3.45.1 on Linux; other versions of the shell may format some things differently.

How tutorial.md is read
-----------------------
* A fenced block with info string ``sql`` is typed into the current sqlite3 session.
  If the next thing after it (blank lines aside) is a fenced ``output`` block, the
  shell's output must match it; a ``sql`` block with no ``output`` block must print
  nothing at all.
* Directives are HTML comments on a line of their own, so they are invisible in the
  rendered page.  ``session`` takes effect where it stands; all others apply to the
  next code block:

    <!-- verify: session [DB] -->   close the current sqlite3 process and start
                                   ``sqlite3 [DB]`` in the project folder (no DB =
                                   a transient in-memory database).
    <!-- verify: setup -->          run the next ``sh`` block with bash in the
                                   project folder (compared with a following
                                   ``output`` block, if any).
    <!-- verify: setup fresh -->    the same, in a brand-new copy of the project
                                   folder (for an alternative route that should
                                   start from scratch).
    <!-- verify: python -->         run the next ``python`` block (cwd: project).
    <!-- verify: skip -->           do not run the next block (illustrative, or
                                   needs something the checker cannot provide).
    <!-- verify: ignore-timing -->  compare the next output ignoring durations:
                                   "Run Time:" lines, "(0.01s)", "12 ms",
                                   "time=1.2ms", "\"ms\":4" and the like.
    <!-- verify: mask REGEX -->     replace REGEX matches by <masked> on both
                                   sides before comparing (may be repeated).

  ``sh``/``console``/``python``/``turtle``/... blocks without such a directive are
  illustrative and are not run.
* Elision: a line consisting of exactly ``...`` in an ``output`` block matches any
  number of lines (including none).
* Before comparing, both sides get: "\\r" removed, trailing spaces removed, trailing
  blank lines removed, and "Parse error near line 12:" (what the shell prints when
  it reads a pipe) shortened to "Parse error:" (what it prints when you type).  With
  ignore-timing or mask, runs of spaces and of box-drawing rules are also collapsed,
  so a masked number of another width still lines up.

Exit status: 0 when every executed block matched, 1 otherwise.
"""

from __future__ import annotations

import argparse
import difflib
import os
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
PROJECT = os.path.dirname(HERE)
DEFAULT_TUTORIAL = os.path.join(PROJECT, "tutorial.md")
BLOCK_TIMEOUT = 300.0  # seconds per block

FENCE = re.compile(r"^(?P<indent> {0,3})(?P<fence>`{3,}|~{3,})(?P<info>[^`]*)$")
DIRECTIVE = re.compile(r"^\s*<!--\s*verify:\s*(?P<body>.*?)\s*-->\s*$")
ERROR_LINE = re.compile(r"^(Parse error|Runtime error|Error) near line \d+:", re.M)
TIMING = [
    (re.compile(r"^Run Time: .*$", re.M), "Run Time: <t>"),
    (re.compile(r'"ms":\s*\d+(\.\d+)?'), '"ms":<t>'),
    (re.compile(r"\b(time|ms|elapsed)=\s*\d+(\.\d+)?\s*(ns|us|µs|ms|s)?"), r"\1=<t>"),
    (re.compile(r"\(\d+(\.\d+)?\s*(ns|us|µs|ms|s)\)"), "(<t>)"),
    (re.compile(r"\b\d+(\.\d+)?\s*(ns|us|µs|ms)\b"), "<t>"),
]
RULES = re.compile(r"[─━═-]{2,}")
SPACES = re.compile(r"[ \t]+")


def platform_ext_name() -> str:
    if sys.platform == "darwin":
        return "libsparqlite.dylib"
    if sys.platform.startswith("win"):
        return "sparqlite.dll"
    return "libsparqlite.so"


# ---------------------------------------------------------------------------
# Parsing tutorial.md
# ---------------------------------------------------------------------------


class Block:
    def __init__(self, info, text, start, end, body_start, directives):
        self.info = info            # first word of the info string ("sql", "output", ...)
        self.text = text            # content, without the fences
        self.start = start          # 0-based line index of the opening fence
        self.end = end              # 0-based line index of the closing fence
        self.body_start = body_start
        self.directives = directives
        self.output = None          # the Block holding the expected output, if paired

    @property
    def line(self) -> int:
        return self.start + 1


class Doc:
    def __init__(self, path: str):
        self.path = path
        with open(path, encoding="utf-8") as f:
            self.lines = f.read().split("\n")
        self.items = []  # ("session", arg, line) or ("block", Block)
        self.parse()

    def parse(self) -> None:
        pending = []
        i = 0
        lines = self.lines
        last_block_end = None
        while i < len(lines):
            m = DIRECTIVE.match(lines[i])
            if m:
                body = m.group("body")
                word, _, rest = body.partition(" ")
                if word == "session":
                    self.items.append(("session", rest.strip() or None, i + 1))
                elif word in ("setup", "python", "skip", "ignore-timing", "mask"):
                    pending.append((word, rest.strip()))
                else:
                    raise SystemExit("%s:%d: unknown verify directive %r" % (self.path, i + 1, word))
                i += 1
                continue
            m = FENCE.match(lines[i])
            if m:
                fence = m.group("fence")
                info = (m.group("info").strip().split() or [""])[0]
                indent = len(m.group("indent"))
                j = i + 1
                body = []
                while j < len(lines):
                    close = lines[j].strip()
                    if close.startswith(fence[0] * len(fence)) and set(close) == {fence[0]}:
                        break
                    body.append(lines[j][indent:] if lines[j][:indent].strip() == "" else lines[j])
                    j += 1
                else:
                    raise SystemExit("%s:%d: unclosed code fence" % (self.path, i + 1))
                blk = Block(info, "\n".join(body), i, j, i + 1, pending)
                pending = []
                if info == "output":
                    prev = self.items[-1][1] if self.items and self.items[-1][0] == "block" else None
                    gap_ok = prev is not None and last_block_end is not None and all(
                        not lines[k].strip() for k in range(last_block_end + 1, i))
                    if not gap_ok or prev.info not in ("sql", "sh", "python") or prev.output:
                        raise SystemExit("%s:%d: an output block must follow a sql, sh or python "
                                         "block directly" % (self.path, i + 1))
                    if blk.directives:
                        raise SystemExit("%s:%d: put directives before the sql block, not before "
                                         "its output" % (self.path, i + 1))
                    prev.output = blk
                else:
                    self.items.append(("block", blk))
                last_block_end = j
                i = j + 1
                continue
            if lines[i].strip() and not lines[i].lstrip().startswith("<!--"):
                pass  # prose
            i += 1
        if pending:
            raise SystemExit("%s: a verify directive at the end applies to no block" % self.path)


# ---------------------------------------------------------------------------
# Comparing
# ---------------------------------------------------------------------------


def clean(text: str) -> str:
    text = text.replace("\r", "")
    text = ERROR_LINE.sub(lambda m: m.group(1) + ":", text)
    lines = [ln.rstrip() for ln in text.split("\n")]
    while lines and not lines[-1]:
        lines.pop()
    while lines and not lines[0]:
        lines.pop(0)
    return "\n".join(lines)


def loosen(text: str, directives) -> str:
    masks = [d[1] for d in directives if d[0] == "mask"]
    timing = any(d[0] == "ignore-timing" for d in directives)
    if not masks and not timing:
        return text
    for rx in masks:
        text = re.sub(rx, "<masked>", text)
    if timing:
        for rx, repl in TIMING:
            text = rx.sub(repl, text)
    text = RULES.sub("─", text)
    text = SPACES.sub(" ", text)
    return text


def matches(expected: str, actual: str) -> bool:
    exp_lines = expected.split("\n")
    if "..." not in exp_lines:
        return expected == actual
    parts = []
    for ln in exp_lines:
        parts.append(r"(?:[^\n]*\n)*?" if ln == "..." else re.escape(ln) + r"\n")
    return re.fullmatch("".join(parts), actual + "\n") is not None


# ---------------------------------------------------------------------------
# Running
# ---------------------------------------------------------------------------


class Session:
    """One sqlite3 shell, fed block by block; output read up to a marker line."""

    def __init__(self, sqlite3: str, db, cwd: str, env):
        args = [sqlite3] + ([db] if db else [])
        self.proc = subprocess.Popen(
            args, cwd=cwd, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT, encoding="utf-8", errors="replace", bufsize=1)
        self.q = queue.Queue()
        self.counter = 0
        self.dead = None
        t = threading.Thread(target=self._pump, daemon=True)
        t.start()

    def _pump(self):
        for line in self.proc.stdout:
            self.q.put(line)
        self.q.put(None)

    def run(self, text: str) -> str:
        if self.dead:
            raise RuntimeError("the session ended earlier: %s" % self.dead)
        self.counter += 1
        marker = "@@VERIFY-BLOCK-%d@@" % self.counter
        try:
            self.proc.stdin.write(text.rstrip("\n") + "\n.print " + marker + "\n")
            self.proc.stdin.flush()
        except (BrokenPipeError, OSError) as e:
            self.dead = "sqlite3 exited (%s)" % e
            raise RuntimeError(self.dead)
        out = []
        deadline = time.monotonic() + BLOCK_TIMEOUT
        while True:
            try:
                line = self.q.get(timeout=max(0.1, deadline - time.monotonic()))
            except queue.Empty:
                self.close()
                self.dead = "no end marker within %ds (an unterminated statement?)" % BLOCK_TIMEOUT
                raise RuntimeError(self.dead)
            if line is None:
                self.dead = "sqlite3 exited with status %s" % self.proc.wait()
                raise RuntimeError(self.dead + "; output so far:\n" + "".join(out))
            if line.rstrip("\r\n") == marker:
                return "".join(out)
            if marker in line:  # e.g. the marker swallowed into an unterminated string
                out.append(line.replace(marker, ""))
                return "".join(out)
            out.append(line)

    def close(self):
        if self.proc.poll() is None:
            try:
                self.proc.stdin.close()
            except OSError:
                pass
            try:
                self.proc.wait(timeout=30)
            except subprocess.TimeoutExpired:
                self.proc.kill()
                self.proc.wait()


def make_root(base: str, ext: str, n: int) -> str:
    """A scratch copy of the project layout the tutorial's commands expect."""
    root = os.path.join(base, "project" if n == 0 else "project-%d" % n)
    os.makedirs(os.path.join(root, "tutorial", "data"))
    os.makedirs(os.path.join(root, "ext", "target", "release"))
    shutil.copy2(os.path.join(HERE, "make_tutorial_data.py"), os.path.join(root, "tutorial"))
    data = os.path.join(HERE, "data")
    if os.path.isdir(data):
        for name in sorted(os.listdir(data)):
            src = os.path.join(data, name)
            # people.nt is large and regenerated on demand; copy only the checked-in files
            if os.path.isfile(src) and name != "people.nt":
                shutil.copy2(src, os.path.join(root, "tutorial", "data", name))
    shutil.copy2(ext, os.path.join(root, "ext", "target", "release", platform_ext_name()))
    return root


def resolve_ext(path: str) -> str:
    for cand in [path] + [path + s for s in (".so", ".dylib", ".dll")]:
        if os.path.isfile(cand):
            return os.path.abspath(cand)
    raise SystemExit("extension not found: %s (build it with: cargo build --release "
                     "--manifest-path ext/Cargo.toml)" % path)


def default_ext():
    rel = os.path.join(PROJECT, "ext", "target", "release")
    for name in ("libsparqlite.so", "libsparqlite.dylib", "sparqlite.dll"):
        if os.path.isfile(os.path.join(rel, name)):
            return os.path.join(rel, name)
    return None


def run_shell(text: str, cwd: str, env) -> str:
    bash = shutil.which("bash", path=env.get("PATH"))
    if not bash:
        raise RuntimeError("bash is needed to run setup blocks")
    p = subprocess.run([bash, "-e", "-c", text], cwd=cwd, env=env, stdout=subprocess.PIPE,
                       stderr=subprocess.STDOUT, encoding="utf-8", errors="replace",
                       timeout=BLOCK_TIMEOUT * 4)
    if p.returncode != 0:
        raise RuntimeError("exit status %d; output:\n%s" % (p.returncode, p.stdout))
    return p.stdout


def run_python(text: str, cwd: str, env, python: str) -> str:
    fd, path = tempfile.mkstemp(suffix=".py")
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(text + "\n")
    try:
        p = subprocess.run([python, path], cwd=cwd, env=env, stdout=subprocess.PIPE,
                           stderr=subprocess.STDOUT, encoding="utf-8", errors="replace",
                           timeout=BLOCK_TIMEOUT)
    finally:
        os.unlink(path)
    if p.returncode != 0:
        raise RuntimeError("exit status %d; output:\n%s" % (p.returncode, p.stdout))
    return p.stdout


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--ext", help="the built extension (default: ext/target/release/libsparqlite.*)")
    ap.add_argument("--sqlite3", default=os.environ.get("SQLITE3", "sqlite3"),
                    help="the sqlite3 shell to use (default: $SQLITE3 or sqlite3 on PATH); "
                         "its directory is also put first on PATH for the setup commands")
    ap.add_argument("--python", default=sys.executable,
                    help="Python for the python blocks (default: this one)")
    ap.add_argument("--tutorial", default=DEFAULT_TUTORIAL, help="default: tutorial.md")
    ap.add_argument("--work", help="scratch directory to use (default: a new temporary one)")
    ap.add_argument("--keep", action="store_true", help="keep the scratch directory")
    ap.add_argument("--update", action="store_true",
                    help="rewrite the output blocks with the actual output (blocks that use "
                         "'...' elision are left alone); review the diff afterwards")
    ap.add_argument("-v", "--verbose", action="store_true", help="print every block's status")
    args = ap.parse_args(argv)

    ext = resolve_ext(args.ext) if args.ext else default_ext()
    if not ext:
        ap.error("no build in ext/target/release/; pass --ext PATH")
    sqlite3 = shutil.which(args.sqlite3)
    if not sqlite3:
        ap.error("sqlite3 shell not found: %s (pass --sqlite3 PATH)" % args.sqlite3)
    doc = Doc(args.tutorial)

    base = args.work or tempfile.mkdtemp(prefix="verify-tutorial-")
    if args.work:
        if os.path.exists(base) and os.listdir(base):
            ap.error("--work %s exists and is not empty" % base)
        os.makedirs(base, exist_ok=True)
    env = dict(os.environ)
    env["PATH"] = os.path.dirname(sqlite3) + os.pathsep + env.get("PATH", "")
    env.pop("SPARQLITE_DEBUG_HOOKS", None)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    roots = [make_root(base, ext, 0)]
    root = roots[0]
    print("tutorial: %s\nextension: %s\nsqlite3:   %s\nscratch:   %s" % (
        os.path.relpath(args.tutorial), ext, sqlite3, base))

    counts = {"sql": 0, "sql-silent": 0, "setup": 0, "python": 0}
    skipped, illustrative, failures, updates = [], [], [], []
    session = None
    t_start = time.monotonic()

    def check(blk, actual, kind):
        actual_clean = clean(actual)
        expected = clean(blk.output.text) if blk.output else ""
        ok = matches(loosen(expected, blk.directives), loosen(actual_clean, blk.directives))
        if ok:
            counts[kind] += 1
            if args.verbose:
                print("  ok    line %-5d %s" % (blk.line, kind))
            return
        failures.append((blk, expected, actual_clean))
        if args.update and (blk.output is None or "..." not in expected.split("\n")):
            updates.append((blk, actual_clean))
        print("  FAIL  line %d (%s block)" % (blk.line, blk.info))
        diff = difflib.unified_diff(expected.split("\n"), actual_clean.split("\n"),
                                    "expected (tutorial.md)", "actual", lineterm="", n=2)
        for d in diff:
            print("        " + d)

    def fail(blk, err):
        failures.append((blk, None, str(err)))
        print("  FAIL  line %d (%s block): %s" % (blk.line, blk.info, err))

    try:
        for item in doc.items:
            if item[0] == "session":
                if session:
                    session.close()
                session = Session(sqlite3, item[1], root, env)
                if args.verbose:
                    print("session line %d: sqlite3 %s" % (item[2], item[1] or ""))
                continue
            blk = item[1]
            words = [d[0] for d in blk.directives]
            if "skip" in words:
                skipped.append(blk)
                continue
            if blk.info == "sql":
                if session is None:
                    fail(blk, "no <!-- verify: session --> before this sql block")
                    continue
                try:
                    out = session.run(blk.text)
                except RuntimeError as e:
                    fail(blk, e)
                    continue
                check(blk, out, "sql" if blk.output else "sql-silent")
            elif "setup" in words and blk.info in ("sh", "bash", "console", "shell"):
                fresh = any(d == ("setup", "fresh") for d in blk.directives)
                cwd = root
                if fresh:
                    cwd = make_root(base, ext, len(roots))
                    roots.append(cwd)
                try:
                    out = run_shell(blk.text, cwd, env)
                except (RuntimeError, subprocess.TimeoutExpired) as e:
                    fail(blk, e)
                    continue
                check(blk, out, "setup")
            elif "python" in words and blk.info == "python":
                try:
                    out = run_python(blk.text, root, env, args.python)
                except (RuntimeError, subprocess.TimeoutExpired) as e:
                    fail(blk, e)
                    continue
                check(blk, out, "python")
            else:
                if blk.directives:
                    fail(blk, "directives %s do not apply to a %r block" % (words, blk.info))
                    continue
                illustrative.append(blk)
    finally:
        if session:
            session.close()

    if args.update and updates:
        lines = doc.lines
        for blk, actual in sorted(updates, key=lambda u: u[0].start, reverse=True):
            new = actual.split("\n") if actual else []
            if blk.output:
                lines[blk.output.body_start:blk.output.end] = new
            elif actual:
                lines[blk.end + 1:blk.end + 1] = ["", "```output"] + new + ["```"]
        with open(args.tutorial, "w", encoding="utf-8", newline="\n") as f:
            f.write("\n".join(lines))
        print("updated %d output block(s) in %s; review the diff" % (len(updates), args.tutorial))

    verified = sum(counts.values())
    print("\nverified %d blocks: %d sql+output pairs, %d sql blocks that print nothing, "
          "%d setup commands, %d python blocks" % (
              verified, counts["sql"], counts["sql-silent"], counts["setup"], counts["python"]))
    print("skipped %d blocks marked skip; %d other blocks are illustrative (%s)" % (
        len(skipped), len(illustrative),
        ", ".join("%s: %d" % (k, sum(1 for b in illustrative if b.info == k))
                  for k in sorted({b.info or "(none)" for b in illustrative}))))
    print("%d failure(s) in %.1fs" % (len(failures), time.monotonic() - t_start))
    if not args.keep and not args.work:
        shutil.rmtree(base, ignore_errors=True)
    elif args.keep:
        print("kept %s" % base)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
