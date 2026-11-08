#!/bin/bash
# Build all four extensions. Requires gcc, SQLite headers (sqlite3ext.h) and cargo.
# sqlite-loadable also needs libclang (it runs bindgen).
set -e
cd "$(dirname "$0")"
gcc -O2 -fPIC -shared -s c/parse_number.c -o c/parse_number.so
gcc -O2 -fPIC -shared -s c-fast/parse_number.c -o c-fast/parse_number.so
(cd rust-rusqlite && cargo build --release)
(cd rust-loadable && cargo build --release)
ls -l c/parse_number.so c-fast/parse_number.so */target/release/libparse_number.so
