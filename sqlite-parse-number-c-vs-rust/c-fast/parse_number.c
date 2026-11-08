/*
** parse_number(value) SQLite extension - fast C version.
**
** Same rules as ../c/parse_number.c, but decimals avoid strtod():
**
**   One pass validates the text and accumulates up to 19 significant digits
**   into a uint64 "mantissa", counting digits after the '.'.  If the mantissa
**   is <= 2^53 and there are <= 22 fractional digits, both mantissa and
**   10^frac are exactly representable doubles, so a single IEEE division
**   gives the correctly rounded result (Clinger's fast path - the same first
**   step Rust's f64::from_str takes).  Anything else (very long numbers)
**   falls back to a second pass that builds a clean buffer for strtod().
**
** Build: gcc -O2 -fPIC -shared parse_number.c -o parse_number.so
*/
#include <sqlite3ext.h>
SQLITE_EXTENSION_INIT1
#include <stdlib.h>
#include <stdint.h>
#include <float.h>

/* The fast path relies on doubles being evaluated at double precision
** (not x87 80-bit extended), otherwise the division can double-round. */
#if defined(FLT_EVAL_METHOD) && FLT_EVAL_METHOD == 0
#  define FAST_PATH_OK 1
#else
#  define FAST_PATH_OK 0
#endif

static const double POW10[23] = {
  1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11,
  1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22
};

static int is_space(unsigned char c){
  return c==' ' || c=='\t' || c=='\n' || c=='\r';
}

/* Slow path: rebuild "-digits.digits" and hand it to strtod.  Input has
** already been validated by the fast pass. */
static void parse_slow(sqlite3_context *ctx, const unsigned char *s, int i, int end){
  char stackbuf[128];
  char *buf = stackbuf;
  if( end - i + 1 > (int)sizeof(stackbuf) ){
    buf = sqlite3_malloc(end - i + 1);
    if( buf==0 ){ sqlite3_result_error_nomem(ctx); return; }
  }
  int len = 0;
  for( ; i<end; i++ ){
    unsigned char c = s[i];
    if( c==0xC2 ){ i++; continue; }                     /* £ (validated) */
    if( (c>='0' && c<='9') || c=='.' || c=='-' ) buf[len++] = (char)c;
  }
  buf[len] = 0;
  sqlite3_result_double(ctx, strtod(buf, 0));
  if( buf!=stackbuf ) sqlite3_free(buf);
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

  int i = 0, end = n;
  while( i<end && is_space(s[i]) ) i++;
  while( end>i && is_space(s[end-1]) ) end--;
  const int start = i;

  uint64_t mant = 0;   /* first <=19 significant digits */
  int nsig = 0;        /* significant digits seen (leading zeros excluded) */
  int ndigits = 0;     /* all digits seen */
  int frac = 0;        /* digits after '.' */
  int seen_dot = 0, neg = 0, emitted = 0;   /* emitted: '-', digit or '.' seen */

  for( ; i<end; i++ ){
    unsigned char c = s[i];
    unsigned d = (unsigned)c - '0';
    if( d<10 ){
      ndigits++;
      frac += seen_dot;
      if( mant!=0 || d!=0 ){
        if( nsig<19 ) mant = mant*10 + d;
        nsig++;
      }
      emitted = 1;
      continue;
    }
    if( c==',' || c=='$' || c=='%' ) continue;
    if( c==0xC2 && i+1<end && s[i+1]==0xA3 ){ i++; continue; }   /* £ */
    if( c=='.' && !seen_dot ){ seen_dot = 1; emitted = 1; continue; }
    if( c=='-' && !emitted ){ neg = 1; emitted = 1; continue; }
    return;   /* invalid -> NULL */
  }
  if( ndigits==0 ) return;

  if( !seen_dot ){
    if( nsig<=19 ){
      if( mant <= (uint64_t)INT64_MAX ){
        sqlite3_result_int64(ctx, neg ? -(sqlite3_int64)mant : (sqlite3_int64)mant);
        return;
      }
      if( neg && mant == (uint64_t)INT64_MAX + 1 ){
        sqlite3_result_int64(ctx, INT64_MIN);
        return;
      }
      /* 19-digit integer above INT64_MAX: exact as uint64, but needs the
      ** slow path for correct rounding to double. */
    }
  }else if( FAST_PATH_OK && nsig<=19 && mant <= ((uint64_t)1<<53) && frac<=22 ){
    double v = (double)mant / POW10[frac];
    sqlite3_result_double(ctx, neg ? -v : v);
    return;
  }
  parse_slow(ctx, s, start, end);
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
