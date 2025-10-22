import { loadPyodide } from 'pyodide';

/**
 * Baseline test: Try to install and use pure Python markdown libraries
 * This establishes whether micropip works for our setup
 */

console.log('========================================');
console.log('Baseline Test: Pure Python Markdown Libraries');
console.log('========================================\n');

async function testMarkdownLibrary(pyodide, libraryName, testCode) {
  console.log(`\n--- Testing ${libraryName} ---`);
  try {
    console.log(`Installing ${libraryName}...`);
    await pyodide.runPythonAsync(`
import micropip
await micropip.install('${libraryName}')
print(f"✓ ${libraryName} installed successfully")
`);

    console.log(`Running test code...`);
    const result = await pyodide.runPythonAsync(testCode);
    console.log(`✓ ${libraryName} works! Result length: ${result.length} chars`);
    console.log(`Sample output: ${result.substring(0, 100)}...`);
    return { success: true, library: libraryName };
  } catch (error) {
    console.log(`✗ ${libraryName} failed: ${error.message}`);
    return { success: false, library: libraryName, error: error.message };
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

    const testMarkdown = '# Hello\\n\\nThis is **bold** and *italic*\\n\\n- Item 1\\n- Item 2';

    const results = [];

    // Test mistune (pure Python, fast)
    results.push(await testMarkdownLibrary(pyodide, 'mistune', `
import mistune
markdown_text = """${testMarkdown}"""
html = mistune.html(markdown_text)
html
`));

    // Test markdown (Python-Markdown)
    results.push(await testMarkdownLibrary(pyodide, 'markdown', `
import markdown
markdown_text = """${testMarkdown}"""
html = markdown.markdown(markdown_text)
html
`));

    // Test markdown2
    results.push(await testMarkdownLibrary(pyodide, 'markdown2', `
import markdown2
markdown_text = """${testMarkdown}"""
html = markdown2.markdown(markdown_text)
html
`));

    console.log('\n========================================');
    console.log('Baseline Test Results');
    console.log('========================================');

    const successful = results.filter(r => r.success);
    const failed = results.filter(r => !r.success);

    console.log(`\n✓ Successful: ${successful.length}/${results.length}`);
    successful.forEach(r => console.log(`  - ${r.library}`));

    if (failed.length > 0) {
      console.log(`\n✗ Failed: ${failed.length}/${results.length}`);
      failed.forEach(r => console.log(`  - ${r.library}: ${r.error}`));
    }

    console.log('\n✓ Baseline test complete!');

  } catch (error) {
    console.error('\n❌ Error:', error.message);
    console.error('Full error:', error);
    process.exit(1);
  }
}

main();
