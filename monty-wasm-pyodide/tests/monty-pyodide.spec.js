const { test, expect } = require('@playwright/test');

const WHEEL_NAME = 'pydantic_monty-0.0.3-cp313-cp313-emscripten_4_0_9_wasm32.whl';

// Helper: load Pyodide and install the monty wheel in the browser
async function setupPyodide(page) {
  // Navigate to a page served by our local HTTP server
  await page.goto('http://localhost:8765/test-page.html');

  // Wait for Pyodide to be available (loaded via CDN script tag)
  await page.waitForFunction(() => typeof loadPyodide === 'function', {
    timeout: 60000,
  });

  // Initialize Pyodide and install the wheel
  const setupResult = await page.evaluate(async (wheelName) => {
    const pyodide = await loadPyodide({
      indexURL: '/pyodide/',
    });
    await pyodide.loadPackage('micropip');
    const micropip = pyodide.pyimport('micropip');
    const wheelUrl = `http://localhost:8765/${wheelName}`;
    await micropip.install(wheelUrl);
    // Store pyodide globally so subsequent evaluate calls can use it
    window._pyodide = pyodide;
    return pyodide.runPython('import sys; sys.version');
  }, WHEEL_NAME);

  return setupResult;
}

test.describe('Monty WASM in Pyodide', () => {
  test.beforeEach(async ({ page }) => {
    await setupPyodide(page);
  });

  test('import pydantic_monty succeeds', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const pyodide = window._pyodide;
      pyodide.runPython('import pydantic_monty');
      return pyodide.runPython('pydantic_monty.__version__');
    });
    expect(result).toBeTruthy();
    expect(typeof result).toBe('string');
  });

  test('basic arithmetic: 1 + 2 * 3 = 7', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const pyodide = window._pyodide;
      return pyodide.runPython(`
import pydantic_monty
m = pydantic_monty.Monty('1 + 2 * 3')
m.run()
      `);
    });
    expect(result).toBe(7);
  });

  test('input variables: x + y', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const pyodide = window._pyodide;
      return pyodide.runPython(`
import pydantic_monty
m = pydantic_monty.Monty('x + y', inputs=['x', 'y'])
m.run(inputs={"x": 10, "y": 20})
      `);
    });
    expect(result).toBe(30);
  });

  test('string concatenation', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const pyodide = window._pyodide;
      return pyodide.runPython(`
import pydantic_monty
m = pydantic_monty.Monty('"hello " + "world"')
m.run()
      `);
    });
    expect(result).toBe('hello world');
  });

  test('list creation', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const pyodide = window._pyodide;
      return pyodide.runPython(`
import pydantic_monty
m = pydantic_monty.Monty('[1, 2, 3, 4, 5]')
str(m.run())
      `);
    });
    expect(result).toBe('[1, 2, 3, 4, 5]');
  });

  test('exception handling: division by zero', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const pyodide = window._pyodide;
      return pyodide.runPython(`
import pydantic_monty
m = pydantic_monty.Monty('1 / 0')
result = "no error"
try:
    m.run()
except pydantic_monty.MontyRuntimeError as e:
    result = str(e)
result
      `);
    });
    expect(result).toBeTruthy();
    expect(result.toLowerCase()).toContain('zero');
  });

  test('reuse Monty with different inputs', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const pyodide = window._pyodide;
      return pyodide.runPython(`
import pydantic_monty
m = pydantic_monty.Monty('x * y', inputs=['x', 'y'])
r1 = m.run(inputs={"x": 3, "y": 4})
r2 = m.run(inputs={"x": 5, "y": 6})
[r1, r2]
      `).toJs();
    });
    expect(result[0]).toBe(12);
    expect(result[1]).toBe(30);
  });

  test('multiline code execution', async ({ page }) => {
    const result = await page.evaluate(async () => {
      const pyodide = window._pyodide;
      return pyodide.runPython(`
import pydantic_monty
code = '''
x = 0
for i in range(10):
    x = x + i
x
'''
m = pydantic_monty.Monty(code)
m.run()
      `);
    });
    expect(result).toBe(45);
  });
});
