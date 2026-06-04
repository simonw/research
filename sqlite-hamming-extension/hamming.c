/*
 * hamming.c - SQLite extension providing hamming_distance() scalar function.
 *
 * Computes the Hamming distance between two equal-length BLOB arguments
 * using XOR + popcount.  Processes data in 64-bit chunks for speed.
 *
 * Build (Linux):
 *   gcc -g -fPIC -shared hamming.c -o hamming.so -march=native -O3
 */

#include <stdint.h>
#include <sqlite3ext.h>
SQLITE_EXTENSION_INIT1

static void hamming_distance(
    sqlite3_context *context,
    int argc,
    sqlite3_value **argv
) {
    if (argc != 2) {
        sqlite3_result_error(context, "hamming_distance() requires 2 arguments", -1);
        return;
    }

    /* Check for NULL inputs */
    if (sqlite3_value_type(argv[0]) == SQLITE_NULL ||
        sqlite3_value_type(argv[1]) == SQLITE_NULL) {
        sqlite3_result_null(context);
        return;
    }

    const unsigned char *blob1 = sqlite3_value_blob(argv[0]);
    const unsigned char *blob2 = sqlite3_value_blob(argv[1]);
    int len1 = sqlite3_value_bytes(argv[0]);
    int len2 = sqlite3_value_bytes(argv[1]);

    if (len1 != len2) {
        sqlite3_result_error(context, "vectors must be same length", -1);
        return;
    }

    const uint64_t *v1 = (const uint64_t *)blob1;
    const uint64_t *v2 = (const uint64_t *)blob2;

    int chunks = len1 / 8;
    uint64_t distance = 0;

    /* Process 8-byte (64-bit) chunks using popcount */
    for (int i = 0; i < chunks; i++) {
        distance += __builtin_popcountll(v1[i] ^ v2[i]);
    }

    /* Handle remaining bytes */
    for (int i = chunks * 8; i < len1; i++) {
        distance += __builtin_popcount(blob1[i] ^ blob2[i]);
    }

    sqlite3_result_int64(context, (sqlite3_int64)distance);
}

#ifdef _WIN32
__declspec(dllexport)
#endif
int sqlite3_hamming_init(
    sqlite3 *db,
    char **pzErrMsg,
    const sqlite3_api_routines *pApi
) {
    SQLITE_EXTENSION_INIT2(pApi);
    return sqlite3_create_function(
        db, "hamming_distance", 2,
        SQLITE_UTF8 | SQLITE_DETERMINISTIC | SQLITE_INNOCUOUS,
        0,
        hamming_distance,
        0, 0
    );
}
