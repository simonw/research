/*
** parse_number(value) SQLite extension.
**
**   NULL / non-text-non-number  -> NULL
**   INTEGER / REAL              -> returned unchanged
**   TEXT                        -> strip , $ £ %, allow a single leading '-'
**                                  (before any digit), optional single '.',
**                                  surrounding whitespace ignored.
**                                  No '.' -> INTEGER (REAL if it overflows i64)
**                                  With '.' -> REAL.  Anything else -> NULL.
**
** Build: gcc -O2 -fPIC -shared parse_number.c -o parse_number.so
*/
#include <sqlite3ext.h>
SQLITE_EXTENSION_INIT1
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

static int is_space(unsigned char c){
  return c==' ' || c=='\t' || c=='\n' || c=='\r';
}

static void parse_number_func(sqlite3_context *ctx, int argc, sqlite3_value **argv){
  (void)argc;
  switch( sqlite3_value_type(argv[0]) ){
    case SQLITE_INTEGER:
    case SQLITE_FLOAT:
      sqlite3_result_value(ctx, argv[0]);
      return;
    case SQLITE_TEXT:
      break;
    default:
      return; /* NULL, BLOB -> NULL */
  }

  const unsigned char *s = sqlite3_value_text(argv[0]);
  int n = sqlite3_value_bytes(argv[0]);
  if( s==0 ) return;

  /* trim surrounding whitespace */
  int i = 0, end = n;
  while( i<end && is_space(s[i]) ) i++;
  while( end>i && is_space(s[end-1]) ) end--;

  /* Cleaned output: optional '-', digits, optional '.'. Never longer than input. */
  char stackbuf[64];
  char *buf = stackbuf;
  if( end - i + 1 > (int)sizeof(stackbuf) ){
    buf = sqlite3_malloc(end - i + 1);
    if( buf==0 ){ sqlite3_result_error_nomem(ctx); return; }
  }

  int len = 0, ndigits = 0, seen_dot = 0, neg = 0, ok = 1;
  uint64_t acc = 0;   /* integer accumulator */
  int overflow = 0;

  for( ; i<end; i++ ){
    unsigned char c = s[i];
    if( c==',' || c=='$' || c=='%' ) continue;
    if( c==0xC2 && i+1<end && s[i+1]==0xA3 ){ i++; continue; }   /* £ in UTF-8 */
    if( c>='0' && c<='9' ){
      buf[len++] = (char)c;
      ndigits++;
      if( !seen_dot && !overflow ){
        if( acc > (UINT64_MAX - (c-'0'))/10 ) overflow = 1;
        else acc = acc*10 + (c-'0');
      }
      continue;
    }
    if( c=='.' && !seen_dot ){ seen_dot = 1; buf[len++] = '.'; continue; }
    if( c=='-' && !neg && len==0 ){ neg = 1; buf[len++] = '-'; continue; }
    ok = 0;
    break;
  }
  buf[len] = 0;

  if( ok && ndigits>0 ){
    if( !seen_dot && !overflow && acc <= (uint64_t)INT64_MAX ){
      sqlite3_result_int64(ctx, neg ? -(sqlite3_int64)acc : (sqlite3_int64)acc);
    }else if( !seen_dot && !overflow && neg && acc == (uint64_t)INT64_MAX + 1 ){
      sqlite3_result_int64(ctx, INT64_MIN);
    }else{
      sqlite3_result_double(ctx, strtod(buf, 0));
    }
  }
  if( buf!=stackbuf ) sqlite3_free(buf);
}

#ifdef _WIN32
__declspec(dllexport)
#endif
int sqlite3_parsenumber_init(sqlite3 *db, char **pzErrMsg, const sqlite3_api_routines *pApi){
  (void)pzErrMsg;
  SQLITE_EXTENSION_INIT2(pApi);
  return sqlite3_create_function(db, "parse_number", 1,
      SQLITE_UTF8 | SQLITE_DETERMINISTIC | SQLITE_INNOCUOUS,
      0, parse_number_func, 0, 0);
}
