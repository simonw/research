import sqlite3, random, struct, sys, re

EXTS = {
    "c": "c/parse_number.so",
    "c-fast": "c-fast/parse_number.so",
    "rusqlite": "rust-rusqlite/target/release/libparse_number.so",
    "sqlite-loadable": "rust-loadable/target/release/libparse_number.so",
}

def reference(s):
    t = s.strip(" \t\n\r")
    t = re.sub("[,$%£]", "", t)
    if not re.fullmatch(r"-?\d*\.?\d*", t) or not re.search(r"\d", t):
        return None
    if "." not in t:
        n = int(t)
        if -2**63 <= n < 2**63:
            return n
    return float(t)

def key(v):  # bitwise identity, distinguishes -0.0 and int vs float
    if isinstance(v, float): return ("f", struct.pack("<d", v))
    return (type(v).__name__, v)

rnd = random.Random(int(sys.argv[1]) if len(sys.argv) > 1 else 0)
def gen():
    r = rnd.random()
    if r < 0.4:   # random junk from the relevant alphabet
        return "".join(rnd.choice("0123456789.-,$%£ x") for _ in range(rnd.randint(0, 12)))
    if r < 0.8:   # decimals with 1..25 significant digits and varied scale
        digits = "".join(rnd.choice("0123456789") for _ in range(rnd.randint(1, 25)))
        p = rnd.randint(0, len(digits))
        s = digits[:p] + "." + digits[p:]
        if rnd.random() < 0.3: s = "0." + "0" * rnd.randint(0, 25) + digits
        return rnd.choice(["", "-", "$", "-$", "£"]) + s + rnd.choice(["", "%", "  "])
    # exact doubles near 2^53 and halfway cases, written out
    m = rnd.randint(2**52, 2**54)
    k = rnd.randint(0, 25)
    s = str(m).rjust(k + 1, "0")
    return s[:-k] + "." + s[-k:] if k else s

N = 500_000
inputs = [gen() for _ in range(N)]
want = [key(reference(s)) for s in inputs]
bad = 0
for name, path in EXTS.items():
    db = sqlite3.connect(":memory:")
    db.enable_load_extension(True)
    db.load_extension(path)
    db.execute("create table t(i integer primary key, v text)")
    db.executemany("insert into t(v) values (?)", ((s,) for s in inputs))
    got = [key(r[0]) for r in db.execute("select parse_number(v) from t order by i")]
    mism = [(inputs[i], want[i], got[i]) for i in range(N) if got[i] != want[i]]
    bad += len(mism)
    print(f"{name:10} {N - len(mism)}/{N} match reference", *mism[:3], sep="\n  " if mism else "")
sys.exit(1 if bad else 0)
