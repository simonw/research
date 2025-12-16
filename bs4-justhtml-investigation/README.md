# Can BeautifulSoup Use JustHTML as a Parser?

**TL;DR: Yes, with full HTML5 compliance!** This investigation demonstrates that BeautifulSoup 4 can use [JustHTML](https://github.com/EmilStenstrom/justhtml) as its parsing backend with complete HTML5 tree construction.

## Background

- **BeautifulSoup** is a popular Python library for parsing HTML/XML with a simple API
- **JustHTML** is a pure Python HTML5 parser that implements the WHATWG spec exactly, passing all 9k+ html5lib-tests

The question: Can we combine them to get BeautifulSoup's nice API with JustHTML's HTML5 compliance?

## Answer: Yes

I created a working integration (`bs4_justhtml.py`) that lets you do:

```python
from bs4 import BeautifulSoup

soup = BeautifulSoup(html, "justhtml")
soup.find_all("p")  # Works!
soup.select(".container > div")  # CSS selectors work!
```

## How It Works

BeautifulSoup has a plugin system for parsers called "TreeBuilders". I created a `JustHTMLTreeBuilder` that:

1. Uses JustHTML's `JustHTML()` to parse with full HTML5 tree construction
2. Walks the resulting tree and emits events to BeautifulSoup
3. Registers itself as the "justhtml" parser

This approach (vs using the stream API) ensures full HTML5 compliance including implicit element insertion.

## Test Results

All features work:

| Feature | Status |
|---------|--------|
| Basic parsing | ✅ |
| Attributes | ✅ |
| Nested elements | ✅ |
| Comments | ✅ |
| DOCTYPE | ✅ |
| Malformed HTML recovery | ✅ |
| Self-closing tags | ✅ |
| Script/style content | ✅ |
| Unicode | ✅ |
| find()/find_all() | ✅ |
| CSS selectors | ✅ |
| Tables | ✅ |
| **HTML5 implicit elements** | ✅ |
| Bytes input | ✅ |

## HTML5 Implicit Element Insertion

The integration correctly handles implicit element insertion per the HTML5 spec:

```python
html = "<title>Test</title><p>Content</p>"
soup = BeautifulSoup(html, "justhtml")

soup.html  # <html> - auto-created!
soup.head  # <head> - auto-created!
soup.body  # <body> - auto-created!
```

## Files

- `bs4_justhtml.py` - The TreeBuilder integration (can be used as a library)
- `test_integration.py` - Comprehensive test suite
- `notes.md` - Investigation notes

## Usage

```python
import sys
sys.path.insert(0, '/path/to/justhtml/src')

from bs4_justhtml import JustHTMLTreeBuilder  # Registers the parser
from bs4 import BeautifulSoup

html = "<div><p>Hello <strong>world</strong>!</p></div>"
soup = BeautifulSoup(html, "justhtml")

print(soup.p.strong.string)  # "world"
```

## Conclusion

JustHTML can serve as a BeautifulSoup parser backend with **full HTML5 compliance**. The integration uses JustHTML's tree builder directly, ensuring all HTML5 tree construction algorithms are applied correctly.

For production use, consider packaging the integration properly (not using sys.path hacks).
