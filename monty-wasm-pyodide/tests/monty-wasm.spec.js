const { test, expect } = require('@playwright/test');

test.describe('Monty standalone WASM', () => {
  test('WASM module loads and runs basic code', async ({ page }) => {
    const errors = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') errors.push(msg.text());
    });
    page.on('pageerror', (err) => errors.push(err.message));

    await page.goto('http://localhost:8765/demo.html');

    // Wait for status to contain something (loaded or error)
    await page.waitForFunction(
      () => {
        const el = document.getElementById('status');
        return el && (el.className.includes('success') || el.className.includes('error'));
      },
      { timeout: 60000 }
    );

    const statusClass = await page.$eval('#status', (el) => el.className);
    const statusText = await page.textContent('#status');

    if (statusClass.includes('error')) {
      console.log('WASM load failed:', statusText);
      console.log('Console errors:', errors);
    }

    expect(statusClass).toContain('success');
    expect(statusText).toContain('loaded');
  });

  test('basic arithmetic via UI', async ({ page }) => {
    await page.goto('http://localhost:8765/demo.html');
    await page.waitForFunction(
      () => document.getElementById('status')?.className?.includes('success'),
      { timeout: 60000 }
    );

    await page.fill('#code', '1 + 2 * 3');
    await page.click('#run-btn');
    await page.waitForSelector('#output-section.visible', { timeout: 10000 });
    const output = await page.textContent('#output');
    expect(output).toContain('7');
  });

  test('Monty API: constructor, run, withInputs', async ({ page }) => {
    await page.goto('http://localhost:8765/demo.html');
    await page.waitForFunction(
      () => document.getElementById('status')?.className?.includes('success'),
      { timeout: 60000 }
    );

    // Test basic run
    const result1 = await page.evaluate(async () => {
      const { Monty } = await import('./monty_wasm.js');
      return new Monty('1 + 2').run();
    });
    expect(JSON.parse(result1)).toBe(3);

    // Test withInputs
    const result2 = await page.evaluate(async () => {
      const { Monty } = await import('./monty_wasm.js');
      const m = Monty.withInputs('x + y', '["x", "y"]');
      return m.runWithInputs('{"x": 10, "y": 20}');
    });
    expect(JSON.parse(result2)).toBe(30);

    // Test string result
    const result3 = await page.evaluate(async () => {
      const { Monty } = await import('./monty_wasm.js');
      return new Monty('"hello " + "world"').run();
    });
    expect(JSON.parse(result3)).toBe('hello world');

    // Test list result
    const result4 = await page.evaluate(async () => {
      const { Monty } = await import('./monty_wasm.js');
      return new Monty('[1, 2, 3]').run();
    });
    expect(JSON.parse(result4)).toEqual([1, 2, 3]);
  });

  test('print capture works', async ({ page }) => {
    await page.goto('http://localhost:8765/demo.html');
    await page.waitForFunction(
      () => document.getElementById('status')?.className?.includes('success'),
      { timeout: 60000 }
    );

    const result = await page.evaluate(async () => {
      const { Monty } = await import('./monty_wasm.js');
      return new Monty('print("captured output")').runCapturePrint();
    });
    expect(result).toContain('captured output');
  });

  test('error handling - division by zero throws', async ({ page }) => {
    await page.goto('http://localhost:8765/demo.html');
    await page.waitForFunction(
      () => document.getElementById('status')?.className?.includes('success'),
      { timeout: 60000 }
    );

    const errorMsg = await page.evaluate(async () => {
      const { Monty } = await import('./monty_wasm.js');
      try {
        new Monty('1 / 0').run();
        return null;
      } catch (e) {
        return e.toString();
      }
    });
    expect(errorMsg).not.toBeNull();
    expect(errorMsg.toLowerCase()).toMatch(/zero|division/);
  });

  test('multiline for-loop code', async ({ page }) => {
    await page.goto('http://localhost:8765/demo.html');
    await page.waitForFunction(
      () => document.getElementById('status')?.className?.includes('success'),
      { timeout: 60000 }
    );

    const result = await page.evaluate(async () => {
      const { Monty } = await import('./monty_wasm.js');
      const code = 'x = 0\nfor i in range(10):\n    x = x + i\nx';
      return new Monty(code).run();
    });
    expect(JSON.parse(result)).toBe(45);
  });

  test('example buttons populate code', async ({ page }) => {
    await page.goto('http://localhost:8765/demo.html');
    await page.waitForFunction(
      () => document.getElementById('status')?.className?.includes('success'),
      { timeout: 60000 }
    );

    await page.click('[data-example="fibonacci"]');
    const code = await page.$eval('#code', (el) => el.value);
    expect(code.toLowerCase()).toContain('fibonacci');
  });
});
