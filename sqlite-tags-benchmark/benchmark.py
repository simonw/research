"""
Benchmark different approaches to implementing tags in SQLite:
1. JSON arrays - store tags as a JSON array column
2. M2M (many-to-many) tables - classic relational approach
3. FTS (Full-Text Search) - use FTS5 to index space-separated tags
4. LIKE queries - store tags as delimited string, query with LIKE
"""

import sqlite3
import json
import random
import time
import statistics
import os

DB_PATH = "/home/user/research/sqlite-tags-benchmark/benchmark.db"
NUM_ROWS = 100_000
NUM_TAGS_VOCAB = 100
MIN_TAGS = 3
MAX_TAGS = 10
NUM_QUERY_ITERATIONS = 100  # repeat each query type this many times for timing

# Generate tag vocabulary
TAGS = [f"tag_{i:03d}" for i in range(NUM_TAGS_VOCAB)]

random.seed(42)


def generate_rows():
    """Generate NUM_ROWS rows, each with 3-10 random tags."""
    rows = []
    for i in range(NUM_ROWS):
        num_tags = random.randint(MIN_TAGS, MAX_TAGS)
        tags = sorted(random.sample(TAGS, num_tags))
        rows.append((i, f"Item {i}", tags))
    return rows


def pick_query_tags(rows):
    """Pick tags for benchmarking queries - some common, some rare combos."""
    # Single tags to query
    single_tags = random.sample(TAGS, 10)

    # Tag pairs for AND queries
    tag_pairs = []
    for _ in range(10):
        t1, t2 = random.sample(TAGS, 2)
        tag_pairs.append((t1, t2))

    # Tag sets for OR queries
    tag_sets = []
    for _ in range(10):
        n = random.randint(2, 5)
        tag_sets.append(random.sample(TAGS, n))

    return single_tags, tag_pairs, tag_sets


def timeit(func, iterations=NUM_QUERY_ITERATIONS):
    """Run func `iterations` times, return (mean_ms, median_ms, min_ms, results_from_first_run)."""
    # Warm up
    result = func()
    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        func()
        elapsed = (time.perf_counter() - start) * 1000
        times.append(elapsed)
    return {
        "mean_ms": statistics.mean(times),
        "median_ms": statistics.median(times),
        "min_ms": min(times),
        "stddev_ms": statistics.stdev(times) if len(times) > 1 else 0,
        "result_count": len(result) if isinstance(result, list) else result,
    }


# ============================================================
# Strategy 1: JSON Arrays
# ============================================================
def setup_json(conn, rows):
    conn.execute("DROP TABLE IF EXISTS items_json")
    conn.execute("""
        CREATE TABLE items_json (
            id INTEGER PRIMARY KEY,
            name TEXT,
            tags JSON
        )
    """)
    conn.executemany(
        "INSERT INTO items_json (id, name, tags) VALUES (?, ?, ?)",
        [(r[0], r[1], json.dumps(r[2])) for r in rows],
    )
    conn.commit()


def query_json_single(conn, tag):
    """Find rows where tags JSON array contains a specific tag."""
    cur = conn.execute(
        """SELECT id FROM items_json
           WHERE EXISTS (
               SELECT 1 FROM json_each(items_json.tags)
               WHERE json_each.value = ?
           )""",
        (tag,),
    )
    return cur.fetchall()


def query_json_and(conn, tag1, tag2):
    """Find rows where tags contain BOTH tags."""
    cur = conn.execute(
        """SELECT id FROM items_json
           WHERE EXISTS (
               SELECT 1 FROM json_each(items_json.tags)
               WHERE json_each.value = ?
           )
           AND EXISTS (
               SELECT 1 FROM json_each(items_json.tags)
               WHERE json_each.value = ?
           )""",
        (tag1, tag2),
    )
    return cur.fetchall()


def query_json_or(conn, tags):
    """Find rows where tags contain ANY of the given tags."""
    placeholders = ",".join(["?"] * len(tags))
    cur = conn.execute(
        f"""SELECT DISTINCT items_json.id FROM items_json, json_each(items_json.tags)
            WHERE json_each.value IN ({placeholders})""",
        tags,
    )
    return cur.fetchall()


# ============================================================
# Strategy 1b: JSON Arrays with index
# ============================================================
def setup_json_indexed(conn, rows):
    conn.execute("DROP TABLE IF EXISTS items_json_idx")
    conn.execute("""
        CREATE TABLE items_json_idx (
            id INTEGER PRIMARY KEY,
            name TEXT,
            tags JSON
        )
    """)
    conn.executemany(
        "INSERT INTO items_json_idx (id, name, tags) VALUES (?, ?, ?)",
        [(r[0], r[1], json.dumps(r[2])) for r in rows],
    )
    # Create a generated column + index approach won't work directly,
    # so we create a separate lookup table derived from JSON
    conn.execute("DROP TABLE IF EXISTS items_json_idx_lookup")
    conn.execute("""
        CREATE TABLE items_json_idx_lookup (
            item_id INTEGER,
            tag TEXT,
            PRIMARY KEY (tag, item_id)
        ) WITHOUT ROWID
    """)
    conn.execute("""
        INSERT INTO items_json_idx_lookup (item_id, tag)
        SELECT items_json_idx.id, json_each.value
        FROM items_json_idx, json_each(items_json_idx.tags)
    """)
    conn.commit()


def query_json_indexed_single(conn, tag):
    cur = conn.execute(
        "SELECT item_id FROM items_json_idx_lookup WHERE tag = ?", (tag,)
    )
    return cur.fetchall()


def query_json_indexed_and(conn, tag1, tag2):
    cur = conn.execute(
        """SELECT a.item_id FROM items_json_idx_lookup a
           JOIN items_json_idx_lookup b ON a.item_id = b.item_id
           WHERE a.tag = ? AND b.tag = ?""",
        (tag1, tag2),
    )
    return cur.fetchall()


def query_json_indexed_or(conn, tags):
    placeholders = ",".join(["?"] * len(tags))
    cur = conn.execute(
        f"SELECT DISTINCT item_id FROM items_json_idx_lookup WHERE tag IN ({placeholders})",
        tags,
    )
    return cur.fetchall()


# ============================================================
# Strategy 2: M2M (Many-to-Many) Tables
# ============================================================
def setup_m2m(conn, rows):
    conn.execute("DROP TABLE IF EXISTS items_m2m")
    conn.execute("DROP TABLE IF EXISTS tags")
    conn.execute("DROP TABLE IF EXISTS item_tags")

    conn.execute("""
        CREATE TABLE items_m2m (
            id INTEGER PRIMARY KEY,
            name TEXT
        )
    """)
    conn.execute("""
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE
        )
    """)
    conn.execute("""
        CREATE TABLE item_tags (
            item_id INTEGER REFERENCES items_m2m(id),
            tag_id INTEGER REFERENCES tags(id),
            PRIMARY KEY (tag_id, item_id)
        ) WITHOUT ROWID
    """)

    # Insert tags
    tag_map = {}
    for i, tag in enumerate(TAGS):
        conn.execute("INSERT INTO tags (id, name) VALUES (?, ?)", (i, tag))
        tag_map[tag] = i

    # Insert items
    conn.executemany(
        "INSERT INTO items_m2m (id, name) VALUES (?, ?)",
        [(r[0], r[1]) for r in rows],
    )

    # Insert item_tags
    item_tags = []
    for r in rows:
        for tag in r[2]:
            item_tags.append((r[0], tag_map[tag]))
    conn.executemany(
        "INSERT INTO item_tags (item_id, tag_id) VALUES (?, ?)", item_tags
    )

    # Create index on item_id too for reverse lookups
    conn.execute("CREATE INDEX idx_item_tags_item ON item_tags(item_id)")
    conn.commit()


def query_m2m_single(conn, tag):
    cur = conn.execute(
        """SELECT it.item_id FROM item_tags it
           JOIN tags t ON t.id = it.tag_id
           WHERE t.name = ?""",
        (tag,),
    )
    return cur.fetchall()


def query_m2m_and(conn, tag1, tag2):
    cur = conn.execute(
        """SELECT it1.item_id FROM item_tags it1
           JOIN tags t1 ON t1.id = it1.tag_id
           JOIN item_tags it2 ON it1.item_id = it2.item_id
           JOIN tags t2 ON t2.id = it2.tag_id
           WHERE t1.name = ? AND t2.name = ?""",
        (tag1, tag2),
    )
    return cur.fetchall()


def query_m2m_or(conn, tags):
    placeholders = ",".join(["?"] * len(tags))
    cur = conn.execute(
        f"""SELECT DISTINCT it.item_id FROM item_tags it
            JOIN tags t ON t.id = it.tag_id
            WHERE t.name IN ({placeholders})""",
        tags,
    )
    return cur.fetchall()


# ============================================================
# Strategy 3: FTS5 (Full-Text Search)
# ============================================================
def setup_fts(conn, rows):
    conn.execute("DROP TABLE IF EXISTS items_fts")
    conn.execute("DROP TABLE IF EXISTS items_fts_content")

    conn.execute("""
        CREATE TABLE items_fts_content (
            id INTEGER PRIMARY KEY,
            name TEXT,
            tags_text TEXT
        )
    """)
    conn.executemany(
        "INSERT INTO items_fts_content (id, name, tags_text) VALUES (?, ?, ?)",
        [(r[0], r[1], " ".join(r[2])) for r in rows],
    )

    conn.execute("""
        CREATE VIRTUAL TABLE items_fts USING fts5(
            tags_text,
            content=items_fts_content,
            content_rowid=id,
            tokenize='unicode61 remove_diacritics 0'
        )
    """)
    conn.execute("""
        INSERT INTO items_fts(rowid, tags_text)
        SELECT id, tags_text FROM items_fts_content
    """)
    conn.commit()


def query_fts_single(conn, tag):
    cur = conn.execute(
        """SELECT rowid FROM items_fts WHERE tags_text MATCH ?""",
        (f'"{tag}"',),
    )
    return cur.fetchall()


def query_fts_and(conn, tag1, tag2):
    cur = conn.execute(
        """SELECT rowid FROM items_fts WHERE tags_text MATCH ?""",
        (f'"{tag1}" AND "{tag2}"',),
    )
    return cur.fetchall()


def query_fts_or(conn, tags):
    query = " OR ".join(f'"{t}"' for t in tags)
    cur = conn.execute(
        """SELECT rowid FROM items_fts WHERE tags_text MATCH ?""",
        (query,),
    )
    return cur.fetchall()


# ============================================================
# Strategy 4: LIKE queries (delimited string)
# ============================================================
def setup_like(conn, rows):
    conn.execute("DROP TABLE IF EXISTS items_like")
    conn.execute("""
        CREATE TABLE items_like (
            id INTEGER PRIMARY KEY,
            name TEXT,
            tags_str TEXT
        )
    """)
    conn.executemany(
        "INSERT INTO items_like (id, name, tags_str) VALUES (?, ?, ?)",
        [(r[0], r[1], "," + ",".join(r[2]) + ",") for r in rows],
    )
    conn.commit()


def query_like_single(conn, tag):
    cur = conn.execute(
        "SELECT id FROM items_like WHERE tags_str LIKE ?",
        (f"%,{tag},%",),
    )
    return cur.fetchall()


def query_like_and(conn, tag1, tag2):
    cur = conn.execute(
        "SELECT id FROM items_like WHERE tags_str LIKE ? AND tags_str LIKE ?",
        (f"%,{tag1},%", f"%,{tag2},%"),
    )
    return cur.fetchall()


def query_like_or(conn, tags):
    conditions = " OR ".join(["tags_str LIKE ?"] * len(tags))
    params = [f"%,{t},%" for t in tags]
    cur = conn.execute(
        f"SELECT DISTINCT id FROM items_like WHERE {conditions}",
        params,
    )
    return cur.fetchall()


# ============================================================
# Main benchmark runner
# ============================================================
def sizeof_table(conn, table_name):
    """Estimate storage used by a table in pages."""
    cur = conn.execute(f"SELECT COUNT(*) FROM {table_name}")
    row_count = cur.fetchone()[0]
    # Use dbstat if available, otherwise estimate
    try:
        cur = conn.execute(
            "SELECT SUM(pgsize) FROM dbstat WHERE name = ?", (table_name,)
        )
        size = cur.fetchone()[0]
        return size
    except Exception:
        return None


def get_db_size():
    """Get total database file size."""
    return os.path.getsize(DB_PATH)


def run_benchmark():
    if os.path.exists(DB_PATH):
        os.remove(DB_PATH)

    conn = sqlite3.connect(DB_PATH)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA cache_size=-64000")  # 64MB cache

    print("Generating data...")
    rows = generate_rows()
    single_tags, tag_pairs, tag_sets = pick_query_tags(rows)

    # Compute tag frequency for context
    tag_freq = {}
    for _, _, tags in rows:
        for t in tags:
            tag_freq[t] = tag_freq.get(t, 0) + 1
    avg_freq = statistics.mean(tag_freq.values())
    print(f"Average tag frequency: {avg_freq:.0f} / {NUM_ROWS} rows ({avg_freq/NUM_ROWS*100:.1f}%)")
    avg_tags_per_row = statistics.mean(len(r[2]) for r in rows)
    print(f"Average tags per row: {avg_tags_per_row:.1f}")

    strategies = {
        "JSON (no index)": {
            "setup": lambda: setup_json(conn, rows),
            "single": lambda tag: query_json_single(conn, tag),
            "and": lambda t1, t2: query_json_and(conn, t1, t2),
            "or": lambda tags: query_json_or(conn, tags),
            "tables": ["items_json"],
        },
        "JSON + lookup table": {
            "setup": lambda: setup_json_indexed(conn, rows),
            "single": lambda tag: query_json_indexed_single(conn, tag),
            "and": lambda t1, t2: query_json_indexed_and(conn, t1, t2),
            "or": lambda tags: query_json_indexed_or(conn, tags),
            "tables": ["items_json_idx", "items_json_idx_lookup"],
        },
        "M2M tables": {
            "setup": lambda: setup_m2m(conn, rows),
            "single": lambda tag: query_m2m_single(conn, tag),
            "and": lambda t1, t2: query_m2m_and(conn, t1, t2),
            "or": lambda tags: query_m2m_or(conn, tags),
            "tables": ["items_m2m", "tags", "item_tags"],
        },
        "FTS5": {
            "setup": lambda: setup_fts(conn, rows),
            "single": lambda tag: query_fts_single(conn, tag),
            "and": lambda t1, t2: query_fts_and(conn, t1, t2),
            "or": lambda tags: query_fts_or(conn, tags),
            "tables": ["items_fts_content", "items_fts"],
        },
        "LIKE": {
            "setup": lambda: setup_like(conn, rows),
            "single": lambda tag: query_like_single(conn, tag),
            "and": lambda t1, t2: query_like_and(conn, t1, t2),
            "or": lambda tags: query_like_or(conn, tags),
            "tables": ["items_like"],
        },
    }

    results = {}

    for name, strategy in strategies.items():
        print(f"\n{'='*60}")
        print(f"Setting up: {name}")
        setup_start = time.perf_counter()
        strategy["setup"]()
        setup_time = (time.perf_counter() - setup_start) * 1000
        print(f"  Setup time: {setup_time:.1f} ms")

        # Measure storage
        total_size = 0
        for table in strategy["tables"]:
            sz = sizeof_table(conn, table)
            if sz:
                total_size += sz
        if total_size:
            print(f"  Storage: {total_size / 1024:.1f} KB")

        # Benchmark single tag queries
        print(f"  Benchmarking single tag lookup...")
        single_times = []
        single_counts = []
        for tag in single_tags:
            t = timeit(lambda tag=tag: strategy["single"](tag))
            single_times.append(t["median_ms"])
            single_counts.append(t["result_count"])

        # Benchmark AND queries
        print(f"  Benchmarking AND (two tags)...")
        and_times = []
        and_counts = []
        for t1, t2 in tag_pairs:
            t = timeit(lambda t1=t1, t2=t2: strategy["and"](t1, t2))
            and_times.append(t["median_ms"])
            and_counts.append(t["result_count"])

        # Benchmark OR queries
        print(f"  Benchmarking OR (2-5 tags)...")
        or_times = []
        or_counts = []
        for tags in tag_sets:
            t = timeit(lambda tags=tags: strategy["or"](tags))
            or_times.append(t["median_ms"])
            or_counts.append(t["result_count"])

        results[name] = {
            "setup_ms": setup_time,
            "storage_kb": total_size / 1024 if total_size else None,
            "single_tag": {
                "median_ms": statistics.median(single_times),
                "mean_ms": statistics.mean(single_times),
                "min_ms": min(single_times),
                "avg_results": statistics.mean(single_counts),
            },
            "and_tags": {
                "median_ms": statistics.median(and_times),
                "mean_ms": statistics.mean(and_times),
                "min_ms": min(and_times),
                "avg_results": statistics.mean(and_counts),
            },
            "or_tags": {
                "median_ms": statistics.median(or_times),
                "mean_ms": statistics.mean(or_times),
                "min_ms": min(or_times),
                "avg_results": statistics.mean(or_counts),
            },
        }

    conn.close()

    # Print summary table
    print("\n\n")
    print("=" * 90)
    print("BENCHMARK RESULTS SUMMARY")
    print("=" * 90)
    print(f"{'Strategy':<22} {'Setup(ms)':>10} {'Storage':>10} {'Single':>10} {'AND':>10} {'OR':>10}")
    print(f"{'':22} {'':>10} {'(KB)':>10} {'(med ms)':>10} {'(med ms)':>10} {'(med ms)':>10}")
    print("-" * 90)
    for name, r in results.items():
        storage = f"{r['storage_kb']:.0f}" if r['storage_kb'] else "N/A"
        print(
            f"{name:<22} {r['setup_ms']:>10.1f} {storage:>10} "
            f"{r['single_tag']['median_ms']:>10.3f} "
            f"{r['and_tags']['median_ms']:>10.3f} "
            f"{r['or_tags']['median_ms']:>10.3f}"
        )

    print("\n")
    print("Average result counts (to verify correctness):")
    print(f"{'Strategy':<22} {'Single':>10} {'AND':>10} {'OR':>10}")
    print("-" * 55)
    for name, r in results.items():
        print(
            f"{name:<22} {r['single_tag']['avg_results']:>10.0f} "
            f"{r['and_tags']['avg_results']:>10.0f} "
            f"{r['or_tags']['avg_results']:>10.0f}"
        )

    # Save results as JSON for later analysis
    with open("/home/user/research/sqlite-tags-benchmark/results.json", "w") as f:
        json.dump(results, f, indent=2)

    print("\nResults saved to results.json")
    return results


if __name__ == "__main__":
    run_benchmark()
