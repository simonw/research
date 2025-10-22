import { loadPyodide } from 'pyodide';

/**
 * Main test: Try various approaches to get cmarkgfm working in Pyodide
 */

console.log('========================================');
console.log('cmarkgfm in Pyodide Test');
console.log('========================================\n');

async function testApproach(pyodide, name, testCode) {
  console.log(`\n--- Approach: ${name} ---`);
  try {
    const result = await pyodide.runPythonAsync(testCode);
    console.log(`✓ ${name} succeeded!`);
    if (result) {
      console.log(`Result: ${result.substring(0, 200)}...`);
    }
    return { success: true, approach: name };
  } catch (error) {
    console.log(`✗ ${name} failed: ${error.message}`);
    return { success: false, approach: name, error: error.message };
  }
}

async function main() {
  try {
    console.log('Loading Pyodide...');
    const pyodide = await loadPyodide();
    console.log('✓ Pyodide loaded');

    console.log('Loading micropip...');
    await pyodide.loadPackage('micropip');
    console.log('✓ micropip loaded\n');

    const testMarkdown = '# Hello\\n\\nThis is **bold** and *italic*\\n\\n- Item 1\\n- Item 2\\n\\n~~strikethrough~~';

    const results = [];

    // Approach 1: Try direct micropip install (will likely fail - no pure Python wheel)
    results.push(await testApproach(pyodide, 'Direct micropip install', `
import micropip
await micropip.install('cmarkgfm')
import cmarkgfm
markdown_text = """${testMarkdown}"""
html = cmarkgfm.github_flavored_markdown_to_html(markdown_text)
html
`));

    // Approach 2: Try to install from PyPI with binary wheel (will fail - not built for wasm)
    results.push(await testApproach(pyodide, 'Install with any wheel', `
import micropip
# Try to force install even if not pure Python
await micropip.install('cmarkgfm', keep_going=True)
import cmarkgfm
markdown_text = """${testMarkdown}"""
html = cmarkgfm.github_flavored_markdown_to_html(markdown_text)
html
`));

    // Approach 3: Check if we can at least import the package metadata
    results.push(await testApproach(pyodide, 'Check package availability', `
import micropip
# Check what's available
packages = await micropip.list()
'cmarkgfm' in [p.name for p in packages]
`));

    console.log('\n========================================');
    console.log('cmarkgfm Test Results');
    console.log('========================================');

    const successful = results.filter(r => r.success);
    const failed = results.filter(r => !r.success);

    console.log(`\n✓ Successful: ${successful.length}/${results.length}`);
    successful.forEach(r => console.log(`  - ${r.approach}`));

    console.log(`\n✗ Failed: ${failed.length}/${results.length}`);
    failed.forEach(r => console.log(`  - ${r.approach}: ${r.error.substring(0, 100)}`));

    if (successful.length === 0) {
      console.log('\n⚠️  All approaches failed. cmarkgfm requires building for WebAssembly.');
      console.log('This is expected - cmarkgfm has C extensions that need compilation.');
    } else {
      console.log('\n✓ At least one approach succeeded!');
    }

  } catch (error) {
    console.error('\n❌ Error:', error.message);
    console.error('Full error:', error);
    process.exit(1);
  }
}

main();
