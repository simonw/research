"""
Comprehensive tests for the bs4 + justhtml integration.
"""

import sys
sys.path.insert(0, '/tmp/justhtml/src')

from bs4_justhtml import JustHTMLTreeBuilder
from bs4 import BeautifulSoup
from bs4.element import Comment, Doctype


def test_basic_parsing():
    """Test basic HTML parsing."""
    html = "<html><head><title>Test</title></head><body><p>Hello</p></body></html>"
    soup = BeautifulSoup(html, "justhtml")
    assert soup.title.string == "Test"
    assert soup.p.string == "Hello"
    print("✓ Basic parsing works")


def test_attributes():
    """Test attribute handling."""
    html = '<div id="main" class="container large" data-value="123">Content</div>'
    soup = BeautifulSoup(html, "justhtml")
    div = soup.div
    assert div['id'] == 'main'
    assert 'container' in div['class']
    assert 'large' in div['class']
    assert div['data-value'] == '123'
    print("✓ Attribute handling works")


def test_nested_elements():
    """Test nested elements."""
    html = "<div><p><span><strong>Deep</strong></span></p></div>"
    soup = BeautifulSoup(html, "justhtml")
    assert soup.div.p.span.strong.string == "Deep"
    print("✓ Nested elements work")


def test_comments():
    """Test comment handling."""
    html = "<div><!-- This is a comment -->Text</div>"
    soup = BeautifulSoup(html, "justhtml")
    comments = soup.find_all(string=lambda text: isinstance(text, Comment))
    assert len(comments) == 1
    assert "This is a comment" in comments[0]
    print("✓ Comments work")


def test_doctype():
    """Test DOCTYPE handling."""
    html = "<!DOCTYPE html><html><head></head><body></body></html>"
    soup = BeautifulSoup(html, "justhtml")
    # The doctype should be present
    items = list(soup.children)
    assert any(isinstance(item, Doctype) for item in items)
    print("✓ DOCTYPE handling works")


def test_malformed_html():
    """Test handling of malformed HTML (HTML5 error recovery)."""
    # Missing closing tags
    html = "<div><p>Paragraph 1<p>Paragraph 2</div>"
    soup = BeautifulSoup(html, "justhtml")
    ps = soup.find_all('p')
    assert len(ps) == 2
    print("✓ Malformed HTML (missing closing tags) handled")

    # Unclosed tags
    html = "<b><i>bold and italic</b> just italic</i>"
    soup = BeautifulSoup(html, "justhtml")
    assert soup.b is not None
    print("✓ Overlapping tags handled")


def test_self_closing_tags():
    """Test self-closing/void elements."""
    html = '<img src="test.jpg" alt="Test"><br><input type="text">'
    soup = BeautifulSoup(html, "justhtml")
    assert soup.img['src'] == 'test.jpg'
    assert soup.br is not None
    assert soup.input['type'] == 'text'
    print("✓ Self-closing tags work")


def test_script_and_style():
    """Test script and style content handling."""
    html = """
    <html>
    <head>
        <style>body { color: red; }</style>
        <script>var x = 1 < 2;</script>
    </head>
    <body></body>
    </html>
    """
    soup = BeautifulSoup(html, "justhtml")
    assert "color: red" in soup.style.string
    assert "var x = 1 < 2" in soup.script.string
    print("✓ Script and style content preserved")


def test_unicode():
    """Test unicode handling."""
    html = "<p>Hello 世界 🌍</p>"
    soup = BeautifulSoup(html, "justhtml")
    assert "世界" in soup.p.string
    assert "🌍" in soup.p.string
    print("✓ Unicode handling works")


def test_find_methods():
    """Test BeautifulSoup find methods work with justhtml parser."""
    html = """
    <div class="container">
        <p class="intro">First</p>
        <p class="content">Second</p>
        <span id="special">Special</span>
    </div>
    """
    soup = BeautifulSoup(html, "justhtml")

    # find
    p = soup.find('p', class_='intro')
    assert p.string == 'First'

    # find_all
    ps = soup.find_all('p')
    assert len(ps) == 2

    # CSS selector (if soupsieve is installed)
    try:
        selected = soup.select('.container p')
        assert len(selected) == 2
        print("✓ CSS selectors work")
    except NotImplementedError:
        print("  (CSS selectors not available)")

    # find by id
    span = soup.find(id='special')
    assert span.string == 'Special'

    print("✓ Find methods work")


def test_table_parsing():
    """Test table parsing."""
    html = """
    <table>
        <thead><tr><th>Header</th></tr></thead>
        <tbody>
            <tr><td>Cell 1</td></tr>
            <tr><td>Cell 2</td></tr>
        </tbody>
    </table>
    """
    soup = BeautifulSoup(html, "justhtml")
    assert soup.table is not None
    assert soup.th.string == "Header"
    tds = soup.find_all('td')
    assert len(tds) == 2
    print("✓ Table parsing works")


def test_html5_implicit_elements():
    """Test HTML5 implicit element handling.

    The full tree builder integration correctly inserts implicit elements
    (html, head, body) when they're missing from the source, following
    the HTML5 specification.
    """
    # HTML5 parser should auto-create html, head, body
    html = "<title>Test</title><p>Content</p>"
    soup = BeautifulSoup(html, "justhtml")
    assert soup.html is not None, "html element should be implicitly created"
    assert soup.head is not None, "head element should be implicitly created"
    assert soup.body is not None, "body element should be implicitly created"
    assert soup.title.string == "Test"
    assert soup.p.string == "Content"
    print("✓ HTML5 implicit element insertion works")


def test_encoding_bytes():
    """Test parsing from bytes."""
    html_bytes = b"<html><head><meta charset='utf-8'></head><body><p>Hello</p></body></html>"
    soup = BeautifulSoup(html_bytes, "justhtml")
    assert soup.p.string == "Hello"
    print("✓ Bytes input works")


def run_all_tests():
    print("Running bs4 + justhtml integration tests")
    print("=" * 50)

    test_basic_parsing()
    test_attributes()
    test_nested_elements()
    test_comments()
    test_doctype()
    test_malformed_html()
    test_self_closing_tags()
    test_script_and_style()
    test_unicode()
    test_find_methods()
    test_table_parsing()
    test_html5_implicit_elements()
    test_encoding_bytes()

    print("=" * 50)
    print("All tests passed! ✓")


if __name__ == "__main__":
    run_all_tests()
