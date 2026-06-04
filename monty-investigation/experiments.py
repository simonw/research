from __future__ import annotations

import argparse
import asyncio
import dataclasses
import json
import math
import tempfile
import time
from pathlib import Path
from typing import Any, Callable

import pydantic_monty


ROOT = Path(__file__).resolve().parent


def clipped(value: Any, *, max_len: int = 500) -> str:
    try:
        text = repr(value)
    except Exception as exc:  # pragma: no cover - defensive reporting helper
        text = f"<repr failed: {type(exc).__name__}: {exc}>"
    if len(text) > max_len:
        return text[: max_len - 3] + "..."
    return text


def describe_error(exc: BaseException) -> dict[str, Any]:
    info: dict[str, Any] = {
        "error_type": type(exc).__name__,
        "message": str(exc),
    }
    if isinstance(exc, pydantic_monty.MontyError):
        inner = exc.exception()
        info["inner_type"] = type(inner).__name__
        info["inner_message"] = str(inner)
        if hasattr(exc, "display"):
            try:
                info["display_type_msg"] = exc.display("type-msg")  # type: ignore[arg-type]
            except Exception:
                pass
    return info


def run_case(
    name: str,
    code: str,
    *,
    inputs_decl: list[str] | None = None,
    inputs: dict[str, Any] | None = None,
    limits: pydantic_monty.ResourceLimits | dict[str, Any] | None = None,
    external_functions: dict[str, Callable[..., Any]] | None = None,
    mount: Any = None,
    os: Any = None,
    type_check: bool = False,
    type_check_stubs: str | None = None,
) -> dict[str, Any]:
    t0 = time.perf_counter()
    collector = pydantic_monty.CollectString()
    record: dict[str, Any] = {
        "name": name,
        "code": code,
        "type_check": type_check,
    }
    try:
        monty = pydantic_monty.Monty(
            code,
            inputs=inputs_decl,
            type_check=type_check,
            type_check_stubs=type_check_stubs,
        )
    except Exception as exc:
        record.update({"status": "error", "phase": "construct", **describe_error(exc)})
        record["elapsed_ms"] = round((time.perf_counter() - t0) * 1000, 3)
        return record

    try:
        output = monty.run(
            inputs=inputs,
            limits=limits,
            external_functions=external_functions,
            print_callback=collector,
            mount=mount,
            os=os,
        )
    except Exception as exc:
        record.update({"status": "error", "phase": "run", **describe_error(exc)})
    else:
        record.update(
            {
                "status": "ok",
                "output_type": type(output).__name__,
                "output_repr": clipped(output),
            }
        )
    record["stdout"] = collector.output
    record["elapsed_ms"] = round((time.perf_counter() - t0) * 1000, 3)
    return record


def type_check_case(name: str, code: str, *, stubs: str | None = None) -> dict[str, Any]:
    t0 = time.perf_counter()
    record = {"name": name, "code": code, "stubs": stubs}
    try:
        monty = pydantic_monty.Monty(code)
        monty.type_check(stubs)
    except Exception as exc:
        record.update({"status": "error", **describe_error(exc)})
        if isinstance(exc, pydantic_monty.MontyTypingError):
            record["concise"] = exc.display("concise")
    else:
        record["status"] = "ok"
    record["elapsed_ms"] = round((time.perf_counter() - t0) * 1000, 3)
    return record


async def async_case(name: str, code: str, external_functions: dict[str, Callable[..., Any]]) -> dict[str, Any]:
    t0 = time.perf_counter()
    record = {"name": name, "code": code}
    collector = pydantic_monty.CollectString()
    try:
        monty = pydantic_monty.Monty(code)
        output = await monty.run_async(external_functions=external_functions, print_callback=collector)
    except Exception as exc:
        record.update({"status": "error", **describe_error(exc)})
    else:
        record.update({"status": "ok", "output_type": type(output).__name__, "output_repr": clipped(output)})
    record["stdout"] = collector.output
    record["elapsed_ms"] = round((time.perf_counter() - t0) * 1000, 3)
    return record


def syntax_and_runtime_cases() -> list[dict[str, Any]]:
    cases: list[tuple[str, str]] = [
        ("arithmetic_precedence", "1 + 2 * 3 - 4 // 2"),
        ("floats_and_round", "round(3.14159, 2)"),
        ("strings_and_fstrings", "name = 'Monty'\nf'hello {name.lower()} {2 + 3}'"),
        ("format_spec_simple", "f'{3.14159:.2f}'"),
        ("format_spec_comma_grouping", "f'{1000:,d}'"),
        ("format_spec_underscore_grouping", "f'{1000:_d}'"),
        ("format_spec_alternate_hex", "f'{255:#x}'"),
        ("bytes_literal_concat", "b'abc' + b'd'"),
        ("bytes_constructor_from_list", "bytes([100])"),
        ("list_dict_set_tuple_literals", "([1, 2], {'a': 1}, {1, 2}, (1, 2))"),
        ("slicing", "data = [0, 1, 2, 3, 4]\n(data[1:4], data[::-1])"),
        ("unpack_assignment", "a, *mid, z = [1, 2, 3, 4]\n(a, mid, z)"),
        ("augmented_assignment", "x = 1\nx += 4\nx"),
        ("walrus_expression", "(x := 4) + x"),
        ("if_else", "x = 4\n'even' if x % 2 == 0 else 'odd'"),
        ("for_range", "total = 0\nfor i in range(10):\n    total += i\ntotal"),
        ("while_break_continue", "i = 0\nout = []\nwhile True:\n    i += 1\n    if i % 2 == 0:\n        continue\n    out.append(i)\n    if i >= 5:\n        break\nout"),
        ("try_except_finally", "log = []\ntry:\n    1 / 0\nexcept ZeroDivisionError:\n    log.append('caught')\nfinally:\n    log.append('finally')\nlog"),
        ("assert_caught", "try:\n    assert False, 'boom'\nexcept AssertionError as e:\n    result = str(e)\nresult"),
        ("function_def_recursion", "def fib(n):\n    if n <= 1:\n        return n\n    return fib(n - 1) + fib(n - 2)\nfib(10)"),
        ("default_args", "def add(a, b=10):\n    return a + b\n(add(1), add(1, 2))"),
        ("keyword_args", "def join(a, b, sep=':'):\n    return f'{a}{sep}{b}'\njoin(b='right', a='left', sep='|')"),
        ("varargs", "def f(*args):\n    return args\nf(1, 2, 3)"),
        ("kwargs", "def f(**kwargs):\n    return kwargs\nf(a=1, b=2)"),
        ("lambda_expr", "(lambda x: x + 1)(4)"),
        ("list_comprehension", "[x * x for x in range(6) if x % 2 == 0]"),
        ("dict_comprehension", "{str(x): x * 2 for x in range(3)}"),
        ("set_comprehension", "{x % 3 for x in range(10)}"),
        ("generator_expression_sum", "sum(x * x for x in range(5))"),
        ("nested_comprehension", "[(x, y) for x in range(3) for y in range(2)]"),
        ("enumerate_zip", "list(zip(range(3), ['a', 'b', 'c']))"),
        ("map_filter", "list(map(str, filter(lambda x: x % 2, range(6))))"),
        ("sort_key_lambda", "sorted(['aaa', 'b', 'cc'], key=lambda s: len(s))"),
        ("any_all", "(any([False, 0, 'x']), all([1, True, 'x']))"),
        ("min_max", "(min([3, 1, 2]), max([3, 1, 2]))"),
        ("abs_pow_divmod", "(abs(-4), pow(2, 8), divmod(17, 5))"),
        ("isinstance_type_repr", "(isinstance(1, int), type('x') is str, repr([1, 2]))"),
        ("hash_builtin", "hash(('a', 1))"),
        ("id_builtin", "id(object())"),
        ("dir_builtin", "dir(1)"),
        ("locals_builtin", "locals()"),
        ("getattr_builtin", "getattr('abc', 'upper')()"),
        ("setattr_builtin", "setattr(1, 'x', 2)"),
        ("callable_builtin", "(callable(len), callable(1))"),
        ("dict_methods", "d = {'a': 1}\nd.setdefault('b', 2)\n(d.get('a'), sorted(d.items()))"),
        ("string_methods", "' a,b,c '.strip().replace(',', '|').split('|')"),
        ("class_definition", "class Point:\n    pass\nPoint()"),
        ("match_statement", "x = 1\nmatch x:\n    case 1:\n        'one'\n    case _:\n        'other'"),
        ("with_statement", "with open('/tmp/anything') as f:\n    f.read()"),
        ("yield_expression", "def gen():\n    yield 1\ngen()"),
        ("complex_number", "1 + 2j"),
        ("matrix_multiply", "1 @ 2"),
        ("eval_builtin", "eval('1 + 2')"),
        ("exec_builtin", "exec('x = 1')"),
        ("globals_builtin", "globals()"),
    ]
    return [run_case(name, code) for name, code in cases]


def import_and_stdlib_cases() -> list[dict[str, Any]]:
    cases = [
        ("sys_version_info", "import sys\n(sys.version_info.major, sys.version_info.minor, sys.version_info.micro)"),
        ("typing_type_checking", "from typing import TYPE_CHECKING\nTYPE_CHECKING"),
        ("typing_namedtuple_import", "from typing import NamedTuple\nrepr(NamedTuple)"),
        ("re_findall", "import re\nre.findall(r'\\d+', 'a12 b34')"),
        ("re_match_groups", "import re\nm = re.match(r'(\\d+)-(\\w+)', '123-abc')\nm.groups()"),
        ("json_loads_dumps", "import json\nobj = json.loads('{\"a\": [1, 2]}')\njson.dumps(obj)"),
        ("datetime_construct", "import datetime\ndatetime.date(2026, 5, 22).isoformat()"),
        ("datetime_now_without_os", "import datetime\ndatetime.datetime.now()"),
        ("os_getenv_without_os", "import os\nos.getenv('HOME')"),
        ("os_environ_without_os", "import os\nos.environ"),
        ("pathlib_exists_without_os", "from pathlib import Path\nPath('/etc/passwd').exists()"),
        ("open_builtin", "open('/etc/passwd').read()"),
        ("math_import", "import math\nmath.sqrt(9)"),
        ("statistics_import", "import statistics\nstatistics.mean([1, 2, 3])"),
        ("random_import", "import random\nrandom.random()"),
        ("socket_import", "import socket\nsocket.socket()"),
        ("subprocess_import", "import subprocess\nsubprocess.run(['echo', 'hi'])"),
        ("third_party_import", "import pydantic\npydantic.__version__"),
    ]
    return [run_case(name, code) for name, code in cases]


def input_conversion_cases() -> list[dict[str, Any]]:
    @dataclasses.dataclass
    class Person:
        name: str
        age: int

    return [
        run_case("plain_inputs", "x + y", inputs_decl=["x", "y"], inputs={"x": 2, "y": 3}),
        run_case("nested_inputs", "data['items'][1]['n']", inputs_decl=["data"], inputs={"data": {"items": [{"n": 1}, {"n": 2}]}}),
        run_case("set_input_roundtrip", "x", inputs_decl=["x"], inputs={"x": {1, 2, 3}}),
        run_case("bytes_input_roundtrip", "x + b'!'", inputs_decl=["x"], inputs={"x": b"hello"}),
        run_case("big_int_input", "x * 2", inputs_decl=["x"], inputs={"x": 2**100}),
        run_case("dataclass_field_access", "p.name + ':' + str(p.age)", inputs_decl=["p"], inputs={"p": Person("Ada", 36)}),
        run_case("unsupported_input_regex", "x", inputs_decl=["x"], inputs={"x": __import__("re").compile("x")}),
        run_case("missing_required_input", "x + y", inputs_decl=["x", "y"], inputs={"x": 1}),
        run_case("unexpected_inputs", "1 + 1", inputs={"x": 1}),
    ]


def external_and_snapshot_cases() -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []

    def host_add(a: int, b: int) -> int:
        return a + b

    def fail_value() -> None:
        raise ValueError("from host")

    def host_read_len(path: str) -> int:
        return len(Path(path).read_text())

    records.append(run_case("external_function_sync", "host_add(2, 3)", external_functions={"host_add": host_add}))
    records.append(
        run_case(
            "external_function_exception_caught",
            "try:\n    fail_value()\nexcept ValueError as e:\n    result = str(e)\nresult",
            external_functions={"fail_value": fail_value},
        )
    )
    records.append(run_case("external_function_missing", "missing_tool(1)", external_functions={}))
    records.append(
        run_case(
            "external_function_can_read_host_when_exposed",
            "host_read_len('/etc/hosts') > 0",
            external_functions={"host_read_len": host_read_len},
        )
    )

    t0 = time.perf_counter()
    record: dict[str, Any] = {"name": "start_resume_snapshot", "code": "data = fetch(url)\nlen(data)"}
    try:
        monty = pydantic_monty.Monty(record["code"], inputs=["url"])
        snap = monty.start(inputs={"url": "https://example.test/data"})
        record["first_snapshot_type"] = type(snap).__name__
        record["function_name"] = getattr(snap, "function_name", None)
        record["args_repr"] = clipped(getattr(snap, "args", None))
        dumped = snap.dump()
        record["snapshot_dump_bytes"] = len(dumped)
        restored = pydantic_monty.load_snapshot(dumped)
        done = restored.resume({"return_value": "abcdef"})
        record["status"] = "ok"
        record["complete_type"] = type(done).__name__
        record["output_repr"] = clipped(done.output)
    except Exception as exc:
        record.update({"status": "error", **describe_error(exc)})
    record["elapsed_ms"] = round((time.perf_counter() - t0) * 1000, 3)
    records.append(record)

    t0 = time.perf_counter()
    record = {"name": "monty_dump_load", "code": "x + 1"}
    try:
        monty = pydantic_monty.Monty("x + 1", inputs=["x"])
        blob = monty.dump()
        restored = pydantic_monty.Monty.load(blob)
        record.update(
            {
                "status": "ok",
                "dump_bytes": len(blob),
                "output_repr": clipped(restored.run(inputs={"x": 41})),
            }
        )
    except Exception as exc:
        record.update({"status": "error", **describe_error(exc)})
    record["elapsed_ms"] = round((time.perf_counter() - t0) * 1000, 3)
    records.append(record)

    return records


async def async_cases() -> list[dict[str, Any]]:
    async def fetch(value: str) -> str:
        await asyncio.sleep(0.01)
        return value.upper()

    async def slow(value: int) -> int:
        await asyncio.sleep(0.01)
        return value * 2

    return [
        await async_case("await_async_external", "await fetch('abc')", {"fetch": fetch}),
        await async_case(
            "asyncio_gather_external",
            "import asyncio\nawait asyncio.gather(slow(2), slow(3), slow(4))",
            {"slow": slow},
        ),
    ]


def filesystem_sandbox_cases() -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []

    fs = pydantic_monty.OSAccess(
        [pydantic_monty.MemoryFile("/data/input.txt", content="alpha\nbeta\n")],
        environ={"SECRET": "mounted-env"},
    )
    records.append(run_case("osaccess_read_memory_file", "from pathlib import Path\nPath('/data/input.txt').read_text()", os=fs))
    records.append(run_case("osaccess_write_memory_file", "from pathlib import Path\nPath('/data/new.txt').write_text('hello')", os=fs))
    records.append(run_case("osaccess_getenv_allowed", "import os\nos.getenv('SECRET')", os=fs))
    records.append(run_case("osaccess_missing_env", "import os\nos.getenv('HOME')", os=fs))
    records.append(run_case("osaccess_no_network_import", "import socket\nsocket.gethostbyname('example.com')", os=fs))

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "hello.txt").write_text("hello from host")
        (root / "subdir").mkdir()
        (root / "subdir" / "nested.txt").write_text("nested")

        ro = pydantic_monty.MountDir("/mnt", root, mode="read-only")
        records.append(run_case("mount_read_only_read", "from pathlib import Path\nPath('/mnt/hello.txt').read_text()", mount=ro))
        records.append(run_case("mount_read_only_write_blocked", "from pathlib import Path\nPath('/mnt/new.txt').write_text('x')", mount=ro))
        records.append(run_case("mount_path_traversal_blocked", "from pathlib import Path\nPath('/mnt/../../etc/passwd').read_text()", mount=ro))
        records.append(run_case("mount_unmounted_path_blocked", "from pathlib import Path\nPath('/other/place').exists()", mount=ro))

        overlay = pydantic_monty.MountDir("/mnt", root, mode="overlay")
        records.append(run_case("mount_overlay_write_read", "from pathlib import Path\nPath('/mnt/overlay.txt').write_text('shadow')\nPath('/mnt/overlay.txt').read_text()", mount=overlay))
        records.append({"name": "mount_overlay_host_unchanged", "status": "ok", "host_exists": (root / "overlay.txt").exists()})

        rw = pydantic_monty.MountDir("/mnt", root, mode="read-write")
        records.append(run_case("mount_read_write_writes_host", "from pathlib import Path\nPath('/mnt/rw.txt').write_text('persist')", mount=rw))
        records.append({"name": "mount_read_write_host_changed", "status": "ok", "host_content": (root / "rw.txt").read_text()})

    return records


def resource_limit_cases() -> list[dict[str, Any]]:
    return [
        run_case(
            "timeout_infinite_loop",
            "while True:\n    pass",
            limits={"max_duration_secs": 0.05},
        ),
        run_case(
            "timeout_builtin_sum_huge_range",
            "sum(range(10**18))",
            limits={"max_duration_secs": 0.05},
        ),
        run_case(
            "memory_limit_list_growth",
            "xs = []\nfor i in range(1000):\n    xs.append('x' * 100)\nlen(xs)",
            limits={"max_memory": 100},
        ),
        run_case(
            "allocation_limit_many_lists",
            "xs = []\nfor i in range(1000):\n    xs.append([i])\nlen(xs)",
            limits={"max_allocations": 10},
        ),
        run_case(
            "recursion_limit",
            "def recurse(n):\n    if n <= 0:\n        return 0\n    return 1 + recurse(n - 1)\nrecurse(20)",
            limits={"max_recursion_depth": 5},
        ),
        run_case(
            "bigint_pow_memory_limit",
            "2 ** 10000000",
            limits={"max_memory": 1_000_000},
        ),
        run_case(
            "normal_work_with_limits",
            "sum(range(1000))",
            limits={"max_duration_secs": 1.0, "max_memory": 1_000_000, "max_allocations": 100_000},
        ),
    ]


def type_check_cases() -> list[dict[str, Any]]:
    return [
        type_check_case("type_check_valid", "def add(a: int, b: int) -> int:\n    return a + b\nadd(1, 2)"),
        type_check_case("type_check_bad_operator", '"hello" + 1'),
        type_check_case("type_check_bad_return", "def f() -> int:\n    return 'x'"),
        type_check_case("type_check_undefined", "missing_name + 1"),
        type_check_case("type_check_with_stub", "result = external(1)", stubs="def external(x: int) -> str: ..."),
        type_check_case("type_check_stub_mismatch", "result = external('bad')", stubs="def external(x: int) -> str: ..."),
    ]


def repl_cases() -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    repl = pydantic_monty.MontyRepl()
    steps = [
        ("repl_define", "x = 10"),
        ("repl_use_previous", "x + 5"),
        ("repl_define_function", "def double(v):\n    return v * 2"),
        ("repl_call_function", "double(x)"),
        ("repl_state_after_error", "1 / 0"),
        ("repl_still_alive", "x"),
    ]
    for name, code in steps:
        t0 = time.perf_counter()
        record = {"name": name, "code": code}
        collector = pydantic_monty.CollectString()
        try:
            output = repl.feed_run(code, print_callback=collector)
        except Exception as exc:
            record.update({"status": "error", **describe_error(exc)})
        else:
            record.update({"status": "ok", "output_type": type(output).__name__, "output_repr": clipped(output)})
        record["stdout"] = collector.output
        record["elapsed_ms"] = round((time.perf_counter() - t0) * 1000, 3)
        records.append(record)
    return records


def run_all() -> dict[str, Any]:
    package_info = {
        "pydantic_monty_version": pydantic_monty.__version__,
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
    }
    results = {
        "package": package_info,
        "syntax_and_runtime": syntax_and_runtime_cases(),
        "imports_and_stdlib": import_and_stdlib_cases(),
        "input_conversion": input_conversion_cases(),
        "external_and_snapshots": external_and_snapshot_cases(),
        "async_external": asyncio.run(async_cases()),
        "filesystem_sandbox": filesystem_sandbox_cases(),
        "resource_limits": resource_limit_cases(),
        "type_checking": type_check_cases(),
        "repl": repl_cases(),
    }
    return results


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", default=str(ROOT / "results.json"))
    args = parser.parse_args()
    results = run_all()
    output_path = Path(args.json)
    output_path.write_text(json.dumps(results, indent=2, sort_keys=True) + "\n")

    total = 0
    ok = 0
    for section, records in results.items():
        if section == "package":
            continue
        assert isinstance(records, list)
        total += len(records)
        ok += sum(1 for r in records if r.get("status") == "ok")
    print(f"wrote {output_path}")
    print(f"cases: {total}, ok: {ok}, errors: {total - ok}")


if __name__ == "__main__":
    main()
