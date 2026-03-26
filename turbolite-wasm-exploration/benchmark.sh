#!/bin/bash
# Benchmark script using rodney browser automation
set -e

# Ensure rodney is available
RODNEY="uvx rodney"

echo "=== Turbolite WASM Benchmark ==="
echo "Date: $(date)"
echo ""

# Navigate to the page
$RODNEY reload 2>/dev/null || true
sleep 1
$RODNEY waitload 2>/dev/null

# Run the benchmark via the in-page runner
echo "Starting in-browser benchmark (both formats, all sizes)..."
echo "This will take a while for large databases..."

$RODNEY js "
(async () => {
  try {
    window.benchmarkComplete = false;
    window.benchmarkResults = [];

    const formats = ['sqlite', 'turbolite'];
    const sizes = ['tiny', 'small', 'medium', 'large'];
    const modes = ['full', 'range'];
    const queries = {
      point: \"SELECT * FROM users WHERE id = 42\",
      filter: \"SELECT name, city, score FROM users WHERE city = 'Tokyo' AND age > 30 LIMIT 20\",
      agg: \"SELECT city, COUNT(*) as cnt, AVG(score) as avg_score FROM users GROUP BY city ORDER BY cnt DESC\",
      join: \"SELECT u.name, p.name as product, o.quantity, o.total FROM orders o JOIN users u ON o.user_id = u.id JOIN products p ON o.product_id = p.id WHERE u.city = 'London' LIMIT 20\",
      complex: \"SELECT u.city, COUNT(DISTINCT o.id) as order_count, SUM(o.total) as revenue, AVG(o.total) as avg_order FROM orders o JOIN users u ON o.user_id = u.id GROUP BY u.city ORDER BY revenue DESC\",
      count: \"SELECT (SELECT COUNT(*) FROM users) as users, (SELECT COUNT(*) FROM products) as products, (SELECT COUNT(*) FROM orders) as orders\",
    };

    const results = [];

    for (const format of formats) {
      for (const size of sizes) {
        for (const mode of modes) {
          // Open database
          document.getElementById('format-select').value = format;
          document.getElementById('size-select').value = size;
          document.getElementById('mode-select').value = mode;

          try {
            if (db) { db.close(); db = null; }
            await openDatabase();
            const openStats = { ...dbStats };

            for (const [queryName, sql] of Object.entries(queries)) {
              // Run each query 3 times, take median
              const timings = [];
              let rowCount = 0;
              for (let trial = 0; trial < 3; trial++) {
                const t0 = performance.now();
                const res = db.exec(sql);
                timings.push(performance.now() - t0);
                if (res.length > 0) rowCount = res[0].values.length;
              }
              timings.sort((a, b) => a - b);
              const medianMs = timings[1]; // median of 3

              results.push({
                format, size, mode, queryName,
                queryTimeMs: parseFloat(medianMs.toFixed(3)),
                rowCount,
                openTimeMs: openStats.openTimeMs,
                fetchTimeMs: openStats.fetchTimeMs,
                parseTimeMs: openStats.parseTimeMs,
                requests: openStats.requests,
                bytesTransferred: openStats.bytesTransferred,
              });
            }

            if (db) { db.close(); db = null; }
          } catch (e) {
            results.push({
              format, size, mode, queryName: 'OPEN_ERROR',
              error: e.message,
            });
          }
        }
      }
    }

    window.benchmarkResults = results;
    window.benchmarkComplete = true;
    return 'done:' + results.length;
  } catch(e) { return 'ERROR: ' + e.message + '\\n' + e.stack; }
})()
" 2>&1

echo ""
echo "Extracting results..."

# Extract results as JSON
$RODNEY js "JSON.stringify(window.benchmarkResults)" 2>&1 > /home/user/research/turbolite-wasm-exploration/benchmark_results.json

echo "Results saved to benchmark_results.json"
echo "Done!"
