/*
 * Redis JavaScript Module
 *
 * Provides JavaScript scripting for Redis using mquickjs engine.
 * Similar to Lua's EVAL command, but with JavaScript.
 *
 * Commands:
 *   JS.EVAL <script> <numkeys> [key ...] [arg ...]
 *   JS.CALL <sha1> <numkeys> [key ...] [arg ...]
 *   JS.LOAD <script>
 *   JS.EXISTS <sha1> [sha1 ...]
 *   JS.FLUSH [ASYNC|SYNC]
 *
 * Inside JavaScript, you have access to:
 *   - redis.call(cmd, arg1, arg2, ...) - Call Redis command
 *   - redis.pcall(cmd, arg1, arg2, ...) - Call Redis command (protected)
 *   - redis.log(level, message) - Log a message
 *   - redis.sha1hex(str) - Get SHA1 hex of a string
 *   - KEYS[] - Array of key arguments
 *   - ARGV[] - Array of other arguments
 *
 * MIT License - see LICENSE file
 */

#define REDISMODULE_EXPERIMENTAL_API
#include "redismodule.h"

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <ctype.h>
#include <sys/time.h>

/* mquickjs headers - must include priv header for stdlib */
#include "mquickjs.h"

/* Default memory size for JS context (256KB) */
#define JS_MEM_SIZE (256 * 1024)

/* Script cache entry */
typedef struct ScriptEntry {
    char sha1[41];
    char *script;
    size_t script_len;
    struct ScriptEntry *next;
} ScriptEntry;

/* Module global state */
static ScriptEntry *script_cache = NULL;
static int script_count = 0;

/* Thread-local storage for current Redis context */
static RedisModuleCtx *current_rctx = NULL;
static RedisModuleString **current_keys = NULL;
static int current_numkeys = 0;
static RedisModuleString **current_argv = NULL;
static int current_argc = 0;

/* SHA1 implementation */
static void sha1_to_hex(const unsigned char *sha1, char *hex) {
    static const char hexchars[] = "0123456789abcdef";
    for (int i = 0; i < 20; i++) {
        hex[i * 2] = hexchars[(sha1[i] >> 4) & 0xf];
        hex[i * 2 + 1] = hexchars[sha1[i] & 0xf];
    }
    hex[40] = '\0';
}

typedef struct {
    uint32_t state[5];
    uint32_t count[2];
    unsigned char buffer[64];
} SHA1_CTX;

#define rol(value, bits) (((value) << (bits)) | ((value) >> (32 - (bits))))

static void SHA1Transform(uint32_t state[5], const unsigned char buffer[64]) {
    uint32_t a, b, c, d, e;
    uint32_t block[80];

    for (int i = 0; i < 16; i++) {
        block[i] = ((uint32_t)buffer[i*4] << 24) | ((uint32_t)buffer[i*4+1] << 16) |
                   ((uint32_t)buffer[i*4+2] << 8) | (uint32_t)buffer[i*4+3];
    }
    for (int i = 16; i < 80; i++) {
        block[i] = rol(block[i-3] ^ block[i-8] ^ block[i-14] ^ block[i-16], 1);
    }

    a = state[0]; b = state[1]; c = state[2]; d = state[3]; e = state[4];

    for (int i = 0; i < 20; i++) {
        uint32_t t = rol(a, 5) + ((b & c) | ((~b) & d)) + e + block[i] + 0x5A827999;
        e = d; d = c; c = rol(b, 30); b = a; a = t;
    }
    for (int i = 20; i < 40; i++) {
        uint32_t t = rol(a, 5) + (b ^ c ^ d) + e + block[i] + 0x6ED9EBA1;
        e = d; d = c; c = rol(b, 30); b = a; a = t;
    }
    for (int i = 40; i < 60; i++) {
        uint32_t t = rol(a, 5) + ((b & c) | (b & d) | (c & d)) + e + block[i] + 0x8F1BBCDC;
        e = d; d = c; c = rol(b, 30); b = a; a = t;
    }
    for (int i = 60; i < 80; i++) {
        uint32_t t = rol(a, 5) + (b ^ c ^ d) + e + block[i] + 0xCA62C1D6;
        e = d; d = c; c = rol(b, 30); b = a; a = t;
    }

    state[0] += a; state[1] += b; state[2] += c; state[3] += d; state[4] += e;
}

static void SHA1Init(SHA1_CTX *context) {
    context->state[0] = 0x67452301;
    context->state[1] = 0xEFCDAB89;
    context->state[2] = 0x98BADCFE;
    context->state[3] = 0x10325476;
    context->state[4] = 0xC3D2E1F0;
    context->count[0] = context->count[1] = 0;
}

static void SHA1Update(SHA1_CTX *context, const unsigned char *data, size_t len) {
    size_t i, j;
    j = (context->count[0] >> 3) & 63;
    if ((context->count[0] += (uint32_t)(len << 3)) < (len << 3))
        context->count[1]++;
    context->count[1] += (uint32_t)(len >> 29);
    if ((j + len) > 63) {
        memcpy(&context->buffer[j], data, (i = 64-j));
        SHA1Transform(context->state, context->buffer);
        for (; i + 63 < len; i += 64)
            SHA1Transform(context->state, &data[i]);
        j = 0;
    } else {
        i = 0;
    }
    memcpy(&context->buffer[j], &data[i], len - i);
}

static void SHA1Final(unsigned char digest[20], SHA1_CTX *context) {
    unsigned char finalcount[8];
    for (int i = 0; i < 8; i++)
        finalcount[i] = (unsigned char)((context->count[(i >= 4 ? 0 : 1)] >> ((3-(i & 3)) * 8)) & 255);
    SHA1Update(context, (unsigned char *)"\200", 1);
    while ((context->count[0] & 504) != 448)
        SHA1Update(context, (unsigned char *)"\0", 1);
    SHA1Update(context, finalcount, 8);
    for (int i = 0; i < 20; i++)
        digest[i] = (unsigned char)((context->state[i>>2] >> ((3-(i & 3)) * 8)) & 255);
}

static void compute_sha1(const char *script, size_t len, char *sha1hex) {
    SHA1_CTX ctx;
    unsigned char sha1[20];
    SHA1Init(&ctx);
    SHA1Update(&ctx, (const unsigned char *)script, len);
    SHA1Final(sha1, &ctx);
    sha1_to_hex(sha1, sha1hex);
}

/* Script cache functions */
static ScriptEntry *find_script(const char *sha1) {
    ScriptEntry *entry = script_cache;
    while (entry) {
        if (strcmp(entry->sha1, sha1) == 0)
            return entry;
        entry = entry->next;
    }
    return NULL;
}

static ScriptEntry *add_script(const char *sha1, const char *script, size_t len) {
    ScriptEntry *entry = RedisModule_Alloc(sizeof(ScriptEntry));
    if (!entry) return NULL;

    memcpy(entry->sha1, sha1, 41);
    entry->script = RedisModule_Alloc(len + 1);
    if (!entry->script) {
        RedisModule_Free(entry);
        return NULL;
    }
    memcpy(entry->script, script, len);
    entry->script[len] = '\0';
    entry->script_len = len;
    entry->next = script_cache;
    script_cache = entry;
    script_count++;
    return entry;
}

static void clear_script_cache(void) {
    ScriptEntry *entry = script_cache;
    while (entry) {
        ScriptEntry *next = entry->next;
        RedisModule_Free(entry->script);
        RedisModule_Free(entry);
        entry = next;
    }
    script_cache = NULL;
    script_count = 0;
}

/*
 * JavaScript C Functions
 * These are called from JS and must match the mquickjs JSCFunction signature.
 */

/* js_print - console.log implementation */
static JSValue js_print(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) {
    (void)this_val;
    for (int i = 0; i < argc; i++) {
        if (i != 0) printf(" ");
        if (JS_IsString(ctx, argv[i])) {
            JSCStringBuf buf;
            const char *str = JS_ToCString(ctx, argv[i], &buf);
            if (str) printf("%s", str);
        } else {
            JS_PrintValueF(ctx, argv[i], 0);
        }
    }
    printf("\n");
    return JS_UNDEFINED;
}

/* js_date_now - Date.now() implementation */
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) {
    (void)this_val;
    (void)argc;
    (void)argv;
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return JS_NewInt64(ctx, (int64_t)tv.tv_sec * 1000 + (tv.tv_usec / 1000));
}

/* js_date_constructor - Date constructor (minimal) */
static JSValue js_date_constructor(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) {
    (void)this_val;
    (void)argv;
    if (!(argc & FRAME_CF_CTOR))
        return JS_ThrowTypeError(ctx, "must be called with new");
    /* Return a simple Date object - just stores the timestamp */
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return JS_NewInt64(ctx, (int64_t)tv.tv_sec * 1000 + (tv.tv_usec / 1000));
}

/* Convert Redis reply to JavaScript value */
static JSValue reply_to_js(JSContext *ctx, RedisModuleCallReply *reply) {
    if (!reply) {
        return JS_NULL;
    }

    int type = RedisModule_CallReplyType(reply);

    switch (type) {
        case REDISMODULE_REPLY_STRING:
        case REDISMODULE_REPLY_VERBATIM_STRING: {
            size_t len;
            const char *str = RedisModule_CallReplyStringPtr(reply, &len);
            return JS_NewStringLen(ctx, str, len);
        }

        case REDISMODULE_REPLY_INTEGER: {
            long long val = RedisModule_CallReplyInteger(reply);
            return JS_NewInt64(ctx, val);
        }

        case REDISMODULE_REPLY_DOUBLE: {
            double val = RedisModule_CallReplyDouble(reply);
            return JS_NewFloat64(ctx, val);
        }

        case REDISMODULE_REPLY_BOOL: {
            int val = RedisModule_CallReplyBool(reply);
            return JS_NewBool(val);
        }

        case REDISMODULE_REPLY_NULL:
            return JS_NULL;

        case REDISMODULE_REPLY_ARRAY:
        case REDISMODULE_REPLY_SET: {
            size_t len = RedisModule_CallReplyLength(reply);
            JSValue arr = JS_NewArray(ctx, (int)len);
            if (JS_IsException(arr)) return arr;

            for (size_t i = 0; i < len; i++) {
                RedisModuleCallReply *elem = RedisModule_CallReplyArrayElement(reply, i);
                JSValue val = reply_to_js(ctx, elem);
                if (JS_IsException(val)) {
                    return val;
                }
                JS_SetPropertyUint32(ctx, arr, (uint32_t)i, val);
            }
            return arr;
        }

        case REDISMODULE_REPLY_MAP: {
            JSValue obj = JS_NewObject(ctx);
            if (JS_IsException(obj)) return obj;

            size_t len = RedisModule_CallReplyLength(reply);
            for (size_t i = 0; i < len; i++) {
                RedisModuleCallReply *key_reply, *val_reply;
                RedisModule_CallReplyMapElement(reply, i, &key_reply, &val_reply);

                size_t key_len;
                const char *key_str = RedisModule_CallReplyStringPtr(key_reply, &key_len);
                char *key_cstr = RedisModule_Alloc(key_len + 1);
                memcpy(key_cstr, key_str, key_len);
                key_cstr[key_len] = '\0';

                JSValue jsval = reply_to_js(ctx, val_reply);
                if (JS_IsException(jsval)) {
                    RedisModule_Free(key_cstr);
                    return jsval;
                }
                JS_SetPropertyStr(ctx, obj, key_cstr, jsval);
                RedisModule_Free(key_cstr);
            }
            return obj;
        }

        case REDISMODULE_REPLY_ERROR: {
            size_t len;
            const char *str = RedisModule_CallReplyStringPtr(reply, &len);
            char *msg = RedisModule_Alloc(len + 1);
            memcpy(msg, str, len);
            msg[len] = '\0';
            JSValue err = JS_ThrowError(ctx, JS_CLASS_ERROR, "%s", msg);
            RedisModule_Free(msg);
            return err;
        }

        default:
            return JS_NULL;
    }
}

/* redis.call() - call Redis command, throws on error */
static JSValue js_redis_call(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) {
    (void)this_val;

    if (argc < 1) {
        return JS_ThrowTypeError(ctx, "redis.call requires at least one argument");
    }

    if (!current_rctx) {
        return JS_ThrowInternalError(ctx, "No Redis context available");
    }

    /* Get command name */
    JSCStringBuf cmd_buf;
    const char *cmd = JS_ToCString(ctx, argv[0], &cmd_buf);
    if (!cmd) {
        return JS_ThrowTypeError(ctx, "Command must be a string");
    }

    /* Build arguments array */
    RedisModuleString **args = NULL;
    if (argc > 1) {
        args = RedisModule_Alloc(sizeof(RedisModuleString *) * (size_t)(argc - 1));
        for (int i = 1; i < argc; i++) {
            JSCStringBuf arg_buf;
            size_t arg_len;
            const char *arg = JS_ToCStringLen(ctx, &arg_len, argv[i], &arg_buf);
            if (!arg) {
                for (int j = 0; j < i - 1; j++) {
                    RedisModule_FreeString(current_rctx, args[j]);
                }
                RedisModule_Free(args);
                return JS_ThrowTypeError(ctx, "Argument must be convertible to string");
            }
            args[i - 1] = RedisModule_CreateString(current_rctx, arg, arg_len);
        }
    }

    /* Call Redis command */
    RedisModuleCallReply *reply;
    if (argc > 1) {
        reply = RedisModule_Call(current_rctx, cmd, "v", args, (size_t)(argc - 1));
    } else {
        reply = RedisModule_Call(current_rctx, cmd, "");
    }

    /* Free arguments */
    if (args) {
        for (int i = 0; i < argc - 1; i++) {
            RedisModule_FreeString(current_rctx, args[i]);
        }
        RedisModule_Free(args);
    }

    /* Convert reply to JavaScript */
    JSValue result = reply_to_js(ctx, reply);

    if (reply) {
        RedisModule_FreeCallReply(reply);
    }

    return result;
}

/* redis.pcall() - protected call, returns error object instead of throwing */
static JSValue js_redis_pcall(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) {
    (void)this_val;

    if (argc < 1) {
        return JS_ThrowTypeError(ctx, "redis.pcall requires at least one argument");
    }

    if (!current_rctx) {
        return JS_ThrowInternalError(ctx, "No Redis context available");
    }

    JSCStringBuf cmd_buf;
    const char *cmd = JS_ToCString(ctx, argv[0], &cmd_buf);
    if (!cmd) {
        return JS_ThrowTypeError(ctx, "Command must be a string");
    }

    RedisModuleString **args = NULL;
    if (argc > 1) {
        args = RedisModule_Alloc(sizeof(RedisModuleString *) * (size_t)(argc - 1));
        for (int i = 1; i < argc; i++) {
            JSCStringBuf arg_buf;
            size_t arg_len;
            const char *arg = JS_ToCStringLen(ctx, &arg_len, argv[i], &arg_buf);
            if (!arg) {
                for (int j = 0; j < i - 1; j++) {
                    RedisModule_FreeString(current_rctx, args[j]);
                }
                RedisModule_Free(args);
                return JS_ThrowTypeError(ctx, "Argument must be convertible to string");
            }
            args[i - 1] = RedisModule_CreateString(current_rctx, arg, arg_len);
        }
    }

    RedisModuleCallReply *reply;
    if (argc > 1) {
        reply = RedisModule_Call(current_rctx, cmd, "v", args, (size_t)(argc - 1));
    } else {
        reply = RedisModule_Call(current_rctx, cmd, "");
    }

    if (args) {
        for (int i = 0; i < argc - 1; i++) {
            RedisModule_FreeString(current_rctx, args[i]);
        }
        RedisModule_Free(args);
    }

    /* For pcall, convert errors to error objects instead of throwing */
    if (reply && RedisModule_CallReplyType(reply) == REDISMODULE_REPLY_ERROR) {
        size_t len;
        const char *str = RedisModule_CallReplyStringPtr(reply, &len);
        JSValue obj = JS_NewObject(ctx);
        JS_SetPropertyStr(ctx, obj, "err", JS_NewStringLen(ctx, str, len));
        RedisModule_FreeCallReply(reply);
        return obj;
    }

    JSValue result = reply_to_js(ctx, reply);

    if (reply) {
        RedisModule_FreeCallReply(reply);
    }

    return result;
}

/* redis.log() - log a message */
static JSValue js_redis_log(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) {
    (void)this_val;

    if (argc < 2) {
        return JS_ThrowTypeError(ctx, "redis.log requires level and message");
    }

    int level;
    if (JS_ToInt32(ctx, &level, argv[0])) {
        return JS_EXCEPTION;
    }

    JSCStringBuf msg_buf;
    const char *msg = JS_ToCString(ctx, argv[1], &msg_buf);
    if (!msg) {
        return JS_ThrowTypeError(ctx, "Message must be a string");
    }

    const char *level_str;
    switch (level) {
        case 0: level_str = "debug"; break;
        case 1: level_str = "verbose"; break;
        case 2: level_str = "notice"; break;
        case 3: level_str = "warning"; break;
        default: level_str = "notice"; break;
    }

    if (current_rctx) {
        RedisModule_Log(current_rctx, level_str, "JS: %s", msg);
    }

    return JS_UNDEFINED;
}

/* redis.sha1hex() - compute SHA1 of a string */
static JSValue js_redis_sha1hex(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) {
    (void)this_val;

    if (argc < 1) {
        return JS_ThrowTypeError(ctx, "redis.sha1hex requires a string argument");
    }

    JSCStringBuf str_buf;
    size_t len;
    const char *str = JS_ToCStringLen(ctx, &len, argv[0], &str_buf);
    if (!str) {
        return JS_ThrowTypeError(ctx, "Argument must be a string");
    }

    char sha1hex[41];
    compute_sha1(str, len, sha1hex);

    return JS_NewString(ctx, sha1hex);
}

/* Include the generated stdlib header - AFTER function definitions */
#include "redis_js_stdlib.h"

/* Convert JS value to Redis reply */
static int js_to_reply(RedisModuleCtx *rctx, JSContext *ctx, JSValue val) {
    if (JS_IsNull(val) || JS_IsUndefined(val)) {
        RedisModule_ReplyWithNull(rctx);
    } else if (JS_IsBool(val)) {
        int v = JS_VALUE_GET_SPECIAL_VALUE(val);
        RedisModule_ReplyWithLongLong(rctx, v);
    } else if (JS_IsInt(val)) {
        int v = JS_VALUE_GET_INT(val);
        RedisModule_ReplyWithLongLong(rctx, v);
    } else if (JS_IsNumber(ctx, val)) {
        double v;
        JS_ToNumber(ctx, &v, val);
        RedisModule_ReplyWithDouble(rctx, v);
    } else if (JS_IsString(ctx, val)) {
        JSCStringBuf buf;
        size_t len;
        const char *str = JS_ToCStringLen(ctx, &len, val, &buf);
        if (str) {
            RedisModule_ReplyWithStringBuffer(rctx, str, len);
        } else {
            RedisModule_ReplyWithNull(rctx);
        }
    } else if (JS_GetClassID(ctx, val) == JS_CLASS_ARRAY) {
        JSValue len_val = JS_GetPropertyStr(ctx, val, "length");
        int len = 0;
        JS_ToInt32(ctx, &len, len_val);

        RedisModule_ReplyWithArray(rctx, (long)len);
        for (int i = 0; i < len; i++) {
            JSValue elem = JS_GetPropertyUint32(ctx, val, (uint32_t)i);
            js_to_reply(rctx, ctx, elem);
        }
    } else {
        JSValue str_val = JS_ToString(ctx, val);
        if (!JS_IsException(str_val)) {
            JSCStringBuf buf;
            size_t len;
            const char *str = JS_ToCStringLen(ctx, &len, str_val, &buf);
            if (str) {
                RedisModule_ReplyWithStringBuffer(rctx, str, len);
            } else {
                RedisModule_ReplyWithNull(rctx);
            }
        } else {
            RedisModule_ReplyWithNull(rctx);
        }
    }
    return REDISMODULE_OK;
}

/* Execute JavaScript in a fresh context */
static int execute_js(RedisModuleCtx *ctx, const char *script, size_t script_len,
                      RedisModuleString **keys, int numkeys,
                      RedisModuleString **argv, int argc) {
    /* Allocate memory for JS context */
    uint8_t *mem_buf = RedisModule_Alloc(JS_MEM_SIZE);
    if (!mem_buf) {
        RedisModule_ReplyWithError(ctx, "ERR out of memory for JS context");
        return REDISMODULE_ERR;
    }

    /* Create JS context with our stdlib */
    JSContext *js_ctx = JS_NewContext(mem_buf, JS_MEM_SIZE, &js_stdlib);
    if (!js_ctx) {
        RedisModule_Free(mem_buf);
        RedisModule_ReplyWithError(ctx, "ERR failed to create JS context");
        return REDISMODULE_ERR;
    }

    /* Set up global state for callbacks */
    current_rctx = ctx;
    current_keys = keys;
    current_numkeys = numkeys;
    current_argv = argv;
    current_argc = argc;

    /* Get global object */
    JSValue global = JS_GetGlobalObject(js_ctx);

    /* Create KEYS array */
    JSValue keys_arr = JS_NewArray(js_ctx, numkeys);
    if (!JS_IsException(keys_arr)) {
        for (int i = 0; i < numkeys; i++) {
            size_t len;
            const char *key = RedisModule_StringPtrLen(keys[i], &len);
            JS_SetPropertyUint32(js_ctx, keys_arr, (uint32_t)i, JS_NewStringLen(js_ctx, key, len));
        }
        JS_SetPropertyStr(js_ctx, global, "KEYS", keys_arr);
    }

    /* Create ARGV array */
    int argv_count = argc - numkeys;
    if (argv_count < 0) argv_count = 0;
    JSValue argv_arr = JS_NewArray(js_ctx, argv_count);
    if (!JS_IsException(argv_arr)) {
        for (int i = 0; i < argv_count; i++) {
            size_t len;
            const char *arg = RedisModule_StringPtrLen(argv[numkeys + i], &len);
            JS_SetPropertyUint32(js_ctx, argv_arr, (uint32_t)i, JS_NewStringLen(js_ctx, arg, len));
        }
        JS_SetPropertyStr(js_ctx, global, "ARGV", argv_arr);
    }

    /*
     * Wrap script in a function to allow 'return' statements.
     * This makes the syntax similar to Lua's EVAL where bare returns are allowed.
     * The wrapper is: (function(){  <script>  })()
     */
    const char *prefix = "(function(){";
    const char *suffix = "})()";
    size_t prefix_len = strlen(prefix);
    size_t suffix_len = strlen(suffix);
    size_t wrapped_len = prefix_len + script_len + suffix_len;
    char *wrapped_script = RedisModule_Alloc(wrapped_len + 1);
    if (!wrapped_script) {
        current_rctx = NULL;
        JS_FreeContext(js_ctx);
        RedisModule_Free(mem_buf);
        RedisModule_ReplyWithError(ctx, "ERR out of memory for script wrapper");
        return REDISMODULE_ERR;
    }
    memcpy(wrapped_script, prefix, prefix_len);
    memcpy(wrapped_script + prefix_len, script, script_len);
    memcpy(wrapped_script + prefix_len + script_len, suffix, suffix_len);
    wrapped_script[wrapped_len] = '\0';

    /* Evaluate wrapped script */
    JSValue result = JS_Eval(js_ctx, wrapped_script, wrapped_len, "<script>", JS_EVAL_RETVAL);

    RedisModule_Free(wrapped_script);

    int ret = REDISMODULE_OK;

    if (JS_IsException(result)) {
        JSValue exc = JS_GetException(js_ctx);
        JSCStringBuf buf;
        const char *err_str = JS_ToCString(js_ctx, exc, &buf);
        if (err_str) {
            char *err_msg = RedisModule_Alloc(strlen(err_str) + 16);
            sprintf(err_msg, "ERR JS: %s", err_str);
            RedisModule_ReplyWithError(ctx, err_msg);
            RedisModule_Free(err_msg);
        } else {
            RedisModule_ReplyWithError(ctx, "ERR JS execution failed");
        }
        ret = REDISMODULE_ERR;
    } else {
        js_to_reply(ctx, js_ctx, result);
    }

    /* Clean up */
    current_rctx = NULL;
    current_keys = NULL;
    current_numkeys = 0;
    current_argv = NULL;
    current_argc = 0;

    JS_FreeContext(js_ctx);
    RedisModule_Free(mem_buf);

    return ret;
}

/* JS.EVAL command */
int JSEval_RedisCommand(RedisModuleCtx *ctx, RedisModuleString **argv, int argc) {
    if (argc < 3) {
        return RedisModule_WrongArity(ctx);
    }

    size_t script_len;
    const char *script = RedisModule_StringPtrLen(argv[1], &script_len);

    long long numkeys;
    if (RedisModule_StringToLongLong(argv[2], &numkeys) != REDISMODULE_OK || numkeys < 0) {
        return RedisModule_ReplyWithError(ctx, "ERR value is not an integer or out of range");
    }

    if (numkeys > argc - 3) {
        return RedisModule_ReplyWithError(ctx, "ERR Number of keys can't be greater than number of args");
    }

    char sha1[41];
    compute_sha1(script, script_len, sha1);
    if (!find_script(sha1)) {
        add_script(sha1, script, script_len);
    }

    return execute_js(ctx, script, script_len, argv + 3, (int)numkeys, argv + 3, argc - 3);
}

/* JS.CALL command */
int JSCall_RedisCommand(RedisModuleCtx *ctx, RedisModuleString **argv, int argc) {
    if (argc < 3) {
        return RedisModule_WrongArity(ctx);
    }

    size_t sha1_len;
    const char *sha1 = RedisModule_StringPtrLen(argv[1], &sha1_len);

    if (sha1_len != 40) {
        return RedisModule_ReplyWithError(ctx, "NOSCRIPT No matching script. Use JS.EVAL or JS.LOAD.");
    }

    ScriptEntry *entry = find_script(sha1);
    if (!entry) {
        return RedisModule_ReplyWithError(ctx, "NOSCRIPT No matching script. Use JS.EVAL or JS.LOAD.");
    }

    long long numkeys;
    if (RedisModule_StringToLongLong(argv[2], &numkeys) != REDISMODULE_OK || numkeys < 0) {
        return RedisModule_ReplyWithError(ctx, "ERR value is not an integer or out of range");
    }

    if (numkeys > argc - 3) {
        return RedisModule_ReplyWithError(ctx, "ERR Number of keys can't be greater than number of args");
    }

    return execute_js(ctx, entry->script, entry->script_len, argv + 3, (int)numkeys, argv + 3, argc - 3);
}

/* JS.LOAD command */
int JSLoad_RedisCommand(RedisModuleCtx *ctx, RedisModuleString **argv, int argc) {
    if (argc != 2) {
        return RedisModule_WrongArity(ctx);
    }

    size_t script_len;
    const char *script = RedisModule_StringPtrLen(argv[1], &script_len);

    char sha1[41];
    compute_sha1(script, script_len, sha1);

    if (!find_script(sha1)) {
        if (!add_script(sha1, script, script_len)) {
            return RedisModule_ReplyWithError(ctx, "ERR out of memory");
        }
    }

    return RedisModule_ReplyWithStringBuffer(ctx, sha1, 40);
}

/* JS.EXISTS command */
int JSExists_RedisCommand(RedisModuleCtx *ctx, RedisModuleString **argv, int argc) {
    if (argc < 2) {
        return RedisModule_WrongArity(ctx);
    }

    RedisModule_ReplyWithArray(ctx, argc - 1);

    for (int i = 1; i < argc; i++) {
        size_t len;
        const char *sha1 = RedisModule_StringPtrLen(argv[i], &len);
        if (len == 40 && find_script(sha1)) {
            RedisModule_ReplyWithLongLong(ctx, 1);
        } else {
            RedisModule_ReplyWithLongLong(ctx, 0);
        }
    }

    return REDISMODULE_OK;
}

/* JS.FLUSH command */
int JSFlush_RedisCommand(RedisModuleCtx *ctx, RedisModuleString **argv, int argc) {
    (void)argv;
    if (argc > 2) {
        return RedisModule_WrongArity(ctx);
    }

    clear_script_cache();
    return RedisModule_ReplyWithSimpleString(ctx, "OK");
}

/* Module initialization */
int RedisModule_OnLoad(RedisModuleCtx *ctx, RedisModuleString **argv, int argc) {
    REDISMODULE_NOT_USED(argv);
    REDISMODULE_NOT_USED(argc);

    if (RedisModule_Init(ctx, "js", 1, REDISMODULE_APIVER_1) == REDISMODULE_ERR)
        return REDISMODULE_ERR;

    if (RedisModule_CreateCommand(ctx, "js.eval",
            JSEval_RedisCommand, "write deny-oom", 0, 0, 0) == REDISMODULE_ERR)
        return REDISMODULE_ERR;

    if (RedisModule_CreateCommand(ctx, "js.call",
            JSCall_RedisCommand, "write deny-oom", 0, 0, 0) == REDISMODULE_ERR)
        return REDISMODULE_ERR;

    if (RedisModule_CreateCommand(ctx, "js.load",
            JSLoad_RedisCommand, "readonly", 0, 0, 0) == REDISMODULE_ERR)
        return REDISMODULE_ERR;

    if (RedisModule_CreateCommand(ctx, "js.exists",
            JSExists_RedisCommand, "readonly fast", 0, 0, 0) == REDISMODULE_ERR)
        return REDISMODULE_ERR;

    if (RedisModule_CreateCommand(ctx, "js.flush",
            JSFlush_RedisCommand, "write", 0, 0, 0) == REDISMODULE_ERR)
        return REDISMODULE_ERR;

    RedisModule_Log(ctx, "notice", "Redis JavaScript module loaded successfully");

    return REDISMODULE_OK;
}

int RedisModule_OnUnload(RedisModuleCtx *ctx) {
    REDISMODULE_NOT_USED(ctx);
    clear_script_cache();
    return REDISMODULE_OK;
}
