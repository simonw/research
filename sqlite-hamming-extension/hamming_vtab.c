/*
 * hamming_vtab.c - SQLite virtual table for top-k Hamming distance search.
 *
 * Provides a virtual table "hamming_topk" that scans a source table's
 * BLOB embeddings, computing Hamming distance against a query vector,
 * and returns the top-k nearest results using a max-heap — avoiding a
 * full sort of all rows.
 *
 * Also registers the scalar hamming_distance() for convenience.
 *
 * Usage:
 *   CREATE VIRTUAL TABLE search USING hamming_topk(documents, embedding);
 *
 *   SELECT source_rowid, distance FROM search
 *   WHERE query = X'aabb...' AND k = 10;
 *
 * Build (Linux):
 *   gcc -g -fPIC -shared hamming_vtab.c -o hamming_vtab.so -O3 -mpopcnt
 */

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <sqlite3ext.h>
SQLITE_EXTENSION_INIT1

/* ========================================================================= */
/* Scalar hamming_distance (same as hamming.c)                               */
/* ========================================================================= */

static void hamming_distance_func(
    sqlite3_context *context,
    int argc,
    sqlite3_value **argv
) {
    if (argc != 2) {
        sqlite3_result_error(context, "hamming_distance() requires 2 arguments", -1);
        return;
    }
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

    for (int i = 0; i < chunks; i++) {
        distance += __builtin_popcountll(v1[i] ^ v2[i]);
    }
    for (int i = chunks * 8; i < len1; i++) {
        distance += __builtin_popcount(blob1[i] ^ blob2[i]);
    }

    sqlite3_result_int64(context, (sqlite3_int64)distance);
}


/* ========================================================================= */
/* Max-heap for top-k selection                                              */
/* ========================================================================= */

typedef struct {
    sqlite3_int64 rowid;
    int distance;
} HeapEntry;

typedef struct {
    HeapEntry *entries;
    int capacity;
    int size;
} MaxHeap;

static void heap_init(MaxHeap *h, int capacity) {
    h->entries = (HeapEntry *)sqlite3_malloc(sizeof(HeapEntry) * capacity);
    h->capacity = capacity;
    h->size = 0;
}

static void heap_free(MaxHeap *h) {
    if (h->entries) {
        sqlite3_free(h->entries);
        h->entries = NULL;
    }
    h->size = 0;
    h->capacity = 0;
}

/* Compare for max-heap ordering: returns 1 if a is "worse" (higher priority
 * for eviction) than b.  Higher distance is worse; on tie, higher rowid. */
static int heap_greater(const HeapEntry *a, const HeapEntry *b) {
    if (a->distance != b->distance) return a->distance > b->distance;
    return a->rowid > b->rowid;
}

static void heap_sift_up(MaxHeap *h, int i) {
    while (i > 0) {
        int parent = (i - 1) / 2;
        if (heap_greater(&h->entries[i], &h->entries[parent])) {
            HeapEntry tmp = h->entries[i];
            h->entries[i] = h->entries[parent];
            h->entries[parent] = tmp;
            i = parent;
        } else {
            break;
        }
    }
}

static void heap_sift_down(MaxHeap *h, int i) {
    while (1) {
        int largest = i;
        int left = 2 * i + 1;
        int right = 2 * i + 2;
        if (left < h->size && heap_greater(&h->entries[left], &h->entries[largest]))
            largest = left;
        if (right < h->size && heap_greater(&h->entries[right], &h->entries[largest]))
            largest = right;
        if (largest != i) {
            HeapEntry tmp = h->entries[i];
            h->entries[i] = h->entries[largest];
            h->entries[largest] = tmp;
            i = largest;
        } else {
            break;
        }
    }
}

static void heap_offer(MaxHeap *h, sqlite3_int64 rowid, int distance) {
    if (h->size < h->capacity) {
        h->entries[h->size].rowid = rowid;
        h->entries[h->size].distance = distance;
        h->size++;
        heap_sift_up(h, h->size - 1);
    } else if (distance < h->entries[0].distance ||
               (distance == h->entries[0].distance && rowid < h->entries[0].rowid)) {
        h->entries[0].rowid = rowid;
        h->entries[0].distance = distance;
        heap_sift_down(h, 0);
    }
}

static int cmp_entries_asc(const void *a, const void *b) {
    int da = ((const HeapEntry *)a)->distance;
    int db = ((const HeapEntry *)b)->distance;
    if (da != db) return da - db;
    sqlite3_int64 ra = ((const HeapEntry *)a)->rowid;
    sqlite3_int64 rb = ((const HeapEntry *)b)->rowid;
    if (ra < rb) return -1;
    if (ra > rb) return 1;
    return 0;
}

/* Compute hamming distance between two byte arrays of equal length. */
static int compute_hamming(const unsigned char *a, const unsigned char *b, int len) {
    const uint64_t *va = (const uint64_t *)a;
    const uint64_t *vb = (const uint64_t *)b;
    int chunks = len / 8;
    uint64_t dist = 0;
    for (int i = 0; i < chunks; i++) {
        dist += __builtin_popcountll(va[i] ^ vb[i]);
    }
    for (int i = chunks * 8; i < len; i++) {
        dist += __builtin_popcount(a[i] ^ b[i]);
    }
    return (int)dist;
}


/* ========================================================================= */
/* Preloaded embedding cache                                                 */
/* ========================================================================= */

typedef struct {
    sqlite3_int64 *rowids;      /* array of rowids */
    unsigned char *embeddings;  /* contiguous buffer: N * vec_size bytes */
    int count;                  /* number of rows */
    int vec_size;               /* bytes per embedding */
} EmbeddingCache;

static void cache_free(EmbeddingCache *cache) {
    if (cache->rowids) { sqlite3_free(cache->rowids); cache->rowids = NULL; }
    if (cache->embeddings) { sqlite3_free(cache->embeddings); cache->embeddings = NULL; }
    cache->count = 0;
    cache->vec_size = 0;
}

static int cache_load(EmbeddingCache *cache, sqlite3 *db,
                      const char *table_name, const char *column_name) {
    cache->rowids = NULL;
    cache->embeddings = NULL;
    cache->count = 0;
    cache->vec_size = 0;

    /* First, count rows and determine vec_size */
    char *sql_count = sqlite3_mprintf(
        "SELECT COUNT(*) FROM \"%w\"", table_name);
    sqlite3_stmt *stmt = NULL;
    int rc = sqlite3_prepare_v2(db, sql_count, -1, &stmt, NULL);
    sqlite3_free(sql_count);
    if (rc != SQLITE_OK) return rc;

    rc = sqlite3_step(stmt);
    int total = 0;
    if (rc == SQLITE_ROW) {
        total = sqlite3_column_int(stmt, 0);
    }
    sqlite3_finalize(stmt);
    if (total == 0) return SQLITE_OK;

    /* Get vec_size from first row */
    char *sql_first = sqlite3_mprintf(
        "SELECT LENGTH(\"%w\") FROM \"%w\" LIMIT 1",
        column_name, table_name);
    rc = sqlite3_prepare_v2(db, sql_first, -1, &stmt, NULL);
    sqlite3_free(sql_first);
    if (rc != SQLITE_OK) return rc;

    int vec_size = 0;
    if (sqlite3_step(stmt) == SQLITE_ROW) {
        vec_size = sqlite3_column_int(stmt, 0);
    }
    sqlite3_finalize(stmt);
    if (vec_size == 0) return SQLITE_OK;

    /* Allocate contiguous buffers */
    cache->rowids = (sqlite3_int64 *)sqlite3_malloc64(
        (sqlite3_int64)total * sizeof(sqlite3_int64));
    cache->embeddings = (unsigned char *)sqlite3_malloc64(
        (sqlite3_int64)total * vec_size);
    if (!cache->rowids || !cache->embeddings) {
        cache_free(cache);
        return SQLITE_NOMEM;
    }

    /* Load all embeddings */
    char *sql_load = sqlite3_mprintf(
        "SELECT rowid, \"%w\" FROM \"%w\"",
        column_name, table_name);
    rc = sqlite3_prepare_v2(db, sql_load, -1, &stmt, NULL);
    sqlite3_free(sql_load);
    if (rc != SQLITE_OK) {
        cache_free(cache);
        return rc;
    }

    int idx = 0;
    while (sqlite3_step(stmt) == SQLITE_ROW && idx < total) {
        cache->rowids[idx] = sqlite3_column_int64(stmt, 0);
        const void *blob = sqlite3_column_blob(stmt, 1);
        int blob_len = sqlite3_column_bytes(stmt, 1);
        if (blob_len == vec_size) {
            memcpy(cache->embeddings + (sqlite3_int64)idx * vec_size, blob, vec_size);
            idx++;
        }
    }
    sqlite3_finalize(stmt);

    cache->count = idx;
    cache->vec_size = vec_size;
    return SQLITE_OK;
}


/* ========================================================================= */
/* Virtual table implementation                                              */
/* ========================================================================= */

typedef struct {
    sqlite3_vtab base;
    sqlite3 *db;
    char *table_name;
    char *column_name;
    EmbeddingCache cache;
    int cache_loaded;
} HammingVTab;

typedef struct {
    sqlite3_vtab_cursor base;
    MaxHeap heap;
    int current;
    int done;
} HammingCursor;

#define VTAB_COL_ROWID    0
#define VTAB_COL_DISTANCE 1
#define VTAB_COL_QUERY    2
#define VTAB_COL_K        3

static int vtabConnect(
    sqlite3 *db,
    void *pAux,
    int argc,
    const char *const *argv,
    sqlite3_vtab **ppVTab,
    char **pzErr
) {
    if (argc < 5) {
        *pzErr = sqlite3_mprintf(
            "hamming_topk requires 2 arguments: table_name, column_name");
        return SQLITE_ERROR;
    }

    const char *table_name = argv[3];
    const char *column_name = argv[4];

    int rc = sqlite3_declare_vtab(db,
        "CREATE TABLE x(source_rowid INTEGER, distance INTEGER, "
        "query BLOB HIDDEN, k INTEGER HIDDEN)");
    if (rc != SQLITE_OK) return rc;

    HammingVTab *vtab = (HammingVTab *)sqlite3_malloc(sizeof(HammingVTab));
    if (!vtab) return SQLITE_NOMEM;
    memset(vtab, 0, sizeof(*vtab));

    vtab->db = db;
    vtab->table_name = sqlite3_mprintf("%s", table_name);
    vtab->column_name = sqlite3_mprintf("%s", column_name);
    vtab->cache_loaded = 0;

    *ppVTab = &vtab->base;
    return SQLITE_OK;
}

static int vtabDisconnect(sqlite3_vtab *pVTab) {
    HammingVTab *vtab = (HammingVTab *)pVTab;
    cache_free(&vtab->cache);
    if (vtab->table_name) sqlite3_free(vtab->table_name);
    if (vtab->column_name) sqlite3_free(vtab->column_name);
    sqlite3_free(vtab);
    return SQLITE_OK;
}

static int vtabBestIndex(sqlite3_vtab *pVTab, sqlite3_index_info *pIdxInfo) {
    int query_idx = -1;
    int k_idx = -1;

    for (int i = 0; i < pIdxInfo->nConstraint; i++) {
        if (!pIdxInfo->aConstraint[i].usable) continue;
        if (pIdxInfo->aConstraint[i].op != SQLITE_INDEX_CONSTRAINT_EQ) continue;

        if (pIdxInfo->aConstraint[i].iColumn == VTAB_COL_QUERY) {
            query_idx = i;
        }
        if (pIdxInfo->aConstraint[i].iColumn == VTAB_COL_K) {
            k_idx = i;
        }
    }

    if (query_idx < 0) {
        pIdxInfo->estimatedCost = 1e18;
        return SQLITE_OK;
    }

    int argv_index = 1;
    pIdxInfo->aConstraintUsage[query_idx].argvIndex = argv_index++;
    pIdxInfo->aConstraintUsage[query_idx].omit = 1;

    if (k_idx >= 0) {
        pIdxInfo->aConstraintUsage[k_idx].argvIndex = argv_index++;
        pIdxInfo->aConstraintUsage[k_idx].omit = 1;
        pIdxInfo->idxNum = 2;
    } else {
        pIdxInfo->idxNum = 1;
    }

    pIdxInfo->estimatedCost = 1000.0;
    pIdxInfo->estimatedRows = 10;

    return SQLITE_OK;
}

static int vtabOpen(sqlite3_vtab *pVTab, sqlite3_vtab_cursor **ppCursor) {
    HammingCursor *cur = (HammingCursor *)sqlite3_malloc(sizeof(HammingCursor));
    if (!cur) return SQLITE_NOMEM;
    memset(cur, 0, sizeof(*cur));
    cur->done = 1;
    *ppCursor = &cur->base;
    return SQLITE_OK;
}

static int vtabClose(sqlite3_vtab_cursor *pCursor) {
    HammingCursor *cur = (HammingCursor *)pCursor;
    heap_free(&cur->heap);
    sqlite3_free(cur);
    return SQLITE_OK;
}

static int vtabFilter(
    sqlite3_vtab_cursor *pCursor,
    int idxNum,
    const char *idxStr,
    int argc,
    sqlite3_value **argv
) {
    HammingCursor *cur = (HammingCursor *)pCursor;
    HammingVTab *vtab = (HammingVTab *)pCursor->pVtab;

    heap_free(&cur->heap);
    cur->current = -1;
    cur->done = 1;

    if (argc < 1) return SQLITE_OK;
    if (sqlite3_value_type(argv[0]) == SQLITE_NULL) return SQLITE_OK;

    const unsigned char *query_blob = sqlite3_value_blob(argv[0]);
    int query_len = sqlite3_value_bytes(argv[0]);

    int k = 10;
    if (argc >= 2 && sqlite3_value_type(argv[1]) != SQLITE_NULL) {
        k = sqlite3_value_int(argv[1]);
        if (k < 1) k = 1;
        if (k > 10000) k = 10000;
    }

    /* Lazy-load cache on first query */
    if (!vtab->cache_loaded) {
        int rc = cache_load(&vtab->cache, vtab->db,
                           vtab->table_name, vtab->column_name);
        if (rc != SQLITE_OK) return rc;
        vtab->cache_loaded = 1;
    }

    EmbeddingCache *cache = &vtab->cache;

    if (cache->count == 0 || cache->vec_size != query_len) {
        return SQLITE_OK;
    }

    heap_init(&cur->heap, k);

    /* Pure C scan over contiguous memory — no SQLite API calls per row */
    const int vec_size = cache->vec_size;
    const unsigned char *emb_base = cache->embeddings;

    for (int i = 0; i < cache->count; i++) {
        const unsigned char *emb = emb_base + (sqlite3_int64)i * vec_size;
        int dist = compute_hamming(query_blob, emb, vec_size);
        heap_offer(&cur->heap, cache->rowids[i], dist);
    }

    /* Sort heap entries by distance ascending for output */
    if (cur->heap.size > 0) {
        qsort(cur->heap.entries, cur->heap.size, sizeof(HeapEntry), cmp_entries_asc);
        cur->current = 0;
        cur->done = 0;
    }

    return SQLITE_OK;
}

static int vtabNext(sqlite3_vtab_cursor *pCursor) {
    HammingCursor *cur = (HammingCursor *)pCursor;
    cur->current++;
    if (cur->current >= cur->heap.size) {
        cur->done = 1;
    }
    return SQLITE_OK;
}

static int vtabEof(sqlite3_vtab_cursor *pCursor) {
    HammingCursor *cur = (HammingCursor *)pCursor;
    return cur->done;
}

static int vtabColumn(sqlite3_vtab_cursor *pCursor, sqlite3_context *ctx, int i) {
    HammingCursor *cur = (HammingCursor *)pCursor;
    switch (i) {
        case VTAB_COL_ROWID:
            sqlite3_result_int64(ctx, cur->heap.entries[cur->current].rowid);
            break;
        case VTAB_COL_DISTANCE:
            sqlite3_result_int(ctx, cur->heap.entries[cur->current].distance);
            break;
        default:
            sqlite3_result_null(ctx);
            break;
    }
    return SQLITE_OK;
}

static int vtabRowid(sqlite3_vtab_cursor *pCursor, sqlite3_int64 *pRowid) {
    HammingCursor *cur = (HammingCursor *)pCursor;
    *pRowid = cur->current;
    return SQLITE_OK;
}

static sqlite3_module hamming_topk_module = {
    .iVersion = 0,
    .xCreate = vtabConnect,
    .xConnect = vtabConnect,
    .xBestIndex = vtabBestIndex,
    .xDisconnect = vtabDisconnect,
    .xDestroy = vtabDisconnect,
    .xOpen = vtabOpen,
    .xClose = vtabClose,
    .xFilter = vtabFilter,
    .xNext = vtabNext,
    .xEof = vtabEof,
    .xColumn = vtabColumn,
    .xRowid = vtabRowid,
};

/* ========================================================================= */
/* Extension entry point                                                     */
/* ========================================================================= */

#ifdef _WIN32
__declspec(dllexport)
#endif
int sqlite3_hammingvtab_init(
    sqlite3 *db,
    char **pzErrMsg,
    const sqlite3_api_routines *pApi
) {
    SQLITE_EXTENSION_INIT2(pApi);

    int rc = sqlite3_create_function(
        db, "hamming_distance", 2,
        SQLITE_UTF8 | SQLITE_DETERMINISTIC | SQLITE_INNOCUOUS,
        0,
        hamming_distance_func,
        0, 0
    );
    if (rc != SQLITE_OK) return rc;

    rc = sqlite3_create_module(db, "hamming_topk", &hamming_topk_module, NULL);
    return rc;
}
