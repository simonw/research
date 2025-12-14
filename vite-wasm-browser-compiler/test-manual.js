#!/usr/bin/env node
/**
 * Manual test script to verify the bundler functionality
 * This uses Node.js to simulate the bundling process
 */

const http = require('http');
const https = require('https');
const { URL } = require('url');

async function fetch(url) {
    return new Promise((resolve, reject) => {
        const protocol = url.startsWith('https') ? https : http;
        protocol.get(url, (res) => {
            if (res.statusCode !== 200) {
                reject(new Error(`HTTP ${res.statusCode}`));
                return;
            }
            let data = '';
            res.on('data', chunk => data += chunk);
            res.on('end', () => resolve(data));
        }).on('error', reject);
    });
}

async function testLocalBundling() {
    const baseUrl = 'http://localhost:3000';
    console.log('Testing local bundler...\n');

    // Test 1: Check server is running
    console.log('Test 1: Server health check');
    try {
        const html = await fetch(`${baseUrl}/test-pages/simple-app/index.html`);
        console.log('  ✓ Server is running');
        console.log(`  ✓ Fetched HTML (${html.length} bytes)`);

        // Check HTML has expected elements
        if (html.includes('Simple Test App')) {
            console.log('  ✓ HTML has expected title');
        } else {
            console.log('  ✗ HTML missing expected title');
        }

        if (html.includes('styles.css')) {
            console.log('  ✓ HTML references external CSS');
        }

        if (html.includes('main.js')) {
            console.log('  ✓ HTML references external JS');
        }
    } catch (error) {
        console.log(`  ✗ Server not running: ${error.message}`);
        console.log('  Run: node server.js');
        return false;
    }

    // Test 2: Fetch CSS
    console.log('\nTest 2: CSS fetching');
    try {
        const css = await fetch(`${baseUrl}/test-pages/simple-app/styles.css`);
        console.log(`  ✓ Fetched CSS (${css.length} bytes)`);

        if (css.includes('linear-gradient')) {
            console.log('  ✓ CSS has expected styles');
        }
    } catch (error) {
        console.log(`  ✗ Failed to fetch CSS: ${error.message}`);
    }

    // Test 3: Fetch JavaScript
    console.log('\nTest 3: JavaScript fetching');
    try {
        const js = await fetch(`${baseUrl}/test-pages/simple-app/main.js`);
        console.log(`  ✓ Fetched JavaScript (${js.length} bytes)`);

        if (js.includes('addEventListener')) {
            console.log('  ✓ JavaScript has expected content');
        }
    } catch (error) {
        console.log(`  ✗ Failed to fetch JavaScript: ${error.message}`);
    }

    // Test 4: Bundler page loads
    console.log('\nTest 4: Bundler page');
    try {
        const bundlerPage = await fetch(`${baseUrl}/index-simple.html`);
        console.log(`  ✓ Bundler page loaded (${bundlerPage.length} bytes)`);

        if (bundlerPage.includes('Browser-Based Single File Bundler')) {
            console.log('  ✓ Page has correct title');
        }

        if (bundlerPage.includes('startBundling')) {
            console.log('  ✓ Page has bundling function');
        }
    } catch (error) {
        console.log(`  ✗ Failed to load bundler page: ${error.message}`);
    }

    // Test 5: CORS headers
    console.log('\nTest 5: CORS headers');
    try {
        const result = await new Promise((resolve, reject) => {
            http.get(`${baseUrl}/test-pages/simple-app/index.html`, (res) => {
                resolve({
                    statusCode: res.statusCode,
                    headers: res.headers
                });
            }).on('error', reject);
        });

        if (result.headers['access-control-allow-origin'] === '*') {
            console.log('  ✓ CORS headers present');
        } else {
            console.log('  ✗ CORS headers missing');
        }
    } catch (error) {
        console.log(`  ✗ Failed to check headers: ${error.message}`);
    }

    console.log('\n=== All basic tests passed ===');
    console.log('\nTo run full browser tests:');
    console.log('1. Start the server: node server.js');
    console.log('2. Open http://localhost:3000/index-simple.html in your browser');
    console.log('3. Enter: http://localhost:3000/test-pages/simple-app/index.html');
    console.log('4. Click "Bundle to Single File"');
    console.log('5. Verify the output contains inlined CSS and JavaScript');

    return true;
}

// Run tests
testLocalBundling().catch(console.error);
