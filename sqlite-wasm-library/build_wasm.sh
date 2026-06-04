#!/bin/bash
set -e

WASI_SDK_PATH=/home/user/research/sqlite-wasm-library/wasi-sdk-21.0
SQLITE_DIR=/home/user/research/sqlite-wasm-library/sqlite-autoconf-3450300
OUTPUT_DIR=/home/user/research/sqlite-wasm-library/sqlite3_wasm/src/sqlite3_wasm

CC="${WASI_SDK_PATH}/bin/clang"
SYSROOT="${WASI_SDK_PATH}/share/wasi-sysroot"

# Define all the SQLite functions we need to export
EXPORTS=(
    "malloc"
    "free"
    "realloc"
    "strlen"
    "sqlite3_libversion"
    "sqlite3_libversion_number"
    "sqlite3_open"
    "sqlite3_open_v2"
    "sqlite3_close"
    "sqlite3_close_v2"
    "sqlite3_exec"
    "sqlite3_prepare_v2"
    "sqlite3_prepare_v3"
    "sqlite3_step"
    "sqlite3_finalize"
    "sqlite3_reset"
    "sqlite3_clear_bindings"
    "sqlite3_bind_null"
    "sqlite3_bind_int"
    "sqlite3_bind_int64"
    "sqlite3_bind_double"
    "sqlite3_bind_text"
    "sqlite3_bind_blob"
    "sqlite3_bind_parameter_count"
    "sqlite3_bind_parameter_name"
    "sqlite3_bind_parameter_index"
    "sqlite3_column_count"
    "sqlite3_column_name"
    "sqlite3_column_type"
    "sqlite3_column_int"
    "sqlite3_column_int64"
    "sqlite3_column_double"
    "sqlite3_column_text"
    "sqlite3_column_blob"
    "sqlite3_column_bytes"
    "sqlite3_data_count"
    "sqlite3_errmsg"
    "sqlite3_errcode"
    "sqlite3_extended_errcode"
    "sqlite3_errstr"
    "sqlite3_last_insert_rowid"
    "sqlite3_changes"
    "sqlite3_total_changes"
    "sqlite3_interrupt"
    "sqlite3_complete"
    "sqlite3_busy_timeout"
    "sqlite3_get_autocommit"
    "sqlite3_db_readonly"
    "sqlite3_stmt_readonly"
    "sqlite3_sql"
    "sqlite3_expanded_sql"
    "sqlite3_db_handle"
    "sqlite3_next_stmt"
    "sqlite3_column_database_name"
    "sqlite3_column_table_name"
    "sqlite3_column_origin_name"
    "sqlite3_column_decltype"
    "sqlite3_sourceid"
)

# Build the export flags
EXPORT_FLAGS=""
for func in "${EXPORTS[@]}"; do
    EXPORT_FLAGS="${EXPORT_FLAGS} -Wl,--export=${func}"
done

echo "Compiling SQLite to WASM..."
$CC \
    --sysroot="${SYSROOT}" \
    --target=wasm32-wasi \
    -O2 \
    -DSQLITE_THREADSAFE=0 \
    -DSQLITE_OMIT_LOAD_EXTENSION \
    -DSQLITE_ENABLE_FTS5 \
    -DSQLITE_ENABLE_JSON1 \
    -DSQLITE_ENABLE_RTREE \
    -DSQLITE_TEMP_STORE=2 \
    -DSQLITE_USE_URI=1 \
    -DSQLITE_ENABLE_COLUMN_METADATA \
    -D_WASI_EMULATED_SIGNAL \
    -D_WASI_EMULATED_GETPID \
    -lwasi-emulated-signal \
    -lwasi-emulated-getpid \
    -Wl,--no-entry \
    -Wl,--export-memory \
    ${EXPORT_FLAGS} \
    -o "${OUTPUT_DIR}/sqlite3.wasm" \
    "${SQLITE_DIR}/sqlite3.c"

echo "SQLite WASM compiled successfully!"
echo "Output: ${OUTPUT_DIR}/sqlite3.wasm"
ls -lh "${OUTPUT_DIR}/sqlite3.wasm"
