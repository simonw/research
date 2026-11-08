import sqlite3, sys

EXTS = {
    "c": "c/parse_number.so",
    "c-fast": "c-fast/parse_number.so",
    "rusqlite": "rust-rusqlite/target/release/libparse_number.so",
    "sqlite-loadable": "rust-loadable/target/release/libparse_number.so",
}

CASES = [
    ("1234", 1234), ("1,234", 1234), ("$1,234.50", 1234.5), ("£99", 99),
    ("£1,000,000", 1000000), ("45%", 45), ("12.5%", 12.5), ("-5", -5),
    ("-$1,234.56", -1234.56), ("$-5", -5), ("  42  ", 42), ("0.5", 0.5),
    (".5", 0.5), ("5.", 5.0), ("007", 7),
    ("9223372036854775807", 9223372036854775807),
    ("-9223372036854775808", -9223372036854775808),
    ("9223372036854775808", 9.223372036854776e18),
    ("123456789012345678901234567890", 1.2345678901234568e29),
    ("1" * 100 + ".5", float("1" * 100 + ".5")),
    # NULLs
    ("", None), ("   ", None), ("abc", None), ("-", None), ("$", None),
    (",,,", None), (".", None), ("--5", None), ("5-", None), ("1.2.3", None),
    ("1e5", None), ("+5", None), ("12abc", None), ("1 000", None), ("€5", None),
    ("Â", None), (None, None),
    # non-text passthrough
    (42, 42), (3.14, 3.14), (b"123", None),
]

ok = True
for name, path in EXTS.items():
    db = sqlite3.connect(":memory:")
    db.enable_load_extension(True)
    db.load_extension(path)
    fails = []
    for inp, want in CASES:
        got, typ = db.execute("select parse_number(?), typeof(parse_number(?))", (inp, inp)).fetchone()
        want_typ = {int: "integer", float: "real", type(None): "null"}[type(want)]
        if got != want or typ != want_typ:
            fails.append((inp, want, got, typ))
    print(f"{name:16} {len(CASES)-len(fails)}/{len(CASES)} passed")
    for f in fails:
        ok = False
        print("   FAIL", f)
sys.exit(0 if ok else 1)
