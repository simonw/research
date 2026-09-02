import time

import pytest

import wasmi_sandbox as ws


@pytest.fixture
def js():
    return ws.QuickJS(max_memory=32 << 20, timeout=5)


def test_js_eval_values(js):
    assert js.eval("1 + 2") == 3
    assert js.eval("({a: [1, 2, 3], b: 'hi'})") == {"a": [1, 2, 3], "b": "hi"}
    assert js.eval("'hello'.toUpperCase()") == "HELLO"
    assert js.eval("undefined") == "undefined"


def test_js_output_and_host_functions(js):
    js.eval("console.log('hi', 42); print({x: 1})")
    assert js.take_output() == "hi 42\n[object Object]\n"

    @js.function
    def add(a, b):
        return a + b

    js.register("lookup", lambda key: {"key": key, "value": key[::-1]})
    assert js.eval("host.add(40, 2)") == 42
    assert js.eval("host.lookup('abc')") == {"key": "abc", "value": "cba"}
    with pytest.raises(ws.JSError, match="no host function"):
        js.eval("host.missing()")
    # host exceptions become JS exceptions the guest can catch
    js.register("fail", lambda: 1 / 0)
    assert js.eval("try { host.fail() } catch (e) { 'caught: ' + e.message }").startswith("caught: ZeroDivisionError")


def test_js_exception(js):
    with pytest.raises(ws.JSError, match="TypeError"):
        js.eval("null.x")


def test_js_hard_timeout(js):
    t = time.time()
    with pytest.raises(ws.Timeout):
        js.eval("while (true) {}", timeout=0.3)
    assert time.time() - t < 2
    assert js.eval("1 + 1") == 2  # runtime survives (state may be inconsistent in general)


def test_js_soft_timeout(js):
    with pytest.raises(ws.JSError, match="interrupted"):
        js.eval("while (true) {}", soft_timeout=0.2)


def test_js_fuel():
    js = ws.QuickJS(fuel=20_000_000)
    with pytest.raises(ws.OutOfFuel):
        js.eval("function fib(n){return n<2?n:fib(n-1)+fib(n-2)}; fib(30)")


def test_js_memory_limit():
    js = ws.QuickJS(max_memory=16 << 20)
    with pytest.raises(ws.JSError, match="out of memory"):
        js.eval("let a = []; while (true) { a.push('x'.repeat(1 << 16)) }")
    assert js.memory_size <= 16 << 20


def test_js_no_filesystem_or_network(js):
    # Nothing but the intentional globals exists.
    assert js.eval("typeof require") == "undefined"
    assert js.eval("typeof std") == "undefined"
    assert js.eval("typeof os") == "undefined"
    assert js.eval("typeof fetch") == "undefined"


@pytest.fixture
def mp():
    return ws.MicroPython(max_memory=32 << 20, timeout=10)


def test_mp_exec(mp):
    assert mp.exec("print('hi', 1 + 2, [x * x for x in range(4)])") == "hi 3 [0, 1, 4, 9]\n"
    assert mp.exec("x = 5") == ""
    assert mp.exec("print(x * 2)") == "10\n"  # globals persist between execs


def test_mp_exceptions(mp):
    assert mp.exec("try:\n    1/0\nexcept ZeroDivisionError as e:\n    print('caught', e)") == "caught divide by zero\n"
    with pytest.raises(ws.PythonError, match="ValueError: boom"):
        mp.exec("raise ValueError('boom')")
    assert mp.sjlj.unwinds >= 2
    assert mp.exec("print('still alive')") == "still alive\n"


def test_mp_host_functions(mp):
    mp.register("add", lambda a, b: a + b)
    mp.register("info", lambda: {"answer": 42, "items": [1, "two"]})
    assert mp.exec("import host\nprint(host.call('add', 40, 2))\nprint(host.call('info')['items'][1])") == "42\ntwo\n"
    with pytest.raises(ws.PythonError, match="no host function"):
        mp.exec("import host\nhost.call('nope')")
    out = mp.exec("import host\ntry:\n    host.call('nope')\nexcept RuntimeError as e:\n    print('caught', e)")
    assert out.startswith("caught NameError")


def test_mp_timeout(mp):
    t = time.time()
    with pytest.raises(ws.Timeout):
        mp.exec("while True:\n    pass", timeout=0.5)
    assert time.time() - t < 3


def test_mp_fuel():
    mp = ws.MicroPython(fuel=3_000_000)
    with pytest.raises(ws.OutOfFuel):
        mp.exec("x = 0\nwhile True:\n    x += 1")


def test_mp_memory_limit():
    mp = ws.MicroPython(max_memory=8 << 20)
    with pytest.raises(ws.PythonError, match="MemoryError"):
        mp.exec("a = []\nwhile True:\n    a.append('x' * 65536)")
    assert mp.memory_size <= 8 << 20


def test_mp_no_filesystem(mp):
    with pytest.raises(ws.PythonError, match="OSError"):
        mp.exec("open('/etc/passwd')")
    with pytest.raises(ws.PythonError, match="ImportError"):
        mp.exec("import os")


def test_mp_unbounded_recursion_is_caught_in_guest(mp):
    out = mp.exec("def f():\n    return f() + 1\ntry:\n    f()\nexcept RuntimeError as e:\n    print('caught:', e)")
    assert out == "caught: maximum recursion depth exceeded\n"
    assert mp.sjlj.stack_guard_hits >= 1
    assert mp.exec("print(sum(range(10)))") == "45\n"


def test_mp_python_trampolines_reference_implementation():
    mp = ws.MicroPython(native_sjlj=False, timeout=30)
    assert mp.exec("try:\n    1/0\nexcept ZeroDivisionError as e:\n    print('caught', e)") == "caught divide by zero\n"
    assert mp.sjlj.invokes > 0 and mp.sjlj.unwinds >= 1


def test_js_deep_recursion_is_a_catchable_range_error(js):
    out = js.eval("let d = 0; function f() { d++; return f() + 1 }; try { f() } catch (e) { e.name }")
    assert out == "RangeError"
    assert js.eval("d > 100") is True
