"""Run untrusted Python (MicroPython) with hard limits and host functions."""
import time

import wasmi_sandbox as ws

mp = ws.MicroPython(max_memory=32 << 20, timeout=5.0)
FUEL_PER_EXEC = 20_000_000_000  # refilled before each exec


@mp.function
def get_prices(*symbols):
    return {s: len(s) * 10.5 for s in symbols}


print(mp.exec("""
import host
prices = host.call("get_prices", "AAPL", "GOOG")
total = sum(prices.values())
print("prices:", prices, "total:", total)
class Account:
    def __init__(self, balance):
        self.balance = balance
    def withdraw(self, amount):
        if amount > self.balance:
            raise ValueError("insufficient funds")
        self.balance -= amount
a = Account(100)
try:
    a.withdraw(500)
except ValueError as e:
    print("caught:", e)
print(sorted({x % 7 for x in range(100)}))
"""))

for label, code in [
    ("infinite loop", "while True:\n    pass"),
    ("memory bomb", "a = []\nwhile True:\n    a.append('x' * 65536)"),
    ("deep recursion", "def f():\n    return f() + 1\nf()"),
    ("filesystem", "open('/etc/passwd').read()"),
    ("imports", "import os"),
]:
    t = time.time()
    try:
        print(f"{label:16} ->", repr(mp.exec(code, fuel=FUEL_PER_EXEC)))
    except ws.PythonError as e:
        print(f"{label:16} -> PythonError: {e.args[0].strip().splitlines()[-1]}")
    except (ws.Timeout, ws.OutOfFuel, ws.Trap) as e:
        print(f"{label:16} -> {type(e).__name__}: {e}")
    print(f"{'':16}    ({time.time() - t:.2f}s, fuel used {FUEL_PER_EXEC - mp.sb.fuel:,}, wasm memory {mp.memory_size >> 20} MB, invokes {mp.sjlj.invokes:,})")
