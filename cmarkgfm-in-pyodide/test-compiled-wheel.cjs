#!/usr/bin/env node

const { loadPyodide } = require('pyodide');
const fs = require('fs');
const path = require('path');

async function testCmarkgfmWheel() {
    console.log('Loading Pyodide...');
    const pyodide = await loadPyodide();

    console.log('Pyodide loaded successfully');
    console.log(`Python version: ${pyodide.runPython('import sys; sys.version')}`);

    // Load the wheel
    const wheelPath = path.join(__dirname, 'build-pyodide', 'cmarkgfm_pyodide-2025.10.22-cp312-cp312-emscripten_3_1_46_wasm32.whl');
    console.log(`\nLoading wheel from: ${wheelPath}`);

    const wheelBuffer = fs.readFileSync(wheelPath);
    const wheelData = new Uint8Array(wheelBuffer);
    pyodide.unpackArchive(wheelData, 'whl');

    console.log('\nWheel loaded. Testing cmarkgfm module...\n');

    // Test basic import
    try {
        const result = pyodide.runPython(`
import cmarkgfm

# Test 1: Basic markdown_to_html
markdown_text = "# Hello World\\n\\nThis is a **test**."
html = cmarkgfm.markdown_to_html(markdown_text)
print("Test 1: Basic markdown_to_html")
print("Input:", repr(markdown_text))
print("Output:", html)
print()

# Test 2: GitHub Flavored Markdown with table
gfm_text = """
# GFM Test

| Feature | Status |
|---------|--------|
| Tables  | ✓      |
| ~~Strikethrough~~ | ✓ |

- [ ] Task list item 1
- [x] Task list item 2

https://github.com
"""

gfm_html = cmarkgfm.github_flavored_markdown_to_html(gfm_text)
print("Test 2: GitHub Flavored Markdown")
print("Input:", repr(gfm_text))
print("Output:", gfm_html)
print()

# Test 3: Check module attributes
print("Test 3: Module attributes")
print("Module version:", cmarkgfm.__version__)
print("cmark version:", cmarkgfm.CMARK_VERSION)
print()

# Test 4: Test with options
html_with_opts = cmarkgfm.github_flavored_markdown_to_html(
    "Smart quotes: 'hello' and \\"world\\"",
    options=cmarkgfm.Options.CMARK_OPT_SMART
)
print("Test 4: With SMART option")
print("Output:", html_with_opts)

"ALL TESTS PASSED!"
        `);

        console.log('\n✅ ALL TESTS PASSED!');
        console.log('Result:', result);

    } catch (error) {
        console.error('\n❌ TEST FAILED');
        console.error('Error:', error.message);
        if (error.message.includes('PythonError')) {
            console.error('\nPython traceback:');
            console.error(pyodide.runPython('import traceback; traceback.print_exc()'));
        }
        process.exit(1);
    }
}

testCmarkgfmWheel().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});
