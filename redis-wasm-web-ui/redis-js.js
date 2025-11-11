/**
 * JavaScript Redis Implementation
 * A lightweight in-browser implementation of core Redis data structures and commands
 */

class RedisJS {
    constructor() {
        this.data = new Map();
        this.expiry = new Map();
        this.lists = new Map();
        this.sets = new Map();
        this.hashes = new Map();
        this.sortedSets = new Map();
    }

    // Helper: Check if key has expired
    _isExpired(key) {
        if (this.expiry.has(key)) {
            const expiryTime = this.expiry.get(key);
            if (Date.now() >= expiryTime) {
                this._delete(key);
                return true;
            }
        }
        return false;
    }

    // Helper: Delete a key from all data structures
    _delete(key) {
        this.data.delete(key);
        this.expiry.delete(key);
        this.lists.delete(key);
        this.sets.delete(key);
        this.hashes.delete(key);
        this.sortedSets.delete(key);
    }

    // String Commands
    set(key, value, options = {}) {
        if (this._isExpired(key)) {}

        const oldValue = this.data.get(key);

        // Handle NX (only set if not exists)
        if (options.nx && this.data.has(key)) {
            return null;
        }

        // Handle XX (only set if exists)
        if (options.xx && !this.data.has(key)) {
            return null;
        }

        this.data.set(key, String(value));

        // Handle expiration
        if (options.ex) {
            this.expiry.set(key, Date.now() + options.ex * 1000);
        } else if (options.px) {
            this.expiry.set(key, Date.now() + options.px);
        } else if (options.exat) {
            this.expiry.set(key, options.exat * 1000);
        } else if (options.pxat) {
            this.expiry.set(key, options.pxat);
        } else if (!options.keepttl) {
            this.expiry.delete(key);
        }

        // Handle GET option
        if (options.get) {
            return oldValue || null;
        }

        return 'OK';
    }

    get(key) {
        if (this._isExpired(key)) return null;
        return this.data.get(key) || null;
    }

    del(...keys) {
        let count = 0;
        for (const key of keys) {
            if (this.data.has(key) || this.lists.has(key) || this.sets.has(key) ||
                this.hashes.has(key) || this.sortedSets.has(key)) {
                this._delete(key);
                count++;
            }
        }
        return count;
    }

    exists(...keys) {
        let count = 0;
        for (const key of keys) {
            if (!this._isExpired(key)) {
                if (this.data.has(key) || this.lists.has(key) || this.sets.has(key) ||
                    this.hashes.has(key) || this.sortedSets.has(key)) {
                    count++;
                }
            }
        }
        return count;
    }

    keys(pattern) {
        const allKeys = [
            ...this.data.keys(),
            ...this.lists.keys(),
            ...this.sets.keys(),
            ...this.hashes.keys(),
            ...this.sortedSets.keys()
        ];

        // Simple pattern matching (* wildcard only)
        const regex = new RegExp('^' + pattern.replace(/\*/g, '.*') + '$');
        return allKeys.filter(key => !this._isExpired(key) && regex.test(key));
    }

    expire(key, seconds) {
        if (this._isExpired(key)) return 0;
        if (this.exists(key) === 0) return 0;
        this.expiry.set(key, Date.now() + seconds * 1000);
        return 1;
    }

    ttl(key) {
        if (this._isExpired(key)) return -2;
        if (this.exists(key) === 0) return -2;
        if (!this.expiry.has(key)) return -1;
        return Math.ceil((this.expiry.get(key) - Date.now()) / 1000);
    }

    incr(key) {
        const value = this.get(key);
        const numValue = value === null ? 0 : parseInt(value);
        if (isNaN(numValue)) {
            throw new Error('ERR value is not an integer or out of range');
        }
        const newValue = numValue + 1;
        this.set(key, String(newValue));
        return newValue;
    }

    decr(key) {
        const value = this.get(key);
        const numValue = value === null ? 0 : parseInt(value);
        if (isNaN(numValue)) {
            throw new Error('ERR value is not an integer or out of range');
        }
        const newValue = numValue - 1;
        this.set(key, String(newValue));
        return newValue;
    }

    incrby(key, increment) {
        const value = this.get(key);
        const numValue = value === null ? 0 : parseInt(value);
        if (isNaN(numValue) || isNaN(increment)) {
            throw new Error('ERR value is not an integer or out of range');
        }
        const newValue = numValue + parseInt(increment);
        this.set(key, String(newValue));
        return newValue;
    }

    append(key, value) {
        const current = this.get(key) || '';
        const newValue = current + value;
        this.set(key, newValue);
        return newValue.length;
    }

    strlen(key) {
        const value = this.get(key);
        return value ? value.length : 0;
    }

    // List Commands
    lpush(key, ...values) {
        if (!this.lists.has(key)) {
            this.lists.set(key, []);
        }
        const list = this.lists.get(key);
        list.unshift(...values.reverse());
        return list.length;
    }

    rpush(key, ...values) {
        if (!this.lists.has(key)) {
            this.lists.set(key, []);
        }
        const list = this.lists.get(key);
        list.push(...values);
        return list.length;
    }

    lpop(key, count = 1) {
        if (!this.lists.has(key)) return null;
        const list = this.lists.get(key);
        if (list.length === 0) return null;

        if (count === 1) {
            return list.shift();
        }
        const result = [];
        for (let i = 0; i < count && list.length > 0; i++) {
            result.push(list.shift());
        }
        return result;
    }

    rpop(key, count = 1) {
        if (!this.lists.has(key)) return null;
        const list = this.lists.get(key);
        if (list.length === 0) return null;

        if (count === 1) {
            return list.pop();
        }
        const result = [];
        for (let i = 0; i < count && list.length > 0; i++) {
            result.push(list.pop());
        }
        return result;
    }

    llen(key) {
        if (!this.lists.has(key)) return 0;
        return this.lists.get(key).length;
    }

    lrange(key, start, stop) {
        if (!this.lists.has(key)) return [];
        const list = this.lists.get(key);
        const len = list.length;

        // Handle negative indices
        if (start < 0) start = len + start;
        if (stop < 0) stop = len + stop;

        // Clamp to valid range
        start = Math.max(0, start);
        stop = Math.min(len - 1, stop);

        if (start > stop) return [];
        return list.slice(start, stop + 1);
    }

    // Set Commands
    sadd(key, ...members) {
        if (!this.sets.has(key)) {
            this.sets.set(key, new Set());
        }
        const set = this.sets.get(key);
        let added = 0;
        for (const member of members) {
            if (!set.has(member)) {
                set.add(member);
                added++;
            }
        }
        return added;
    }

    srem(key, ...members) {
        if (!this.sets.has(key)) return 0;
        const set = this.sets.get(key);
        let removed = 0;
        for (const member of members) {
            if (set.delete(member)) {
                removed++;
            }
        }
        return removed;
    }

    smembers(key) {
        if (!this.sets.has(key)) return [];
        return Array.from(this.sets.get(key));
    }

    sismember(key, member) {
        if (!this.sets.has(key)) return 0;
        return this.sets.get(key).has(member) ? 1 : 0;
    }

    scard(key) {
        if (!this.sets.has(key)) return 0;
        return this.sets.get(key).size;
    }

    // Hash Commands
    hset(key, ...args) {
        if (!this.hashes.has(key)) {
            this.hashes.set(key, new Map());
        }
        const hash = this.hashes.get(key);
        let added = 0;

        for (let i = 0; i < args.length; i += 2) {
            const field = args[i];
            const value = args[i + 1];
            if (!hash.has(field)) {
                added++;
            }
            hash.set(field, value);
        }
        return added;
    }

    hget(key, field) {
        if (!this.hashes.has(key)) return null;
        const hash = this.hashes.get(key);
        return hash.get(field) || null;
    }

    hgetall(key) {
        if (!this.hashes.has(key)) return [];
        const hash = this.hashes.get(key);
        const result = [];
        for (const [field, value] of hash.entries()) {
            result.push(field, value);
        }
        return result;
    }

    hdel(key, ...fields) {
        if (!this.hashes.has(key)) return 0;
        const hash = this.hashes.get(key);
        let deleted = 0;
        for (const field of fields) {
            if (hash.delete(field)) {
                deleted++;
            }
        }
        return deleted;
    }

    hexists(key, field) {
        if (!this.hashes.has(key)) return 0;
        return this.hashes.get(key).has(field) ? 1 : 0;
    }

    hlen(key) {
        if (!this.hashes.has(key)) return 0;
        return this.hashes.get(key).size;
    }

    hkeys(key) {
        if (!this.hashes.has(key)) return [];
        return Array.from(this.hashes.get(key).keys());
    }

    hvals(key) {
        if (!this.hashes.has(key)) return [];
        return Array.from(this.hashes.get(key).values());
    }

    // Sorted Set Commands (simplified)
    zadd(key, ...args) {
        if (!this.sortedSets.has(key)) {
            this.sortedSets.set(key, new Map());
        }
        const zset = this.sortedSets.get(key);
        let added = 0;

        for (let i = 0; i < args.length; i += 2) {
            const score = parseFloat(args[i]);
            const member = args[i + 1];
            if (isNaN(score)) {
                throw new Error('ERR value is not a valid float');
            }
            if (!zset.has(member)) {
                added++;
            }
            zset.set(member, score);
        }
        return added;
    }

    zscore(key, member) {
        if (!this.sortedSets.has(key)) return null;
        const zset = this.sortedSets.get(key);
        const score = zset.get(member);
        return score !== undefined ? score : null;
    }

    zcard(key) {
        if (!this.sortedSets.has(key)) return 0;
        return this.sortedSets.get(key).size;
    }

    zrange(key, start, stop, withscores = false) {
        if (!this.sortedSets.has(key)) return [];
        const zset = this.sortedSets.get(key);

        // Sort by score
        const sorted = Array.from(zset.entries()).sort((a, b) => a[1] - b[1]);

        const len = sorted.length;
        if (start < 0) start = len + start;
        if (stop < 0) stop = len + stop;

        start = Math.max(0, start);
        stop = Math.min(len - 1, stop);

        if (start > stop) return [];

        const slice = sorted.slice(start, stop + 1);
        if (withscores) {
            const result = [];
            for (const [member, score] of slice) {
                result.push(member, score);
            }
            return result;
        }
        return slice.map(([member]) => member);
    }

    // Generic Commands
    type(key) {
        if (this._isExpired(key)) return 'none';
        if (this.data.has(key)) return 'string';
        if (this.lists.has(key)) return 'list';
        if (this.sets.has(key)) return 'set';
        if (this.hashes.has(key)) return 'hash';
        if (this.sortedSets.has(key)) return 'zset';
        return 'none';
    }

    flushdb() {
        this.data.clear();
        this.expiry.clear();
        this.lists.clear();
        this.sets.clear();
        this.hashes.clear();
        this.sortedSets.clear();
        return 'OK';
    }

    flushall() {
        return this.flushdb();
    }

    dbsize() {
        return this.data.size + this.lists.size + this.sets.size +
               this.hashes.size + this.sortedSets.size;
    }

    ping(message) {
        return message || 'PONG';
    }

    echo(message) {
        return message;
    }

    info() {
        return `# Server
redis_version:js-1.0.0
redis_mode:standalone
os:Browser
arch_bits:64

# Clients
connected_clients:1

# Memory
used_memory:${JSON.stringify({data: this.data.size}).length}

# Stats
total_commands_processed:0
instantaneous_ops_per_sec:0

# Keyspace
db0:keys=${this.dbsize()},expires=${this.expiry.size}`;
    }
}
