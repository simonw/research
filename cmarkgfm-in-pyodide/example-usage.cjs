#!/usr/bin/env node

/**
 * Example: Using cmarkgfm in Pyodide
 *
 * This demonstrates how to use the compiled cmarkgfm wheel in a Pyodide environment.
 * Run with: node example-usage.cjs
 */

const { loadPyodide } = require('pyodide');
const fs = require('fs');
const path = require('path');

async function main() {
    console.log('🚀 Loading Pyodide...');
    const pyodide = await loadPyodide();
    console.log('✅ Pyodide loaded\n');

    // Load the cmarkgfm wheel
    const wheelPath = path.join(__dirname, 'dist', 'cmarkgfm_pyodide-2025.10.22-cp312-cp312-emscripten_3_1_46_wasm32.whl');
    console.log(`📦 Loading cmarkgfm from: ${path.basename(wheelPath)}`);

    const wheelBuffer = fs.readFileSync(wheelPath);
    const wheelData = new Uint8Array(wheelBuffer);
    pyodide.unpackArchive(wheelData, 'whl');

    console.log('✅ cmarkgfm loaded\n');

    // Example 1: Basic Markdown
    console.log('=== Example 1: Basic Markdown ===');
    const example1 = pyodide.runPython(`
import cmarkgfm

markdown = """
# Welcome to cmarkgfm in Pyodide!

This is **bold** and this is *italic*.

## Lists work too:

- Item 1
- Item 2
- Item 3
"""

html = cmarkgfm.markdown_to_html(markdown)
html
    `);
    console.log(example1);

    // Example 2: GitHub Flavored Markdown with Tables
    console.log('\n=== Example 2: GFM Tables ===');
    const example2 = pyodide.runPython(`
markdown_table = """
| Language | WebAssembly Support |
|----------|---------------------|
| Python   | ✅ Via Pyodide      |
| Rust     | ✅ Native           |
| C/C++    | ✅ Via Emscripten   |
"""

html = cmarkgfm.github_flavored_markdown_to_html(markdown_table)
html
    `);
    console.log(example2);

    // Example 3: Task Lists
    console.log('\n=== Example 3: Task Lists ===');
    const example3 = pyodide.runPython(`
tasks = """
## Project Checklist

- [x] Research Pyodide compatibility
- [x] Rewrite without CFFI
- [x] Compile to WebAssembly
- [x] Test in Node.js
- [ ] Test in browser
"""

html = cmarkgfm.github_flavored_markdown_to_html(tasks)
html
    `);
    console.log(example3);

    // Example 4: Strikethrough and Autolinks
    console.log('\n=== Example 4: GFM Features ===');
    const example4 = pyodide.runPython(`
gfm = """
~~This is crossed out~~

Auto-linked URL: https://github.com/github/cmark-gfm
"""

html = cmarkgfm.github_flavored_markdown_to_html(gfm)
html
    `);
    console.log(example4);

    // Example 5: Smart Typography
    console.log('\n=== Example 5: Smart Typography ===');
    const example5 = pyodide.runPython(`
smart = """
"Smart quotes" and 'apostrophes'...
Three dots... and -- dashes.
"""

html = cmarkgfm.markdown_to_html(smart, options=cmarkgfm.Options.CMARK_OPT_SMART)
html
    `);
    console.log(example5);

    // Show module info
    console.log('\n=== Module Information ===');
    const info = pyodide.runPython(`
import cmarkgfm
f"""
Module version: {cmarkgfm.__version__}
cmark-gfm version: {cmarkgfm.CMARK_VERSION}

Available functions:
- markdown_to_html()
- github_flavored_markdown_to_html()
- markdown_to_html_with_extensions()

Available in Options class:
- CMARK_OPT_DEFAULT
- CMARK_OPT_SMART
- CMARK_OPT_UNSAFE
- CMARK_OPT_VALIDATE_UTF8
- ... and more
"""
    `);
    console.log(info);

    console.log('\n✨ All examples completed successfully!');
}

main().catch(err => {
    console.error('Error:', err);
    process.exit(1);
});
