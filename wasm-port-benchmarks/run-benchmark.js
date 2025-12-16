#!/usr/bin/env node
/**
 * Benchmark runner script for HTML Tokenizer (WASM vs JS)
 *
 * This script starts an HTTP server, runs benchmarks using Playwright,
 * and outputs the results.
 */

const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = 8765;
const BASE_DIR = __dirname;

// Simple static file server
function createServer() {
    return http.createServer((req, res) => {
        let filePath = path.join(BASE_DIR, req.url === '/' ? 'benchmark.html' : req.url);

        // Security: prevent directory traversal
        if (!filePath.startsWith(BASE_DIR)) {
            res.writeHead(403);
            res.end('Forbidden');
            return;
        }

        const ext = path.extname(filePath).toLowerCase();
        const contentTypes = {
            '.html': 'text/html',
            '.js': 'application/javascript',
            '.wasm': 'application/wasm',
            '.json': 'application/json',
            '.css': 'text/css',
        };

        fs.readFile(filePath, (err, data) => {
            if (err) {
                console.log(`[Server] 404: ${req.url} (${filePath})`);
                res.writeHead(404);
                res.end('Not found: ' + filePath);
                return;
            }
            console.log(`[Server] 200: ${req.url}`);
            res.writeHead(200, {
                'Content-Type': contentTypes[ext] || 'application/octet-stream',
                'Cross-Origin-Opener-Policy': 'same-origin',
                'Cross-Origin-Embedder-Policy': 'require-corp',
            });
            res.end(data);
        });
    });
}

async function runBenchmarks() {
    console.log('='.repeat(60));
    console.log('HTML Tokenizer Benchmark: WASM vs JavaScript');
    console.log('='.repeat(60));
    console.log();

    // Start server
    const server = createServer();
    await new Promise((resolve) => server.listen(PORT, resolve));
    console.log(`Server started on http://localhost:${PORT}`);

    // Launch browser
    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext();
    const page = await context.newPage();

    // Capture console logs and errors
    page.on('console', (msg) => {
        console.log(`[Browser ${msg.type()}] ${msg.text()}`);
    });

    page.on('pageerror', (err) => {
        console.error(`[Browser error] ${err.message}`);
    });

    try {
        // Navigate to benchmark page
        console.log('\nLoading benchmark page...');
        await page.goto(`http://localhost:${PORT}/benchmark.html`);

        // Wait for page load
        await page.waitForLoadState('networkidle');
        console.log('Page loaded');

        // Check status
        const initialStatus = await page.locator('#status').textContent();
        console.log(`Initial status: ${initialStatus}`);

        // Wait for WASM to initialize with longer timeout and debugging
        console.log('Waiting for WASM initialization...');
        try {
            await page.waitForFunction(() => {
                const status = document.getElementById('status');
                return status && status.textContent.includes('Ready');
            }, { timeout: 60000 });
        } catch (e) {
            const currentStatus = await page.locator('#status').textContent();
            console.log(`Status at timeout: ${currentStatus}`);
            const logContent = await page.locator('#log').textContent();
            console.log(`Log content: ${logContent}`);
            throw e;
        }

        const allResults = [];

        // Run benchmarks for different sizes
        const configs = [
            { htmlSize: 'small', iterations: 200, name: 'Small (~1KB)' },
            { htmlSize: 'medium', iterations: 200, name: 'Medium (~10KB)' },
            { htmlSize: 'large', iterations: 100, name: 'Large (~100KB)' },
            { htmlSize: 'xlarge', iterations: 50, name: 'XLarge (~500KB)' },
        ];

        for (const config of configs) {
            console.log(`\n${'='.repeat(60)}`);
            console.log(`Running: ${config.name} HTML, ${config.iterations} iterations`);
            console.log('='.repeat(60));

            // Set configuration
            await page.fill('#iterations', String(config.iterations));
            await page.selectOption('#htmlSize', config.htmlSize);

            // Run benchmark
            await page.click('#runBenchmark');

            // Wait for completion
            await page.waitForFunction(() => {
                const status = document.getElementById('status');
                return status && status.textContent.includes('complete');
            }, { timeout: 300000 });

            // Extract results
            const results = await page.evaluate(() => {
                const jsonOutput = document.getElementById('json-output');
                return jsonOutput ? JSON.parse(jsonOutput.textContent) : null;
            });

            if (results) {
                allResults.push({
                    config: config.name,
                    ...results,
                });

                console.log(`\nResults for ${config.name}:`);
                console.log(`  HTML size: ${results.config.htmlLength.toLocaleString()} characters`);
                console.log(`  Iterations: ${results.config.iterations}`);
                console.log();
                console.log(`  WASM Tokenizer:`);
                console.log(`    Total time: ${results.wasm.total.toFixed(2)}ms`);
                console.log(`    Average:    ${results.wasm.avg.toFixed(3)}ms`);
                console.log(`    Min:        ${results.wasm.min.toFixed(3)}ms`);
                console.log(`    Max:        ${results.wasm.max.toFixed(3)}ms`);
                console.log(`    Tokens:     ${results.wasm.tokens}`);
                console.log();
                console.log(`  JavaScript Tokenizer:`);
                console.log(`    Total time: ${results.js.total.toFixed(2)}ms`);
                console.log(`    Average:    ${results.js.avg.toFixed(3)}ms`);
                console.log(`    Min:        ${results.js.min.toFixed(3)}ms`);
                console.log(`    Max:        ${results.js.max.toFixed(3)}ms`);
                console.log(`    Tokens:     ${results.js.tokens}`);
                console.log();
                console.log(`  Winner: ${results.winner}`);
                console.log(`  Speedup: ${results.speedup.toFixed(2)}x`);
            }
        }

        // Save all results
        const outputPath = path.join(BASE_DIR, 'benchmark-results.json');
        fs.writeFileSync(outputPath, JSON.stringify(allResults, null, 2));
        console.log(`\nFull results saved to: ${outputPath}`);

        // Print summary table
        console.log('\n' + '='.repeat(60));
        console.log('SUMMARY TABLE');
        console.log('='.repeat(60));
        console.log();
        console.log('| Size    | HTML Chars | WASM Avg (ms) | JS Avg (ms) | Winner | Speedup |');
        console.log('|---------|------------|---------------|-------------|--------|---------|');

        for (const result of allResults) {
            const size = result.config.padEnd(9);
            const chars = result.config.htmlLength.toLocaleString().padStart(10);
            const wasmAvg = result.wasm.avg.toFixed(3).padStart(13);
            const jsAvg = result.js.avg.toFixed(3).padStart(11);
            const winner = result.winner.padEnd(6);
            const speedup = `${result.speedup.toFixed(2)}x`.padStart(7);
            console.log(`| ${size} | ${chars} | ${wasmAvg} | ${jsAvg} | ${winner} | ${speedup} |`);
        }

        console.log();

    } finally {
        await browser.close();
        server.close();
        console.log('\nBenchmark complete!');
    }
}

runBenchmarks().catch((err) => {
    console.error('Benchmark failed:', err);
    process.exit(1);
});
