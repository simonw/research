import { loadPyodide } from 'pyodide';

/**
 * Simple test to verify Pyodide works and explore markdown options
 */

console.log('========================================');
console.log('Simple Pyodide Test');
console.log('========================================\n');

async function main() {
  try {
    console.log('Loading Pyodide...');
    const pyodide = await loadPyodide();
    console.log('✓ Pyodide loaded\n');

    // Test 1: Basic Python
    console.log('--- Test 1: Basic Python ---');
    const result1 = await pyodide.runPythonAsync(`
import sys
import platform

info = f"""
Python: {sys.version}
Platform: {platform.platform()}
Machine: {platform.machine()}
"""
info
`);
    console.log(result1);

    // Test 2: Check what packages are available
    console.log('--- Test 2: Available packages ---');
    const result2 = await pyodide.runPythonAsync(`
import sys
# List some standard library modules
stdlib_modules = [
    'collections', 'itertools', 'functools', 're',
    'html', 'html.parser', 'urllib', 'base64'
]
available = []
for module in stdlib_modules:
    try:
        __import__(module)
        available.append(module)
    except ImportError:
        pass
"Available stdlib modules: " + ", ".join(available)
`);
    console.log(result2 + '\n');

    // Test 3: Can we implement a simple markdown parser in pure Python?
    console.log('--- Test 3: Simple inline markdown parser ---');
    const result3 = await pyodide.runPythonAsync(`
import re
import html

def simple_markdown_to_html(text):
    """Very basic markdown to HTML converter using only stdlib"""
    # Escape HTML first
    text = html.escape(text)

    # Headers
    text = re.sub(r'^### (.+)$', r'<h3>\\1</h3>', text, flags=re.MULTILINE)
    text = re.sub(r'^## (.+)$', r'<h2>\\1</h2>', text, flags=re.MULTILINE)
    text = re.sub(r'^# (.+)$', r'<h1>\\1</h1>', text, flags=re.MULTILINE)

    # Bold
    text = re.sub(r'\\*\\*(.+?)\\*\\*', r'<strong>\\1</strong>', text)

    # Italic
    text = re.sub(r'\\*(.+?)\\*', r'<em>\\1</em>', text)

    # Strikethrough (GFM)
    text = re.sub(r'~~(.+?)~~', r'<del>\\1</del>', text)

    # Line breaks
    text = text.replace('\\n\\n', '</p><p>')

    return f'<p>{text}</p>'

# Test it
markdown_text = """# Hello

This is **bold** and *italic*

~~strikethrough~~"""

html_output = simple_markdown_to_html(markdown_text)
html_output
`);
    console.log('Result:', result3, '\n');

    console.log('========================================');
    console.log('✓ Simple test complete!');
    console.log('========================================');

  } catch (error) {
    console.error('\n❌ Error:', error.message);
    console.error('Full error:', error);
    process.exit(1);
  }
}

main();
