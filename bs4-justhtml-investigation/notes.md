# Investigation: Running bs4 with justhtml as parsing library

## Setup
- Cloned Beautiful Soup from https://github.com/snarfed/beautifulsoup (mirror of official Launchpad repo)
  - Note: The mirror had Python 2 code, so used pip-installed beautifulsoup4 instead
- Cloned justhtml from https://github.com/EmilStenstrom/justhtml

## Investigation Steps

### Step 1: Understanding bs4's tree builder architecture

bs4 uses a "TreeBuilder" plugin system for parsers. Key files:
- `bs4/builder/__init__.py` - Base TreeBuilder class and registry
- `bs4/builder/_html5lib.py` - html5lib integration
- `bs4/builder/_htmlparser.py` - Python's HTMLParser integration
- `bs4/builder/_lxml.py` - lxml integration

The TreeBuilder interface requires:
- `NAME` - Parser name string
- `features` - List of features (HTML, HTML_5, PERMISSIVE, etc.)
- `feed(markup)` - Main parsing method
- `prepare_markup()` - Encoding handling

Parsers call BeautifulSoup methods:
- `soup.handle_starttag(name, namespace, nsprefix, attrs)`
- `soup.handle_endtag(name)`
- `soup.handle_data(data)`
- `soup.endData()` and `soup.endData(Comment)` for special nodes

### Step 2: Understanding justhtml's API

justhtml provides several interfaces:
- `JustHTML(html)` - Full tree parser, returns SimpleDomNode tree
- `stream(html)` - SAX-like event stream (tokenizer-level)
- `TreeBuilder` - Internal tree construction

The stream API yields tuples:
- `("start", (name, attrs))`
- `("end", name)`
- `("text", data)`
- `("comment", data)`
- `("doctype", (name, public_id, system_id))`

### Step 3: Integration Approach

Two possible approaches:

**Approach A: Stream API (simpler)**
Use justhtml's `stream()` function to get SAX-like events and feed to bs4.
- Pros: Simple, clean, similar to how HTMLParser integration works
- Cons: Stream is tokenizer-level, doesn't include HTML5 tree construction

**Approach B: Tree Builder Integration (implemented)**
Use justhtml's full tree construction via `JustHTML()`, walk the resulting
tree and emit events to BeautifulSoup.
- Pros: Full HTML5 compliance including implicit elements
- Works correctly!

### Step 4: Implementation

Implemented Approach B (full tree builder). Created `bs4_justhtml.py` with:
- `JustHTMLTreeBuilder` class extending `HTMLTreeBuilder`
- Uses `JustHTML()` to parse with full HTML5 tree construction
- Walks the tree via `_walk_tree()` to emit bs4 events
- Registered as "justhtml" parser

### Step 5: Testing

Created comprehensive test suite (`test_integration.py`):
- ✓ Basic parsing
- ✓ Attributes handling
- ✓ Nested elements
- ✓ Comments
- ✓ DOCTYPE
- ✓ Malformed HTML recovery
- ✓ Self-closing tags
- ✓ Script/style content
- ✓ Unicode
- ✓ Find methods and CSS selectors
- ✓ Tables
- ✓ HTML5 implicit element insertion
- ✓ Bytes input

## Conclusion

**Yes, it is possible to run bs4 with justhtml as the parsing library.**

The full tree builder integration provides complete HTML5 compliance including:
- Implicit element insertion (html, head, body auto-created)
- Proper error recovery for malformed HTML
- All HTML5 tree construction algorithms
