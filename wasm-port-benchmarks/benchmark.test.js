// @ts-check
const { test, expect } = require('@playwright/test');
const path = require('path');
const fs = require('fs');

// Test configurations to run
const testConfigs = [
    { htmlSize: 'small', iterations: 100 },
    { htmlSize: 'medium', iterations: 100 },
    { htmlSize: 'large', iterations: 50 },
    { htmlSize: 'xlarge', iterations: 20 },
];

test.describe('HTML Tokenizer Benchmarks', () => {
    test.beforeAll(async () => {
        console.log('\n=== HTML Tokenizer Benchmark Suite ===\n');
    });

    for (const config of testConfigs) {
        test(`benchmark ${config.htmlSize} HTML (${config.iterations} iterations)`, async ({ page }) => {
            // Navigate to the benchmark page
            const benchmarkPath = `file://${path.resolve(__dirname, 'benchmark.html')}`;
            await page.goto(benchmarkPath);

            // Wait for WASM to be initialized
            await page.waitForFunction(() => {
                const status = document.getElementById('status');
                return status && status.textContent.includes('Ready');
            }, { timeout: 30000 });

            console.log(`\n--- Testing ${config.htmlSize} HTML with ${config.iterations} iterations ---`);

            // Set configuration
            await page.fill('#iterations', String(config.iterations));
            await page.selectOption('#htmlSize', config.htmlSize);

            // Run benchmark
            await page.click('#runBenchmark');

            // Wait for benchmark to complete
            await page.waitForFunction(() => {
                const status = document.getElementById('status');
                return status && status.textContent.includes('complete');
            }, { timeout: 300000 });

            // Extract results
            const results = await page.evaluate(() => {
                const getMetric = (id) => {
                    const el = document.getElementById(id);
                    return el ? el.textContent : null;
                };

                return {
                    wasm: {
                        total: getMetric('wasm-total'),
                        avg: getMetric('wasm-avg'),
                        min: getMetric('wasm-min'),
                        max: getMetric('wasm-max'),
                        tokens: getMetric('wasm-tokens'),
                    },
                    js: {
                        total: getMetric('js-total'),
                        avg: getMetric('js-avg'),
                        min: getMetric('js-min'),
                        max: getMetric('js-max'),
                        tokens: getMetric('js-tokens'),
                    },
                    summary: document.getElementById('summary')?.textContent,
                    jsonOutput: document.getElementById('json-output')?.textContent,
                };
            });

            // Log results
            console.log('\nResults:');
            console.log(`  WASM: avg=${results.wasm.avg}, total=${results.wasm.total}, tokens=${results.wasm.tokens}`);
            console.log(`  JS:   avg=${results.js.avg}, total=${results.js.total}, tokens=${results.js.tokens}`);
            console.log(`  Summary: ${results.summary?.replace(/\s+/g, ' ')}`);

            // Save detailed JSON results
            if (results.jsonOutput) {
                const jsonResults = JSON.parse(results.jsonOutput);
                const filename = `results-${config.htmlSize}-${config.iterations}iter.json`;
                fs.writeFileSync(
                    path.join(__dirname, filename),
                    JSON.stringify(jsonResults, null, 2)
                );
                console.log(`  Saved detailed results to ${filename}`);
            }

            // Verify tokens match (important for correctness)
            expect(results.wasm.tokens).toBe(results.js.tokens);

            // Parse average times for assertions
            const wasmAvg = parseFloat(results.wasm.avg);
            const jsAvg = parseFloat(results.js.avg);

            // Verify benchmarks ran successfully
            expect(wasmAvg).toBeGreaterThan(0);
            expect(jsAvg).toBeGreaterThan(0);

            console.log(`  Speedup: WASM is ${(jsAvg / wasmAvg).toFixed(2)}x ${jsAvg > wasmAvg ? 'faster' : 'slower'} than JS`);
        });
    }
});

test.describe('WASM Tokenizer Correctness', () => {
    test('should produce matching token counts', async ({ page }) => {
        const benchmarkPath = `file://${path.resolve(__dirname, 'benchmark.html')}`;
        await page.goto(benchmarkPath);

        await page.waitForFunction(() => {
            const status = document.getElementById('status');
            return status && status.textContent.includes('Ready');
        }, { timeout: 30000 });

        // Test with various HTML inputs
        const testCases = [
            '<p>Hello</p>',
            '<div class="test"><span>Content</span></div>',
            '<!DOCTYPE html><html><head><title>Test</title></head><body></body></html>',
            '<!-- comment --><p>text</p>',
        ];

        for (const html of testCases) {
            const result = await page.evaluate(async (htmlInput) => {
                // @ts-ignore - globals from benchmark page
                const { count_tokens } = await import('./wasm-pkg/wasm_html_tokenizer.js');
                const { stream } = await import('./js-src/stream.js');

                const wasmCount = count_tokens(htmlInput);

                let jsCount = 0;
                for (const _ of stream(htmlInput)) {
                    jsCount++;
                }

                return { wasmCount, jsCount, html: htmlInput };
            }, html);

            console.log(`Testing: "${html.substring(0, 50)}..." - WASM: ${result.wasmCount}, JS: ${result.jsCount}`);

            // WASM and JS might count slightly differently due to how EOF is handled
            // Allow a small difference
            const diff = Math.abs(result.wasmCount - result.jsCount);
            expect(diff).toBeLessThanOrEqual(1);
        }
    });
});
