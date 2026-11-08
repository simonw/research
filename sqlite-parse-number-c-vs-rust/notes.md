# Notes: parse_number() SQLite extension, C vs Rust

## Goal

Build a SQLite scalar function `parse_number(value)` that removes `,` `$` `£` `%`,
handles a leading `-` and returns `NULL` for anything it can't parse. Build it
in C and in Rust, then compare performance, code complexity and `.so` size.

## Environment

- Linux x86-64, 2 vCPUs (shared cloud VM, so timings are noisy)
- gcc 13.3.0, rustc 1.97.0
- SQLite 3.53.1, via Python's `sqlite3` module, used as the host to load the
  extensions (the `sqlite3` CLI wasn't installed)

## The rules I settled on

The request left some edge cases open, so I picked these and used them for every version:

- `NULL` -> `NULL`. `INTEGER`/`REAL` input is returned unchanged. `BLOB` -> `NULL`.
- Text: spaces, tabs and newlines at either end are ignored. `,` `$` `£` `%` are
  removed wherever they appear (`£` is the two UTF-8 bytes `C2 A3`).
- One `-` is allowed before the first digit or `.`, so both `-$5` and `$-5` work.
  `5-` and `--5` give `NULL`.
- At most one `.`, and at least one digit. `.5` and `5.` are both fine.
- No `.` gives an INTEGER. A whole number too big for a 64-bit integer comes back as a REAL.
- With a `.` the result is a REAL.
- No exponents, no `+`, no spaces inside the number, no other currency signs.
- `%` is only removed: `45%` -> 45, not 0.45.

The entry point is named `sqlite3_parsenumber_init` because SQLite derives that
name from both `parse_number.so` and `libparse_number.so`, so
`load_extension('path')` works without naming the entry point.

## Versions built

1. `c/` is a straightforward C version. It copies the cleaned characters into a
   buffer (on the stack when the input is under 64 bytes, otherwise from
   `sqlite3_malloc`). It builds integers up digit by digit with overflow checks
   and sends decimals to `strtod()`.
2. `rust-rusqlite/` uses rusqlite 0.41 with the `loadable_extension` feature and
   `Connection::extension_init2` + `create_scalar_function`.
3. `rust-loadable/` uses `sqlite-loadable` 0.0.6-alpha.6 (asg017's framework).
   It compiled fine on rustc 1.97, but it runs bindgen, so it needs libclang and
   takes over a minute to build from scratch.
4. `c-fast/` was added in a second round. It's a C version with its own decimal
   parsing (see below).

Both Rust versions share `rust-core/parse.rs`, a plain function over `&[u8]`
that uses `str::parse::<i64>` and `str::parse::<f64>`.

Release profile: `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.

## Testing

- `test.py` has 40 hand-written cases checked against every extension, including
  `typeof()`. All of them pass for all four versions.
- `fuzz.py` compares results bit for bit, including `-0.0` and integer vs real,
  against a Python reference that uses `float()` (correctly rounded). It runs
  500k inputs per seed: random junk, decimals with 1–25 significant digits, and
  exact doubles near 2^53 written out in full. 4 seeds × 4 extensions = 8M
  checks, with no mismatches. See `results/fuzz.txt`.

## Benchmark method

- 2M-row in-memory table, `select sum(parse_number(v)) from t`, with the whole
  loop running inside SQLite.
- Baseline `sum(length(v))` over the same rows, subtracted to get the net cost
  per row of the function call plus the parsing.
- The first try (one version after another, median of 7) was too noisy, and the
  rankings flipped between runs. Switching to alternating runs between versions
  and taking the fastest of 15 made the results stable.
- Three datasets: mixed (realistic, with ~10% junk), integers only (`$-123,456`)
  and decimals only (`$-123,456.78`).

## Round 1 findings (C vs Rust)

- Mixed data: C 67 ns/row, rusqlite 45, sqlite-loadable 42. C lost, which surprised me.
- Splitting the data showed why. C wins on integers (22–25 vs 32–36 ns) but
  loses badly on decimals (101–106 vs 43–55 ns). The cause is glibc's `strtod`,
  which is general purpose and depends on the locale. Rust's `f64::from_str`
  uses a fast path plus Eisel-Lemire.
- Rust's integer gap is partly the `Vec` it allocates on every call. A
  fixed-size stack buffer took rusqlite from 35 to 31 ns. The rest is the cost
  of rusqlite's closure, `Result` and `Value` wrapping. I reverted that
  experiment, so the code in this folder uses the `Vec`.
- Sizes: C 14 KB (only ~3 KB of real content; the rest is ELF page padding).
  Rust ~300 KB even with `opt-level = "z"`, because it carries Rust's standard
  library, formatting and panic code.

## Round 2: writing a faster C decimal parser

Idea: the scanning loop already sees every digit, so build up the
significant digits in a `uint64_t` (`mant`) while counting the digits after the
`.` (`frac`). If `mant <= 2^53` and `frac <= 22`, then `mant` and `10^frac` are
both exact doubles, and one IEEE division `(double)mant / 1e{frac}` gives the
correctly rounded result. This is Clinger's fast path, the same first step
Rust's parser takes.

Details:
- Leading zeros don't count as significant digits, so `0.000000123` still uses the fast path.
- Only the first 19 significant digits are collected. Past that, or past
  2^53 / 22 fractional digits, it falls back to a second pass that builds a
  buffer and calls `strtod`. That path only runs for very long numbers, which
  money and percentage data never has.
- Integers no longer write to any buffer; it's a single pass.
- The fast path is disabled at compile time unless `FLT_EVAL_METHOD == 0`, because
  x87 80-bit math on 32-bit x86 could round twice.
- `-0.0` behaves the same as `strtod` / Rust (negate after dividing).

Result: c-fast runs at 16–19 ns/row on mixed, integer and decimal data alike,
about 2× faster than either Rust version and 6× faster than plain C on decimals.
It passes all 40 tests and all 8M fuzz checks. Size is unchanged at 14 KB.

## Possible further work (not done)

- Use Eisel-Lemire in C as well, so long inputs don't fall back to `strtod`
  (not worth it for this data).
- Make the `strtod` fallback independent of the locale (`strtod_l` with a C locale).
- `#![no_std]` Rust to shrink the 300 KB binaries.
