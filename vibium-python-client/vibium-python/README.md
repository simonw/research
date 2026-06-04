# vibium-python

Python client for [Vibium](https://github.com/VibiumDev/vibium) browser automation.

## Installation

```bash
uv add vibium-python
```

## Requirements

- Python 3.11+
- The `clicker` binary must be available on PATH (or specify path explicitly)

## Quick Start

### Sync API

```python
from vibium_python import browser

# Launch browser
vibe = browser.launch(headless=True)

# Navigate
vibe.go("https://example.com")

# Find and interact with elements
h1 = vibe.find("h1")
print(h1.text)

button = vibe.find("button")
button.click()

input_elem = vibe.find("input")
input_elem.type("Hello World")

# Screenshot
png_data = vibe.screenshot()

# Cleanup
vibe.quit()
```

### Async API

```python
import asyncio
from vibium_python import async_browser

async def main():
    vibe = await async_browser.launch(headless=True)
    await vibe.go("https://example.com")

    h1 = await vibe.find("h1")
    print(h1.text)

    await vibe.quit()

asyncio.run(main())
```

## API Reference

### browser.launch() / async_browser.launch()

Launch a browser session.

```python
vibe = browser.launch(
    headless=False,           # Show browser window (default)
    executable_path="clicker" # Path to clicker binary
)
```

### Vibe.go(url)

Navigate to a URL.

```python
vibe.go("https://example.com")
await vibe.go("https://example.com")  # async
```

### Vibe.find(selector, timeout=None)

Find an element by CSS selector. Waits for element to exist.

```python
element = vibe.find("h1")
element = vibe.find("#my-id", timeout=5000)  # 5 second timeout
```

### Element.click(timeout=None)

Click an element. Waits for actionability (visible, stable, enabled).

```python
button = vibe.find("button")
button.click()
```

### Element.type(text, timeout=None)

Type text into an element. Waits for actionability (visible, stable, enabled, editable).

```python
input_elem = vibe.find("input")
input_elem.type("Hello World")
```

### Vibe.screenshot()

Capture screenshot as PNG bytes.

```python
png_data = vibe.screenshot()
with open("screenshot.png", "wb") as f:
    f.write(png_data)
```

### Vibe.quit()

Close browser and cleanup.

```python
vibe.quit()
```

## Development

```bash
# Install dependencies
uv sync

# Run tests
uv run pytest -v
```

## License

Apache 2.0
