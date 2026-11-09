"""Run untrusted JavaScript with hard CPU, wall-clock and memory limits,
and a couple of Python functions exposed to the sandbox."""
import time

import wasmi_sandbox as ws

js = ws.QuickJS(
    max_memory=32 << 20,   # wasm linear memory cap (bytes)
    timeout=2.0,           # wall-clock limit per eval (seconds)
)
# Deterministic CPU budget, refilled per eval. Note wasmi charges QuickJS's
# bytecode loop very heavily (see README), so budgets are large numbers.
FUEL_PER_EVAL = 50_000_000_000


@js.function
def lookup_user(user_id):
    """Called from JS as host.lookup_user(id)."""
    return {"id": user_id, "name": f"user-{user_id}", "admin": user_id == 1}


@js.function
def now():
    return time.time()


print(js.eval("""
const u = host.lookup_user(1);
console.log("hello from JS, user is", JSON.stringify(u));
[1, 2, 3].map(x => x * x).concat([u.admin, typeof host.now()])
"""))
print("stdout:", repr(js.take_output()))

for label, code in [
    ("infinite loop", "while (true) {}"),
    ("memory bomb", "let a=[]; for(;;) a.push('x'.repeat(1<<16))"),
    ("deep recursion", "function f(){ return f() + 1 }; f()"),
    ("no filesystem", "typeof std + ' ' + typeof os + ' ' + typeof require"),
]:
    t = time.time()
    try:
        print(f"{label:16} ->", js.eval(code, fuel=FUEL_PER_EVAL))
    except ws.JSError as e:
        print(f"{label:16} -> JSError: {e.args[0].splitlines()[0]}")
    except (ws.Timeout, ws.OutOfFuel, ws.Trap) as e:
        print(f"{label:16} -> {type(e).__name__}: {e}")
    print(f"{'':16}    ({time.time() - t:.2f}s, fuel used {FUEL_PER_EVAL - js.sb.fuel:,}, wasm memory {js.memory_size >> 20} MB)")
