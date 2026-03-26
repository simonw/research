#!/usr/bin/env python3
"""Run benchmark using rodney, one configuration at a time."""
import subprocess
import json
import sys
import time

def rodney(cmd):
    result = subprocess.run(
        ["uvx", "rodney"] + cmd.split(),
        capture_output=True, text=True, timeout=120
    )
    return result.stdout.strip(), result.stderr.strip(), result.returncode

def rodney_js(code, timeout=60):
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

def open_db(format_name, size, mode):
    """Open a database and return open stats."""
    js = f"""
(async () => {{
  try {{
    if (db) {{ db.close(); db = null; }}
    document.getElementById('format-select').value = '{format_name}';
    document.getElementById('size-select').value = '{size}';
    document.getElementById('mode-select').value = '{mode}';
    await openDatabase();
    return JSON.stringify(dbStats);
  }} catch(e) {{ return JSON.stringify({{error: e.message}}); }}
}})()
"""
    result = rodney_js(js, timeout=120)
    try:
        return json.loads(result)
    except:
        return {"error": result}

def run_query(sql, trials=3):
    """Run a query and return median timing."""
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
    return JSON.stringify({{medianMs: timings[Math.floor(timings.length/2)], rowCount, timings}});
  }} catch(e) {{ return JSON.stringify({{error: e.message}}); }}
}})()
"""
    result = rodney_js(js)
    try:
        return json.loads(result)
    except:
        return {"error": result}

def main():
    formats = ["sqlite", "turbolite"]
    sizes = ["tiny", "small", "medium", "large"]
    modes = ["full", "range"]

    all_results = []

    for fmt in formats:
        for size in sizes:
            for mode in modes:
                print(f"\n--- {fmt} / {size} / {mode} ---")
                stats = open_db(fmt, size, mode)
                if "error" in stats:
                    print(f"  OPEN ERROR: {stats['error']}")
                    all_results.append({"format": fmt, "size": size, "mode": mode, "error": stats["error"]})
                    continue

                print(f"  Opened: {stats.get('openTimeMs', '?')}ms, "
                      f"{stats.get('requests', '?')} reqs, "
                      f"{stats.get('bytesTransferred', 0) / 1024:.0f} KB")

                for qname, sql in QUERIES.items():
                    qr = run_query(sql)
                    if "error" in qr:
                        print(f"  {qname}: ERROR - {qr['error']}")
                        all_results.append({
                            "format": fmt, "size": size, "mode": mode,
                            "queryName": qname, "error": qr["error"], **stats
                        })
                    else:
                        print(f"  {qname}: {qr['medianMs']:.2f}ms ({qr['rowCount']} rows)")
                        all_results.append({
                            "format": fmt, "size": size, "mode": mode,
                            "queryName": qname,
                            "queryTimeMs": round(qr["medianMs"], 3),
                            "rowCount": qr["rowCount"],
                            **stats
                        })

    # Save results
    with open("benchmark_results.json", "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\n=== Saved {len(all_results)} results to benchmark_results.json ===")

if __name__ == "__main__":
    main()
