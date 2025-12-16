#!/usr/bin/env node
/**
 * Node.js benchmark for HTML Tokenizer WASM vs JS
 * This script runs benchmarks directly in Node.js without a browser.
 */

import { readFileSync, writeFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Load and initialize WASM
async function loadWasm() {
    const wasmPath = join(__dirname, 'wasm-pkg/wasm_html_tokenizer_bg.wasm');
    const wasmBuffer = readFileSync(wasmPath);

    // We need to simulate what the browser does
    const wasmModule = await WebAssembly.compile(wasmBuffer);

    // Import the JS bindings
    const { default: init, count_tokens, tokenize_html } = await import('./wasm-pkg/wasm_html_tokenizer.js');

    // Initialize with the module
    await init({ module_or_path: wasmModule });

    return { count_tokens, tokenize_html };
}

// Import JS tokenizer
async function loadJsTokenizer() {
    const { stream } = await import('./js-src/stream.js');
    return stream;
}

// Generate test HTML
function generateTestHTML(size) {
    const base = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Test Document</title>
    <link rel="stylesheet" href="styles.css">
</head>
<body>
    <header id="main-header" class="header header-dark">
        <nav class="navigation">
            <ul class="nav-list">
                <li class="nav-item"><a href="/" class="nav-link active">Home</a></li>
                <li class="nav-item"><a href="/about" class="nav-link">About</a></li>
            </ul>
        </nav>
    </header>
    <main role="main" data-page="home">
        <article class="content">
            <h1>Welcome to the Test Page</h1>
            <p class="intro">This is a sample paragraph with <strong>bold text</strong> and <em>italic text</em>.</p>
            <div class="widget" data-widget-type="gallery" data-config='{"autoplay": true}'>
                <img src="image1.jpg" alt="Image 1" loading="lazy" />
            </div>
            <!-- This is a comment -->
            <section id="features">
                <h2>Features</h2>
                <ul>
                    <li>Fast performance</li>
                    <li>Easy to use</li>
                </ul>
            </section>
        </article>
    </main>
    <footer>&copy; 2024 Test Company</footer>
</body>
</html>`;

    let multiplier;
    switch(size) {
        case 'small': multiplier = 1; break;
        case 'medium': multiplier = 10; break;
        case 'large': multiplier = 100; break;
        case 'xlarge': multiplier = 500; break;
        default: multiplier = 10;
    }

    let html = base;
    for (let i = 1; i < multiplier; i++) {
        html += `<section class="extra-section-${i}"><h3>Section ${i}</h3><p>Content for section ${i} with <a href="#link${i}">a link</a>.</p></section>`;
    }

    return html;
}

// Calculate stats
function calculateStats(times) {
    const sorted = [...times].sort((a, b) => a - b);
    const total = times.reduce((a, b) => a + b, 0);
    const avg = total / times.length;
    return {
        total,
        avg,
        min: sorted[0],
        max: sorted[sorted.length - 1],
        median: sorted[Math.floor(sorted.length / 2)],
        stdDev: Math.sqrt(times.reduce((acc, t) => acc + Math.pow(t - avg, 2), 0) / times.length)
    };
}

async function runBenchmarks() {
    console.log('='.repeat(60));
    console.log('HTML Tokenizer Benchmark: WASM vs JavaScript (Node.js)');
    console.log('='.repeat(60));
    console.log();

    // Load both tokenizers
    console.log('Loading WASM tokenizer...');
    const { count_tokens, tokenize_html } = await loadWasm();
    console.log('WASM tokenizer loaded.');

    console.log('Loading JS tokenizer...');
    const stream = await loadJsTokenizer();
    console.log('JS tokenizer loaded.');
    console.log();

    const configs = [
        { htmlSize: 'small', iterations: 500, name: 'Small (~1KB)' },
        { htmlSize: 'medium', iterations: 200, name: 'Medium (~10KB)' },
        { htmlSize: 'large', iterations: 100, name: 'Large (~100KB)' },
        { htmlSize: 'xlarge', iterations: 20, name: 'XLarge (~500KB)' },
    ];

    const allResults = [];

    for (const config of configs) {
        console.log('='.repeat(60));
        console.log(`Running: ${config.name} HTML, ${config.iterations} iterations`);
        console.log('='.repeat(60));

        const testHTML = generateTestHTML(config.htmlSize);
        console.log(`HTML size: ${testHTML.length.toLocaleString()} characters`);

        // Warmup
        console.log('Warming up...');
        for (let i = 0; i < 5; i++) {
            count_tokens(testHTML);
            let c = 0;
            for (const _ of stream(testHTML)) c++;
        }

        // WASM benchmark
        console.log('Running WASM benchmark...');
        const wasmTimes = [];
        let wasmTokens = 0;

        for (let i = 0; i < config.iterations; i++) {
            const start = performance.now();
            wasmTokens = count_tokens(testHTML);
            const end = performance.now();
            wasmTimes.push(end - start);
        }

        // JS benchmark
        console.log('Running JS benchmark...');
        const jsTimes = [];
        let jsTokens = 0;

        for (let i = 0; i < config.iterations; i++) {
            const start = performance.now();
            jsTokens = 0;
            for (const _ of stream(testHTML)) jsTokens++;
            const end = performance.now();
            jsTimes.push(end - start);
        }

        const wasmStats = calculateStats(wasmTimes);
        const jsStats = calculateStats(jsTimes);

        const speedup = jsStats.avg / wasmStats.avg;
        const winner = speedup > 1 ? 'WASM' : 'JavaScript';

        console.log();
        console.log(`WASM Tokenizer:`);
        console.log(`  Total time: ${wasmStats.total.toFixed(2)}ms`);
        console.log(`  Average:    ${wasmStats.avg.toFixed(3)}ms`);
        console.log(`  Min:        ${wasmStats.min.toFixed(3)}ms`);
        console.log(`  Max:        ${wasmStats.max.toFixed(3)}ms`);
        console.log(`  Tokens:     ${wasmTokens}`);
        console.log();
        console.log(`JavaScript Tokenizer:`);
        console.log(`  Total time: ${jsStats.total.toFixed(2)}ms`);
        console.log(`  Average:    ${jsStats.avg.toFixed(3)}ms`);
        console.log(`  Min:        ${jsStats.min.toFixed(3)}ms`);
        console.log(`  Max:        ${jsStats.max.toFixed(3)}ms`);
        console.log(`  Tokens:     ${jsTokens}`);
        console.log();
        console.log(`Winner: ${winner}`);
        console.log(`Speedup: ${speedup.toFixed(2)}x`);
        console.log();

        allResults.push({
            config: config.name,
            htmlLength: testHTML.length,
            iterations: config.iterations,
            wasm: { ...wasmStats, tokens: wasmTokens },
            js: { ...jsStats, tokens: jsTokens },
            speedup,
            winner
        });
    }

    // Print summary table
    console.log('='.repeat(60));
    console.log('SUMMARY TABLE');
    console.log('='.repeat(60));
    console.log();
    console.log('| Size      | HTML Chars | WASM Avg (ms) | JS Avg (ms) | Winner | Speedup |');
    console.log('|-----------|------------|---------------|-------------|--------|---------|');

    for (const result of allResults) {
        const size = result.config.padEnd(11);
        const chars = result.htmlLength.toLocaleString().padStart(10);
        const wasmAvg = result.wasm.avg.toFixed(3).padStart(13);
        const jsAvg = result.js.avg.toFixed(3).padStart(11);
        const winner = result.winner.padEnd(6);
        const speedup = `${result.speedup.toFixed(2)}x`.padStart(7);
        console.log(`| ${size} | ${chars} | ${wasmAvg} | ${jsAvg} | ${winner} | ${speedup} |`);
    }

    console.log();
    console.log('Benchmark complete!');

    // Return results for saving
    return allResults;
}

runBenchmarks()
    .then(results => {
        // Save results to JSON
        const resultsJson = JSON.stringify(results, null, 2);
        const outputPath = join(__dirname, 'benchmark-results.json');
        writeFileSync(outputPath, resultsJson);
        console.log(`\nResults saved to: ${outputPath}`);
    })
    .catch(err => {
        console.error('Benchmark failed:', err);
        process.exit(1);
    });
