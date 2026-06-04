/*
** SQLite Ripgrep Extension - Table-Valued Function
**
** This extension provides a table-valued function 'ripgrep' that runs
** ripgrep searches and returns results as rows.
**
** Usage:
**   SELECT * FROM ripgrep('pattern');
**   SELECT * FROM ripgrep('pattern', '*.py');
**   SELECT * FROM ripgrep('pattern', '*.py', 2.0);
**
** The function returns rows with columns:
**   - path: file path relative to base directory
**   - line_number: line number of the match
**   - line_text: the matching line text
**   - match_text: the actual matched text
**   - match_start: start offset of match in line
**   - match_end: end offset of match in line
**
** Build:
**   gcc -shared -fPIC -o sqlite_ripgrep.so sqlite_ripgrep_ext.c \
**       -I/path/to/sqlite -DSQLITE_RIPGREP_BASE_DIR='"/path/to/search"'
**
** Load:
**   SELECT load_extension('./sqlite_ripgrep');
**   -- or at runtime: sqlite3_load_extension(db, "./sqlite_ripgrep", 0, 0);
*/

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <unistd.h>
#include <sys/wait.h>
#include <sys/time.h>
#include <errno.h>
#include <pthread.h>

#include "sqlite3ext.h"
SQLITE_EXTENSION_INIT1

/* Default base directory - can be overridden at compile time */
#ifndef SQLITE_RIPGREP_BASE_DIR
#define SQLITE_RIPGREP_BASE_DIR "/tmp"
#endif

/* Default time limit in seconds */
#ifndef SQLITE_RIPGREP_DEFAULT_TIME_LIMIT
#define SQLITE_RIPGREP_DEFAULT_TIME_LIMIT 1.0
#endif

/* Maximum results to return */
#define MAX_RESULTS 10000

/* Maximum line length */
#define MAX_LINE_LEN 65536

/* Bitmask values for idxNum in virtual table */
#define RIPGREP_PATTERN   1
#define RIPGREP_GLOB      2
#define RIPGREP_TIMELIMIT 4
#define RIPGREP_BASEDIR   8

/* Result structure */
typedef struct RipgrepResult {
    char *path;
    int line_number;
    char *line_text;
    char *match_text;
    int match_start;
    int match_end;
} RipgrepResult;

/* Cursor structure */
typedef struct ripgrep_cursor {
    sqlite3_vtab_cursor base;
    RipgrepResult *results;
    int num_results;
    int current_row;
    int truncated;
    int time_limit_hit;
    char *error;
} ripgrep_cursor;

/* Virtual table structure */
typedef struct ripgrep_vtab {
    sqlite3_vtab base;
    char *base_directory;
    double default_time_limit;
} ripgrep_vtab;

/* Timer control for time limit */
static volatile sig_atomic_t time_limit_exceeded = 0;
static pid_t rg_pid = 0;

static void timer_handler(int sig) {
    (void)sig;
    time_limit_exceeded = 1;
    if (rg_pid > 0) {
        kill(rg_pid, SIGKILL);
    }
}

/* Simple JSON string extraction helper */
static char *extract_json_string(const char *json, const char *key) {
    char search_key[256];
    snprintf(search_key, sizeof(search_key), "\"%s\":", key);

    const char *pos = strstr(json, search_key);
    if (!pos) return NULL;

    pos += strlen(search_key);
    while (*pos == ' ' || *pos == '\t') pos++;

    if (*pos == '{') {
        /* Nested object - look for "text" inside */
        const char *text_pos = strstr(pos, "\"text\":");
        if (!text_pos) return NULL;
        text_pos += 7;
        while (*text_pos == ' ' || *text_pos == '\t') text_pos++;
        pos = text_pos;
    }

    if (*pos != '"') return NULL;
    pos++;

    /* Find end of string, handling escapes */
    const char *end = pos;
    while (*end && *end != '"') {
        if (*end == '\\' && *(end+1)) end += 2;
        else end++;
    }

    int len = end - pos;
    char *result = malloc(len + 1);
    if (!result) return NULL;

    /* Copy with basic escape handling */
    char *dst = result;
    const char *src = pos;
    while (src < end) {
        if (*src == '\\' && src + 1 < end) {
            src++;
            switch (*src) {
                case 'n': *dst++ = '\n'; break;
                case 't': *dst++ = '\t'; break;
                case 'r': *dst++ = '\r'; break;
                case '"': *dst++ = '"'; break;
                case '\\': *dst++ = '\\'; break;
                default: *dst++ = *src; break;
            }
            src++;
        } else {
            *dst++ = *src++;
        }
    }
    *dst = '\0';

    return result;
}

/* Extract integer from JSON */
static int extract_json_int(const char *json, const char *key) {
    char search_key[256];
    snprintf(search_key, sizeof(search_key), "\"%s\":", key);

    const char *pos = strstr(json, search_key);
    if (!pos) return -1;

    pos += strlen(search_key);
    while (*pos == ' ' || *pos == '\t') pos++;

    return atoi(pos);
}

/* Parse a single ripgrep JSON match line */
static int parse_match_line(const char *line, RipgrepResult *result) {
    /* Check if this is a match type */
    if (!strstr(line, "\"type\":\"match\"")) {
        return 0;
    }

    /* Find the data section */
    const char *data_pos = strstr(line, "\"data\":");
    if (!data_pos) return 0;

    result->path = extract_json_string(data_pos, "path");
    result->line_number = extract_json_int(data_pos, "line_number");
    result->line_text = extract_json_string(data_pos, "lines");

    /* Extract first submatch if present */
    const char *submatches = strstr(data_pos, "\"submatches\":");
    if (submatches) {
        result->match_text = extract_json_string(submatches, "match");
        result->match_start = extract_json_int(submatches, "start");
        result->match_end = extract_json_int(submatches, "end");
    } else {
        result->match_text = NULL;
        result->match_start = -1;
        result->match_end = -1;
    }

    return 1;
}

/* Free a result */
static void free_result(RipgrepResult *result) {
    free(result->path);
    free(result->line_text);
    free(result->match_text);
}

/* Run ripgrep and collect results */
static int run_ripgrep(
    const char *pattern,
    const char *base_dir,
    const char *glob_pattern,
    double time_limit,
    RipgrepResult **results_out,
    int *num_results_out,
    int *truncated_out,
    int *time_limit_hit_out,
    char **error_out
) {
    *results_out = NULL;
    *num_results_out = 0;
    *truncated_out = 0;
    *time_limit_hit_out = 0;
    *error_out = NULL;

    /* Build command */
    int pipe_fd[2];
    if (pipe(pipe_fd) == -1) {
        *error_out = strdup("Failed to create pipe");
        return -1;
    }

    time_limit_exceeded = 0;

    pid_t pid = fork();
    if (pid == -1) {
        close(pipe_fd[0]);
        close(pipe_fd[1]);
        *error_out = strdup("Failed to fork");
        return -1;
    }

    if (pid == 0) {
        /* Child process */
        close(pipe_fd[0]);
        dup2(pipe_fd[1], STDOUT_FILENO);
        close(pipe_fd[1]);

        /* Build args */
        if (glob_pattern && strlen(glob_pattern) > 0) {
            execlp("rg", "rg", "-e", pattern, "--json", "--glob", glob_pattern, base_dir, NULL);
        } else {
            execlp("rg", "rg", "-e", pattern, "--json", base_dir, NULL);
        }
        _exit(127);
    }

    /* Parent process */
    close(pipe_fd[1]);
    rg_pid = pid;

    /* Set up timer */
    struct sigaction sa;
    sa.sa_handler = timer_handler;
    sigemptyset(&sa.sa_mask);
    sa.sa_flags = 0;
    sigaction(SIGALRM, &sa, NULL);

    struct itimerval timer;
    timer.it_value.tv_sec = (long)time_limit;
    timer.it_value.tv_usec = (long)((time_limit - (long)time_limit) * 1000000);
    timer.it_interval.tv_sec = 0;
    timer.it_interval.tv_usec = 0;
    setitimer(ITIMER_REAL, &timer, NULL);

    /* Read results */
    FILE *fp = fdopen(pipe_fd[0], "r");
    if (!fp) {
        close(pipe_fd[0]);
        kill(pid, SIGKILL);
        waitpid(pid, NULL, 0);
        *error_out = strdup("Failed to open pipe for reading");
        return -1;
    }

    RipgrepResult *results = malloc(MAX_RESULTS * sizeof(RipgrepResult));
    if (!results) {
        fclose(fp);
        kill(pid, SIGKILL);
        waitpid(pid, NULL, 0);
        *error_out = strdup("Out of memory");
        return -1;
    }

    int num_results = 0;
    char line[MAX_LINE_LEN];

    while (!time_limit_exceeded && num_results < MAX_RESULTS) {
        if (!fgets(line, sizeof(line), fp)) {
            break;
        }

        RipgrepResult result = {0};
        if (parse_match_line(line, &result)) {
            results[num_results++] = result;
        }
    }

    /* Disable timer */
    timer.it_value.tv_sec = 0;
    timer.it_value.tv_usec = 0;
    setitimer(ITIMER_REAL, &timer, NULL);

    if (time_limit_exceeded) {
        *time_limit_hit_out = 1;
        *truncated_out = 1;
    } else if (num_results >= MAX_RESULTS) {
        *truncated_out = 1;
    }

    fclose(fp);
    kill(pid, SIGKILL);
    waitpid(pid, NULL, 0);
    rg_pid = 0;

    *results_out = results;
    *num_results_out = num_results;

    return 0;
}

/*
** Virtual table methods
*/

static int ripgrepConnect(
    sqlite3 *db,
    void *pAux,
    int argc, const char *const *argv,
    sqlite3_vtab **ppVtab,
    char **pzErr
) {
    (void)pAux;
    (void)argc;
    (void)argv;
    (void)pzErr;

    ripgrep_vtab *pNew;
    int rc;

    rc = sqlite3_declare_vtab(db,
        "CREATE TABLE x("
        "  path TEXT,"
        "  line_number INTEGER,"
        "  line_text TEXT,"
        "  match_text TEXT,"
        "  match_start INTEGER,"
        "  match_end INTEGER,"
        "  pattern TEXT HIDDEN,"
        "  glob TEXT HIDDEN,"
        "  time_limit REAL HIDDEN,"
        "  base_dir TEXT HIDDEN"
        ")"
    );

    if (rc == SQLITE_OK) {
        pNew = sqlite3_malloc(sizeof(*pNew));
        if (pNew == 0) return SQLITE_NOMEM;
        memset(pNew, 0, sizeof(*pNew));
        pNew->base_directory = strdup(SQLITE_RIPGREP_BASE_DIR);
        pNew->default_time_limit = SQLITE_RIPGREP_DEFAULT_TIME_LIMIT;
        *ppVtab = &pNew->base;
    }

    return rc;
}

static int ripgrepDisconnect(sqlite3_vtab *pVtab) {
    ripgrep_vtab *p = (ripgrep_vtab*)pVtab;
    free(p->base_directory);
    sqlite3_free(p);
    return SQLITE_OK;
}

static int ripgrepOpen(sqlite3_vtab *pVtab, sqlite3_vtab_cursor **ppCursor) {
    (void)pVtab;
    ripgrep_cursor *pCur;
    pCur = sqlite3_malloc(sizeof(*pCur));
    if (pCur == 0) return SQLITE_NOMEM;
    memset(pCur, 0, sizeof(*pCur));
    *ppCursor = &pCur->base;
    return SQLITE_OK;
}

static int ripgrepClose(sqlite3_vtab_cursor *cur) {
    ripgrep_cursor *pCur = (ripgrep_cursor*)cur;
    if (pCur->results) {
        for (int i = 0; i < pCur->num_results; i++) {
            free_result(&pCur->results[i]);
        }
        free(pCur->results);
    }
    free(pCur->error);
    sqlite3_free(pCur);
    return SQLITE_OK;
}

static int ripgrepNext(sqlite3_vtab_cursor *cur) {
    ripgrep_cursor *pCur = (ripgrep_cursor*)cur;
    pCur->current_row++;
    return SQLITE_OK;
}

static int ripgrepColumn(sqlite3_vtab_cursor *cur, sqlite3_context *ctx, int i) {
    ripgrep_cursor *pCur = (ripgrep_cursor*)cur;

    if (pCur->current_row >= pCur->num_results) {
        sqlite3_result_null(ctx);
        return SQLITE_OK;
    }

    RipgrepResult *r = &pCur->results[pCur->current_row];

    switch (i) {
        case 0: /* path */
            sqlite3_result_text(ctx, r->path ? r->path : "", -1, SQLITE_TRANSIENT);
            break;
        case 1: /* line_number */
            sqlite3_result_int(ctx, r->line_number);
            break;
        case 2: /* line_text */
            sqlite3_result_text(ctx, r->line_text ? r->line_text : "", -1, SQLITE_TRANSIENT);
            break;
        case 3: /* match_text */
            if (r->match_text) {
                sqlite3_result_text(ctx, r->match_text, -1, SQLITE_TRANSIENT);
            } else {
                sqlite3_result_null(ctx);
            }
            break;
        case 4: /* match_start */
            sqlite3_result_int(ctx, r->match_start);
            break;
        case 5: /* match_end */
            sqlite3_result_int(ctx, r->match_end);
            break;
        case 6: /* pattern (hidden) */
        case 7: /* glob (hidden) */
        case 8: /* time_limit (hidden) */
        case 9: /* base_dir (hidden) */
            sqlite3_result_null(ctx);
            break;
    }

    return SQLITE_OK;
}

static int ripgrepRowid(sqlite3_vtab_cursor *cur, sqlite_int64 *pRowid) {
    ripgrep_cursor *pCur = (ripgrep_cursor*)cur;
    *pRowid = pCur->current_row;
    return SQLITE_OK;
}

static int ripgrepEof(sqlite3_vtab_cursor *cur) {
    ripgrep_cursor *pCur = (ripgrep_cursor*)cur;
    return pCur->current_row >= pCur->num_results;
}

static int ripgrepFilter(
    sqlite3_vtab_cursor *pVtabCursor,
    int idxNum,
    const char *idxStr,
    int argc,
    sqlite3_value **argv
) {
    (void)idxStr;
    (void)argc;

    ripgrep_cursor *pCur = (ripgrep_cursor*)pVtabCursor;
    ripgrep_vtab *pVtab = (ripgrep_vtab*)pVtabCursor->pVtab;

    /* Free any previous results */
    if (pCur->results) {
        for (int i = 0; i < pCur->num_results; i++) {
            free_result(&pCur->results[i]);
        }
        free(pCur->results);
        pCur->results = NULL;
    }
    free(pCur->error);
    pCur->error = NULL;
    pCur->current_row = 0;
    pCur->num_results = 0;

    /* No pattern provided */
    if (!(idxNum & RIPGREP_PATTERN)) {
        return SQLITE_OK;
    }

    /* Extract parameters based on idxNum bitmask */
    int argIdx = 0;
    const char *pattern = NULL;
    const char *glob_pattern = NULL;
    double time_limit = pVtab->default_time_limit;
    const char *base_dir = NULL;

    /* Pattern is always first if present */
    if (idxNum & RIPGREP_PATTERN) {
        pattern = (const char*)sqlite3_value_text(argv[argIdx++]);
    }
    if (idxNum & RIPGREP_GLOB) {
        glob_pattern = (const char*)sqlite3_value_text(argv[argIdx++]);
    }
    if (idxNum & RIPGREP_TIMELIMIT) {
        time_limit = sqlite3_value_double(argv[argIdx++]);
    }
    if (idxNum & RIPGREP_BASEDIR) {
        base_dir = (const char*)sqlite3_value_text(argv[argIdx++]);
    }

    if (!pattern || strlen(pattern) == 0) {
        return SQLITE_OK;
    }

    /* Use provided base_dir or fall back to default */
    const char *search_dir = (base_dir && strlen(base_dir) > 0) ? base_dir : pVtab->base_directory;

    run_ripgrep(
        pattern,
        search_dir,
        glob_pattern,
        time_limit,
        &pCur->results,
        &pCur->num_results,
        &pCur->truncated,
        &pCur->time_limit_hit,
        &pCur->error
    );

    return SQLITE_OK;
}

static int ripgrepBestIndex(sqlite3_vtab *pVtab, sqlite3_index_info *pIdxInfo) {
    (void)pVtab;

    int idxNum = 0;
    int argvIndex = 1;

    /* First pass: find pattern (required) */
    for (int i = 0; i < pIdxInfo->nConstraint; i++) {
        if (!pIdxInfo->aConstraint[i].usable) continue;
        if (pIdxInfo->aConstraint[i].op != SQLITE_INDEX_CONSTRAINT_EQ) continue;

        if (pIdxInfo->aConstraint[i].iColumn == 6) {  /* pattern */
            pIdxInfo->aConstraintUsage[i].argvIndex = argvIndex++;
            pIdxInfo->aConstraintUsage[i].omit = 1;
            idxNum |= RIPGREP_PATTERN;
            break;
        }
    }

    /* Second pass: find optional parameters */
    for (int i = 0; i < pIdxInfo->nConstraint; i++) {
        if (!pIdxInfo->aConstraint[i].usable) continue;
        if (pIdxInfo->aConstraint[i].op != SQLITE_INDEX_CONSTRAINT_EQ) continue;

        int col = pIdxInfo->aConstraint[i].iColumn;

        if (col == 7 && !(idxNum & RIPGREP_GLOB)) {  /* glob */
            pIdxInfo->aConstraintUsage[i].argvIndex = argvIndex++;
            pIdxInfo->aConstraintUsage[i].omit = 1;
            idxNum |= RIPGREP_GLOB;
        } else if (col == 8 && !(idxNum & RIPGREP_TIMELIMIT)) {  /* time_limit */
            pIdxInfo->aConstraintUsage[i].argvIndex = argvIndex++;
            pIdxInfo->aConstraintUsage[i].omit = 1;
            idxNum |= RIPGREP_TIMELIMIT;
        } else if (col == 9 && !(idxNum & RIPGREP_BASEDIR)) {  /* base_dir */
            pIdxInfo->aConstraintUsage[i].argvIndex = argvIndex++;
            pIdxInfo->aConstraintUsage[i].omit = 1;
            idxNum |= RIPGREP_BASEDIR;
        }
    }

    pIdxInfo->idxNum = idxNum;

    if (!(idxNum & RIPGREP_PATTERN)) {
        pIdxInfo->estimatedCost = 1000000000.0;
    } else {
        pIdxInfo->estimatedCost = 1000.0;
    }

    return SQLITE_OK;
}

/*
** Virtual table module definition
*/
static sqlite3_module ripgrepModule = {
    0,                    /* iVersion */
    ripgrepConnect,       /* xCreate */
    ripgrepConnect,       /* xConnect */
    ripgrepBestIndex,     /* xBestIndex */
    ripgrepDisconnect,    /* xDisconnect */
    ripgrepDisconnect,    /* xDestroy */
    ripgrepOpen,          /* xOpen */
    ripgrepClose,         /* xClose */
    ripgrepFilter,        /* xFilter */
    ripgrepNext,          /* xNext */
    ripgrepEof,           /* xEof */
    ripgrepColumn,        /* xColumn */
    ripgrepRowid,         /* xRowid */
    0,                    /* xUpdate */
    0,                    /* xBegin */
    0,                    /* xSync */
    0,                    /* xCommit */
    0,                    /* xRollback */
    0,                    /* xFindFunction */
    0,                    /* xRename */
    0,                    /* xSavepoint */
    0,                    /* xRelease */
    0,                    /* xRollbackTo */
    0,                    /* xShadowName */
    0                     /* xIntegrity */
};

/*
** Extension entry point
*/
#ifdef _WIN32
__declspec(dllexport)
#endif
int sqlite3_sqliteripgrep_init(
    sqlite3 *db,
    char **pzErrMsg,
    const sqlite3_api_routines *pApi
) {
    (void)pzErrMsg;
    SQLITE_EXTENSION_INIT2(pApi);

    return sqlite3_create_module(db, "ripgrep", &ripgrepModule, 0);
}
