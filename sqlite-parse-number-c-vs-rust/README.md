# `parse_number()` as a SQLite extension: C vs Rust

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This compares a SQLite scalar function `parse_number(value)` written in C and in
Rust. The function removes `,` `$` `£` `%`, handles a leading `-` and returns
`NULL` for anything it can't parse:

```sql
select parse_number('-$1,234.56');  -- -1234.56
select parse_number('£1,000,000');  -- 1000000 (INTEGER)
select parse_number('12.5%');       -- 12.5
select parse_number('n/a');         -- NULL
```

I built four versions with identical behaviour, then compared speed, code
size and `.so` size:

| Version | What it is |
|---|---|
| [`c/`](c/parse_number.c) | Straightforward C. Decimals go through `strtod()`. |
| [`c-fast/`](c-fast/parse_number.c) | C with its own correctly rounded decimal fast path |
| [`rust-rusqlite/`](rust-rusqlite/src/lib.rs) | Rust, [rusqlite](https://crates.io/crates/rusqlite) 0.41 with `loadable_extension` |
| [`rust-loadable/`](rust-loadable/src/lib.rs) | Rust, [sqlite-loadable](https://crates.io/crates/sqlite-loadable) 0.0.6-alpha.6 |

The two Rust versions share the parser in [`rust-core/parse.rs`](rust-core/parse.rs).

## Key findings

- **Plain C lost to Rust on speed.** It took 67 ns per row on mixed data vs
  41–46 ns. It was faster on integers but 2× slower on decimals, because
  glibc's `strtod` is much slower than Rust's `f64::from_str`.
- **A ~30-line C fast path fixed that.** It runs at 16–19 ns per row on every
  dataset, about 2× faster than either Rust version, while staying
  bit-for-bit identical (8M fuzz checks).
- **C is 20× smaller**: 14 KB vs ~300 KB. Even with `opt-level = "z"` the Rust
  files stay at 284–306 KB, because they carry Rust's standard library.
- **Rust is shorter and safer to write**: 30–36 lines plus a 41-line shared
  parser, vs 75 (plain C) or 104 (fast C) lines with manual buffers and
  overflow checks.
- **All four are 5–20× faster than a nested `replace()` in pure SQL**, which
  doesn't even enforce the rules.

## Rules

The request left some edge cases open, so I picked these and used them for every version:

- `INTEGER` and `REAL` values are returned unchanged. `NULL` and `BLOB` give `NULL`.
- Spaces, tabs and newlines at either end are ignored. `,` `$` `£` `%` are removed anywhere.
- One `-` is allowed before the first digit (`-$5` and `$-5` both work). One `.`
  is allowed, and at least one digit is required.
- No `.` gives an `INTEGER`, or a `REAL` if it's too big for 64 bits. With a `.` the result is a `REAL`.
- `1e5`, `+5`, `1 000`, `€5`, `5-` and `--5` all give `NULL`. `%` is only removed (`45%` → 45).

## Performance

Each extension is loaded into Python's `sqlite3` (SQLite 3.53.1) and runs
`select sum(parse_number(v)) from t` over 2 million rows, so the whole loop
runs inside SQLite. Runs alternate between versions; each figure is the
fastest of 15 runs, minus a baseline `sum(length(v))` scan of the same rows.
That leaves the net cost of the function call plus the parsing. Machine: 2 vCPU
Linux x86-64 VM, gcc 13.3, rustc 1.97.

Net nanoseconds per row (lower is better):

| Dataset | plain C | **fast C** | rusqlite | sqlite-loadable | pure SQL `replace()` |
|---|---|---|---|---|---|
| Mixed (`$1,234.56`, `£1,000`, `12.5%`, ints, 10% junk) | 67.3 | **19.2** | 45.9 | 40.5 | 328.9 |
| Integers only (`$-123,456`) | 25.1 | **15.8** | 36.4 | 32.1 | 342.5 |
| Decimals only (`$-123,456.78`) | 106.3 | **17.5** | 54.5 | 46.5 | 356.9 |

Raw output: [`results/`](results/).

### Why plain C was slow, and how fast C wins

Plain C copied the cleaned characters into a buffer and called `strtod()`.
glibc's `strtod` is general purpose and depends on the locale. Rust's float
parser first tries a quick exact calculation (*Clinger's fast path*), then uses
Eisel-Lemire.

`c-fast` does that same quick calculation inside the scanning loop it already runs:

```c
/* while scanning: mant = first <=19 significant digits, frac = digits after '.' */
}else if( FAST_PATH_OK && nsig<=19 && mant <= ((uint64_t)1<<53) && frac<=22 ){
  double v = (double)mant / POW10[frac];   /* both exact -> one correctly rounded op */
  sqlite3_result_double(ctx, neg ? -v : v);
  return;
}
parse_slow(ctx, s, start, end);            /* rebuild buffer + strtod, rarely taken */
```

When the digits fit in 2^53 and there are at most 22 after the point, both
numbers are exact doubles, so a single IEEE division gives exactly the answer
`strtod` would. Longer inputs fall back to `strtod`. Integers also stopped
writing to a buffer at all. Two factors made Rust slower than `c-fast` on this
work: rusqlite's wrapping of each call (closure, `Result`, `Value`) and the
`Vec` both Rust versions allocate per call. Replacing that `Vec` with a stack
buffer only moved rusqlite from 35 to 31 ns on integers.

## Size and complexity

| | plain C | fast C | rusqlite | sqlite-loadable |
|---|---|---|---|---|
| Stripped `.so` | 14,408 B | 14,408 B | 320,864 B | 298,464 B |
| …with size optimisation (`-Os` / `opt-level="z"`) | 14,408 B | – | 305,768 B | 284,264 B |
| Code + data actually in the file (`size`) | 3.1 KB | 3.5 KB | 323 KB | 297 KB |
| Lines of code (no blanks or comments) | 75 | 104 | 36 + 41 shared | 30 + 41 shared |
| Dependencies | SQLite header | SQLite header | 20 crates | 62 crates |
| Build from scratch | 0.1 s | 0.1 s | 11 s | 72 s (bindgen, needs libclang) |
| Shared libraries it needs | libc | libc | libc, libgcc_s, ld-linux | libc, libgcc_s, ld-linux |

The C files are ~3 KB of actual content; the rest of the 14 KB is ELF
page alignment. The Rust files carry Rust's standard library, formatting and
panic code, even for one tiny function.

Notes on writing each one:

- **Rust** handles memory and slices for you and gets integer overflow handling
  from `str::parse::<i64>`. rusqlite's `ValueRef` enum makes the type handling
  one clear `match`. sqlite-loadable is a little nicer to write, but it's still
  an alpha.
- **C** needs hand-written buffer handling, overflow checks, an `INT64_MIN`
  special case and, in the fast version, care with floating-point details (the
  fast path is disabled unless `FLT_EVAL_METHOD == 0`). In return it's one file,
  has no dependencies and builds in a tenth of a second.
- Caveat for both C versions: `strtod` reads the decimal point according to the
  host program's locale. In `c-fast` that only matters for the rare fallback path.

## Correctness

- [`test.py`](test.py): 40 hand-written cases, checking both the value and
  `typeof()`, run against every extension. All pass.
- [`fuzz.py`](fuzz.py): compares every extension bit for bit (including `-0.0`
  and integer vs real) against a Python reference that uses `float()`, which is
  correctly rounded. Inputs are random junk, decimals with 1–25 significant
  digits, and exact doubles near 2^53 written out in full. Across 4 seeds ×
  500k inputs × 4 extensions there were **no mismatches**
  ([`results/fuzz.txt`](results/fuzz.txt)).

## Reproduce

```bash
./build.sh            # gcc + cargo; sqlite-loadable needs libclang
python test.py
python fuzz.py 1      # seed
python bench.py mixed # or: int, real
```

Or build just the fast C version:

```bash
gcc -O2 -fPIC -shared c-fast/parse_number.c -o parse_number.so
```

```sql
.load ./parse_number
select parse_number('$1,234.50');
```

See [`notes.md`](notes.md) for the working notes.
