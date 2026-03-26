#!/usr/bin/env python3
"""Run benchmark comparing fzstd (JS) vs WASM zstd decompressors."""
import subprocess
import json
import time

def rodney_js(code, timeout=120):
    result = subprocess.run(
        ["uvx", "rodney", "js", code],
        capture_output=True, text=True, timeout=timeout
    )
    return result.stdout.strip()

QUERIES = {
    "point": "SELECT * FROM users WHERE id = 42",
    "filter": "SELECT name, city, score FROM users WHERE city = 'Tokyo' AND age > 30 LIMIT 20",
    "agg": "SELECT city, COUNT(*) as cnt, AVG(score) as avg_score FROM users GROUP BY city ORDER BY cnt DESC",
    "join": "SELECT u.name, p.name as product, o.quantity, o.total FROM orders o JOIN users u ON o.user_id = u.id JOIN products p ON o.product_id = p.id WHERE u.city = 'London' LIMIT 20",
    "complex": "SELECT u.city, COUNT(DISTINCT o.id) as order_count, SUM(o.total) as revenue FROM orders o JOIN users u ON o.user_id = u.id GROUP BY u.city ORDER BY revenue DESC",
    "count": "SELECT (SELECT COUNT(*) FROM users) as users, (SELECT COUNT(*) FROM products) as products, (SELECT COUNT(*) FROM orders) as orders",
}

def open_db(format_name, size, mode, decompressor):
    escaped_js = f"""
(async () => {{
  try {{
    if (db) {{ db.close(); db = null; }}
    document.getElementById('format-select').value = '{format_name}';
    document.getElementById('size-select').value = '{size}';
    document.getElementById('mode-select').value = '{mode}';
    document.getElementById('decompressor-select').value = '{decompressor}';
    await openDatabase();
    return JSON.stringify(dbStats);
  }} catch(e) {{ return JSON.stringify({{error: e.message}}); }}
}})()
"""
    result = rodney_js(escaped_js, timeout=300)
    try:
        return json.loads(result)
    except:
        return {"error": result}

def run_query(sql, trials=3):
    escaped = sql.replace("'", "\\'").replace('"', '\\"')
    js = f"""
(function() {{
  try {{
    const timings = [];
    let rowCount = 0;
    for (let i = 0; i < {trials}; i++) {{
      const t0 = performance.now();
      const res = db.exec("{escaped}");
      timings.push(performance.now() - t0);
      if (res.length > 0) rowCount = res[0].values.length;
    }}
    timings.sort((a, b) => a - b);
    return JSON.stringify({{medianMs: timings[Math.floor(timings.length/2)], rowCount}});
  }} catch(e) {{ return JSON.stringify({{error: e.message}}); }}
}})()
"""
    result = rodney_js(js)
    try:
        return json.loads(result)
    except:
        return {"error": result}

def main():
    configs = [
        # (format, sizes, modes, decompressors)
        ("sqlite", ["tiny", "small", "medium", "large"], ["full"], ["fzstd"]),
        ("turbolite", ["tiny", "small", "medium", "large"], ["full", "range"], ["fzstd", "wasm"]),
    ]

    all_results = []

    for fmt, sizes, modes, decompressors in configs:
        for size in sizes:
            for mode in modes:
                for decomp in decompressors:
                    label = f"{fmt}/{size}/{mode}"
                    if fmt == "turbolite":
                        label += f"/{decomp}"
                    print(f"\n--- {label} ---")

                    stats = open_db(fmt, size, mode, decomp)
                    if "error" in stats:
                        print(f"  OPEN ERROR: {stats['error']}")
                        all_results.append({"format": fmt, "size": size, "mode": mode, "decompressor": decomp, "error": stats["error"]})
                        continue

                    print(f"  Opened: {stats.get('openTimeMs', '?')}ms, "
                          f"{stats.get('requests', '?')} reqs, "
                          f"{stats.get('bytesTransferred', 0) / 1024:.0f} KB")

                    for qname, sql in QUERIES.items():
                        qr = run_query(sql)
                        if "error" in qr:
                            print(f"  {qname}: ERROR - {qr['error']}")
                        else:
                            print(f"  {qname}: {qr['medianMs']:.2f}ms ({qr['rowCount']} rows)")
                            all_results.append({
                                "format": fmt, "size": size, "mode": mode,
                                "decompressor": decomp,
                                "queryName": qname,
                                "queryTimeMs": round(qr["medianMs"], 3),
                                "rowCount": qr["rowCount"],
                                **stats
                            })

    # Save results
    with open("benchmark_results_wasm.json", "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\n=== Saved {len(all_results)} results to benchmark_results_wasm.json ===")

    # Print summary table
    print("\n=== OPEN TIME COMPARISON (turbolite, full download) ===")
    print(f"{'Size':<10} {'fzstd (JS)':<15} {'WASM zstd':<15} {'Speedup':<10} {'Plain SQLite':<15}")
    for size in ["tiny", "small", "medium", "large"]:
        fzstd_time = next((r["openTimeMs"] for r in all_results if r.get("format") == "turbolite" and r.get("size") == size and r.get("mode") == "full" and r.get("decompressor") == "fzstd" and "queryName" in r), "?")
        wasm_time = next((r["openTimeMs"] for r in all_results if r.get("format") == "turbolite" and r.get("size") == size and r.get("mode") == "full" and r.get("decompressor") == "wasm" and "queryName" in r), "?")
        sqlite_time = next((r["openTimeMs"] for r in all_results if r.get("format") == "sqlite" and r.get("size") == size and r.get("mode") == "full" and "queryName" in r), "?")
        speedup = f"{fzstd_time / wasm_time:.1f}x" if isinstance(fzstd_time, (int, float)) and isinstance(wasm_time, (int, float)) and wasm_time > 0 else "?"
        print(f"{size:<10} {str(fzstd_time)+'ms':<15} {str(wasm_time)+'ms':<15} {speedup:<10} {str(sqlite_time)+'ms':<15}")

if __name__ == "__main__":
    main()
