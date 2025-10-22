import { loadPyodide } from 'pyodide';

/**
 * Baseline test v2: Simpler approach using pyodide.loadPackagesFromImports
 */

console.log('========================================');
console.log('Baseline Test v2: Pure Python Markdown');
console.log('========================================\n');

async function main() {
  try {
    console.log('Loading Pyodide...');
    const pyodide = await loadPyodide({
      indexURL: "https://cdn.jsdelivr.net/pyodide/v0.26.4/full/"
    });
    console.log('✓ Pyodide loaded\n');

    // Test 1: Simple inline Python without external packages
    console.log('--- Test 1: Basic Python execution ---');
    const basicResult = await pyodide.runPythonAsync(`
import sys
f"Python {sys.version} in WebAssembly!"
`);
    console.log(`✓ Result: ${basicResult}\n`);

    // Test 2: Try to use pyodide.loadPackagesFromImports
    console.log('--- Test 2: Install mistune via loadPackagesFromImports ---');
    try {
      await pyodide.loadPackagesFromImports(`import mistune`);
      const result = await pyodide.runPythonAsync(`
import mistune
markdown_text = "# Hello\\n\\nThis is **bold** and *italic*"
html = mistune.html(markdown_text)
html
`);
      console.log('✓ mistune works!');
      console.log(`Result: ${result.substring(0, 100)}...\n`);
    } catch (error) {
      console.log(`✗ mistune failed: ${error.message}\n`);
    }

    // Test 3: Try markdown library
    console.log('--- Test 3: Install markdown via loadPackagesFromImports ---');
    try {
      await pyodide.loadPackagesFromImports(`import markdown`);
      const result = await pyodide.runPythonAsync(`
import markdown
markdown_text = "# Hello\\n\\nThis is **bold** and *italic*"
html = markdown.markdown(markdown_text)
html
`);
      console.log('✓ markdown works!');
      console.log(`Result: ${result.substring(0, 100)}...\n`);
    } catch (error) {
      console.log(`✗ markdown failed: ${error.message}\n`);
    }

    // Test 4: Try to install via pyodide.pyimport and micropip
    console.log('--- Test 4: Try micropip directly in Python ---');
    try {
      const result = await pyodide.runPythonAsync(`
# First try to load micropip from pyodide's built-in packages
try:
    import micropip as _mp
    _mp
except ImportError:
    # If not available, try to get it from pyodide.loadPackage
    from js import pyodide as _pyodide
    import asyncio
    await _pyodide.loadPackage('micropip')
    import micropip as _mp
    _mp

"micropip is available!"
`);
      console.log(`✓ ${result}\n`);
    } catch (error) {
      console.log(`✗ micropip setup failed: ${error.message}\n`);
    }

    console.log('========================================');
    console.log('Baseline Test v2 Complete');
    console.log('========================================');

  } catch (error) {
    console.error('\n❌ Error:', error.message);
    console.error('Full error:', error);
    process.exit(1);
  }
}

main();
