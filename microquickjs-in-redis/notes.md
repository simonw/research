# Redis JavaScript Scripting - Development Notes

## Project Goal
Integrate JavaScript (via mquickjs) as a scripting language in Redis, similar to Lua.

## Initial Exploration

### Repositories Cloned
- Redis: /tmp/redis
- mquickjs: /tmp/mquickjs

### Next Steps
1. Understand Redis scripting architecture (how Lua is integrated)
2. Understand mquickjs API
3. Decide on integration approach:
   - Option A: Redis Module (loadable, non-invasive)
   - Option B: Patch Redis core (more invasive but deeper integration)

---

## Exploration Log

### 2024-12-25: Starting exploration

#### Redis Scripting Architecture
- `script.h` and `script.c` provide the scripting framework
- Lua is deeply integrated via `script_lua.c` and `function_lua.c`
- Key structures:
  - `scriptRunCtx` - context for running scripts
  - Commands: EVAL, EVALSHA, FUNCTION LOAD, etc.

#### Redis Module API
- `redismodule.h` is the public API for loadable modules
- Modules can register commands with `RedisModule_CreateCommand`
- Can call Redis commands via `RedisModule_Call`
- Example modules in `/tmp/redis/src/modules/`

#### mquickjs Architecture
- Minimal JavaScript engine (ES5 subset)
- Very low memory footprint (works with as little as 10KB RAM)
- Key API:
  - `JS_NewContext(mem_buf, mem_size, stdlib)` - create context
  - `JS_Eval(ctx, code, len, filename, flags)` - run JS code
  - `JS_ToCString(ctx, val, buf)` - convert value to C string
  - Uses tracing GC, no explicit `JS_FreeValue` needed
  - Custom C functions can be added via `JSCFunctionDef`

#### Design Decision: Redis Module Approach
A Redis module is the best approach because:
1. Non-invasive - no need to patch Redis core
2. Easy to install/uninstall
3. Clear separation of concerns
4. Can ship standalone

The module will:
1. Provide `JS.EVAL script numkeys key1 key2 ... arg1 arg2 ...` command
2. Expose `redis.call()` and `redis.pcall()` inside JavaScript
3. Expose KEYS and ARGV arrays
4. Support JS.LOAD / JS.CALL for cached scripts (like EVALSHA)

---

## Implementation Log

### 2025-12-25: Implementation Complete

#### Module Structure Created
- `src/redis_js.c` - Main module (~830 lines)
- `src/redis_js_stdlib.c` - JavaScript stdlib with redis object
- `Makefile` - Build system
- `tests/run_tests.sh` - Automated test suite (41 tests)

#### Key Implementation Details

##### Script Wrapping
JavaScript doesn't allow bare `return` statements outside functions (unlike Lua).
Solution: Wrap user scripts in an IIFE:
```javascript
(function(){
  <user script>
})()
```
This allows `return 42` to work just like in Lua's EVAL.

##### mquickjs stdlib Integration
mquickjs uses a pre-compiled stdlib. We needed to:
1. Copy the stdlib definitions from `mqjs_stdlib.c`
2. Add our `redis` object with call/pcall/log/sha1hex methods
3. Use `mquickjs_build` to compile to a header file

##### Type Conversions
- Redis replies -> JS values: Handled in `reply_to_js()`
  - String/Verbatim -> JS string
  - Integer -> JS int64
  - Double -> JS float64
  - Bool -> JS bool
  - Null -> JS null
  - Array/Set -> JS array
  - Map -> JS object
  - Error -> JS exception

- JS values -> Redis replies: Handled in `js_to_reply()`
  - null/undefined -> NULL reply
  - bool -> integer (0/1)
  - int/number -> integer/double reply
  - string -> string reply
  - array -> array reply
  - object -> string (via toString)

##### Script Caching
- SHA1 hash computed for each script
- Simple linked-list cache (ScriptEntry)
- JS.LOAD adds to cache, returns SHA1
- JS.CALL looks up by SHA1
- JS.EXISTS checks cache
- JS.FLUSH clears cache

##### Thread Safety
Using thread-local globals for current Redis context:
```c
static RedisModuleCtx *current_rctx = NULL;
static RedisModuleString **current_keys = NULL;
static int current_numkeys = 0;
```
This allows redis.call() to access the Redis context from within JS callbacks.

#### Challenges Overcome

1. **Function order dependency**: C functions must be defined BEFORE including
   the generated stdlib header that references them.

2. **RedisModule_CallReplyMapElement signature**: Takes 4 arguments (reply, idx,
   &key, &val), not 3 as initially coded.

3. **Bash arithmetic exit codes**: `((TESTS_PASSED++))` returns 1 when value is 0,
   causing test script to exit with `set -e`. Fixed by using `$((VAR + 1))` syntax.

#### Test Results
All 41 tests passing:
- Basic JS.EVAL (return types, arithmetic)
- Arrays and objects
- KEYS and ARGV access
- redis.call() for SET, GET, INCR, etc.
- redis.pcall() for protected calls
- JS.LOAD/JS.CALL script caching
- JS.EXISTS and JS.FLUSH
- Error handling (syntax and runtime errors)
- Complex scripts (loops, functions, recursion)
- redis.log() and redis.sha1hex()

#### Files Produced
- `redis-js.so` - The loadable Redis module (1.3MB)
- Source code in `src/`
- Tests in `tests/`
- Documentation in `README.md`

#### What Works
- Full JavaScript (ES5) execution in Redis
- All Redis commands via redis.call()
- Script caching via SHA1
- Error handling and logging
- Type conversions between JS and Redis

#### Limitations
- 256KB memory per script execution
- No ES6+ features (mquickjs is ES5)
- No async/await (synchronous execution only)
- Script cache is in-memory (lost on restart)

