# Redis JavaScript Module

A Redis module that adds JavaScript scripting support using the [mquickjs](https://github.com/bellard/mquickjs) engine - a minimal, fast JavaScript engine optimized for embedded use.

This provides functionality similar to Redis's built-in Lua scripting, but using JavaScript instead.

## Features

- **JS.EVAL** - Execute JavaScript scripts with KEYS and ARGV arrays
- **JS.LOAD / JS.CALL** - Cache scripts by SHA1 hash and call them by reference
- **JS.EXISTS** - Check if a script is cached
- **JS.FLUSH** - Clear the script cache
- **redis.call()** - Call any Redis command from JavaScript
- **redis.pcall()** - Protected call (returns error object instead of throwing)
- **redis.log()** - Log messages to Redis log
- **redis.sha1hex()** - Compute SHA1 hash of a string

## Requirements

- Redis 7.0+ (for module API)
- GCC or compatible C compiler
- Make

The build process expects mquickjs and Redis source code in /tmp:
- `/tmp/mquickjs` - mquickjs library
- `/tmp/redis` - Redis source (for headers)

## Building

```bash
# Clone the mquickjs library (if not already cloned)
git clone https://github.com/bellard/mquickjs /tmp/mquickjs

# Clone Redis (for headers only)
git clone https://github.com/redis/redis /tmp/redis

# Build the module
make

# This creates redis-js.so
```

## Installation

Load the module when starting Redis:

```bash
redis-server --loadmodule /path/to/redis-js.so
```

Or add to redis.conf:

```
loadmodule /path/to/redis-js.so
```

## Usage

### Basic Evaluation

```bash
# Simple return value
redis-cli JS.EVAL "return 42" 0
# Returns: 42

# String manipulation
redis-cli JS.EVAL "return 'Hello, ' + ARGV[0]" 0 "World"
# Returns: "Hello, World"

# Math operations
redis-cli JS.EVAL "return 10 + 32" 0
# Returns: 42

# Arrays
redis-cli JS.EVAL "return [1, 2, 3]" 0
# Returns: 1) 1  2) 2  3) 3
```

### KEYS and ARGV

Like Lua scripting, JavaScript scripts have access to KEYS and ARGV arrays:

```bash
# KEYS contains key names (for cluster compatibility)
# ARGV contains other arguments
redis-cli JS.EVAL "return KEYS[0] + ':' + ARGV[0]" 1 mykey myvalue
# Returns: "mykey:myvalue"

# Access lengths
redis-cli JS.EVAL "return KEYS.length" 2 key1 key2
# Returns: 2
```

### Calling Redis Commands

Use `redis.call()` to execute Redis commands:

```bash
# SET a key
redis-cli JS.EVAL "return redis.call('SET', KEYS[0], ARGV[0])" 1 mykey myvalue
# Returns: OK

# GET a key
redis-cli JS.EVAL "return redis.call('GET', KEYS[0])" 1 mykey
# Returns: "myvalue"

# Multiple commands
redis-cli JS.EVAL "
    redis.call('SET', 'counter', '0');
    redis.call('INCR', 'counter');
    redis.call('INCR', 'counter');
    return redis.call('GET', 'counter');
" 0
# Returns: "2"
```

### Protected Calls (pcall)

`redis.pcall()` returns an error object instead of throwing:

```bash
redis-cli JS.EVAL "
    var result = redis.pcall('GET', 'nonexistent');
    if (result === null) {
        return 'key not found';
    }
    return result;
" 0
# Returns: "key not found"
```

### Script Caching

For frequently used scripts, cache them with JS.LOAD:

```bash
# Load a script, get its SHA1
redis-cli JS.LOAD "return ARGV[0].toUpperCase()"
# Returns: a1b2c3d4e5f6... (40 char SHA1)

# Call by SHA1
redis-cli JS.CALL "a1b2c3d4e5f6..." 0 "hello"
# Returns: "HELLO"

# Check if script exists
redis-cli JS.EXISTS "a1b2c3d4e5f6..."
# Returns: 1

# Clear all cached scripts
redis-cli JS.FLUSH
# Returns: OK
```

### Complex Scripts

JavaScript functions, loops, and objects are fully supported:

```bash
# Factorial function
redis-cli JS.EVAL "
    function factorial(n) {
        return n <= 1 ? 1 : n * factorial(n - 1);
    }
    return factorial(5);
" 0
# Returns: 120

# Sum using loop
redis-cli JS.EVAL "
    var sum = 0;
    for (var i = 1; i <= 10; i++) {
        sum += i;
    }
    return sum;
" 0
# Returns: 55

# Object manipulation
redis-cli JS.EVAL "
    var obj = {a: 1, b: 2, c: 3};
    return obj.a + obj.b + obj.c;
" 0
# Returns: 6
```

### Logging

Log messages to Redis server log:

```bash
redis-cli JS.EVAL "
    redis.log(redis.LOG_WARNING, 'Something important happened');
    return 'done';
" 0
```

Log levels:
- `redis.LOG_DEBUG` (0)
- `redis.LOG_VERBOSE` (1)
- `redis.LOG_NOTICE` (2)
- `redis.LOG_WARNING` (3)

### SHA1 Hashing

```bash
redis-cli JS.EVAL "return redis.sha1hex('hello')" 0
# Returns: "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d"
```

## Command Reference

### JS.EVAL

```
JS.EVAL script numkeys [key ...] [arg ...]
```

Execute a JavaScript script. The script is wrapped in a function, so you can use `return` statements directly.

### JS.CALL

```
JS.CALL sha1 numkeys [key ...] [arg ...]
```

Execute a cached script by its SHA1 hash. Returns NOSCRIPT error if not found.

### JS.LOAD

```
JS.LOAD script
```

Cache a script and return its SHA1 hash.

### JS.EXISTS

```
JS.EXISTS sha1 [sha1 ...]
```

Check if scripts exist in the cache. Returns an array of 1/0 values.

### JS.FLUSH

```
JS.FLUSH [ASYNC|SYNC]
```

Clear the script cache.

## JavaScript Environment

Scripts run in a minimal JavaScript environment with:

- Standard objects: Object, Array, String, Number, Boolean, Math, JSON
- Global functions: parseInt, parseFloat, isNaN, isFinite
- redis object: redis.call, redis.pcall, redis.log, redis.sha1hex
- Script globals: KEYS[], ARGV[]

### Differences from Lua Scripting

| Feature | Lua | JavaScript |
|---------|-----|------------|
| Syntax | Lua | JavaScript (ES5) |
| Arrays | 1-indexed | 0-indexed |
| Nil | `nil` | `null` |
| Booleans | `false`, `nil` are falsy | `false`, `0`, `null`, `undefined`, `""` are falsy |
| Error handling | pcall | try/catch + redis.pcall |
| String concat | `..` | `+` |

### Memory Limits

Each script execution uses a fresh JavaScript context with 256KB of memory. This is sufficient for most scripts but limits very large data manipulations.

## Testing

Run the test suite:

```bash
# Requires Redis to be installed
./tests/run_tests.sh
```

## Architecture

The module consists of:

1. **redis_js.c** - Main module code:
   - Redis command implementations (JS.EVAL, JS.CALL, etc.)
   - Script caching with SHA1 hashing
   - JavaScript-to-Redis type conversions

2. **redis_js_stdlib.c** - JavaScript standard library definition:
   - Includes standard JS classes
   - Adds `redis` object with call/pcall/log/sha1hex

3. **Makefile** - Build system:
   - Compiles mquickjs library
   - Generates stdlib header using mquickjs_build
   - Links everything into redis-js.so

## License

MIT License

## Credits

- [mquickjs](https://github.com/bellard/mquickjs) - Minimal JavaScript engine by Fabrice Bellard
- [Redis](https://redis.io/) - In-memory data store
