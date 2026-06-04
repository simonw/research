"""
BeautifulSoup TreeBuilder using JustHTML as the parsing library.

This module provides integration between BeautifulSoup 4 and JustHTML,
allowing bs4 to use justhtml's HTML5-compliant parser with full HTML5
tree construction (including implicit element insertion).
"""

import sys
# Only need justhtml from source, bs4 is installed via pip
sys.path.insert(0, '/tmp/justhtml/src')

from bs4.builder import (
    HTML,
    HTML_5,
    HTMLTreeBuilder,
    PERMISSIVE,
)
from bs4.element import (
    Comment,
    Doctype,
)

from justhtml import JustHTML
from justhtml.node import ElementNode, SimpleDomNode, TemplateNode, TextNode


__all__ = ['JustHTMLTreeBuilder']


class JustHTMLTreeBuilder(HTMLTreeBuilder):
    """Use JustHTML to build a tree.

    JustHTML is a pure Python HTML5 parser that implements the WHATWG
    HTML5 specification exactly. It passes all 9k+ tests in the official
    html5lib-tests suite.

    This integration uses justhtml's full tree builder, which means:
    - Implicit elements (html, head, body) are created when missing
    - Foster parenting is handled correctly
    - All HTML5 error recovery rules are applied
    """

    NAME = "justhtml"
    ALTERNATE_NAMES = ["just-html", "justhtml"]

    features = [NAME, HTML_5, HTML, PERMISSIVE]

    def __init__(self, **kwargs):
        super().__init__(**kwargs)

    def prepare_markup(self, markup, user_specified_encoding=None,
                       document_declared_encoding=None, exclude_encodings=None):
        """Prepare markup for parsing.

        JustHTML handles encoding detection internally when given bytes.
        """
        if isinstance(markup, bytes):
            yield (markup, None, None, False)
        else:
            yield (markup, None, None, False)

    def feed(self, markup):
        """Parse markup using JustHTML's full tree builder."""
        # Parse with justhtml to get a complete HTML5-compliant tree
        doc = JustHTML(markup)

        # Walk the tree and emit events to BeautifulSoup
        self._walk_tree(doc.root)

    def _walk_tree(self, node):
        """Recursively walk the justhtml tree and emit bs4 events."""
        name = node.name

        # Handle document root
        if name == "#document":
            if node.children:
                for child in node.children:
                    self._walk_tree(child)
            return

        # Handle document fragment
        if name == "#document-fragment":
            if node.children:
                for child in node.children:
                    self._walk_tree(child)
            return

        # Handle DOCTYPE
        if name == "!doctype":
            doctype_data = node.data
            if doctype_data:
                doctype = Doctype.for_name_and_ids(
                    doctype_data.name or "",
                    doctype_data.public_id,
                    doctype_data.system_id
                )
            else:
                doctype = Doctype.for_name_and_ids("html", None, None)
            self.soup.object_was_parsed(doctype)
            return

        # Handle comments
        if name == "#comment":
            self.soup.endData()
            self.soup.handle_data(node.data or "")
            self.soup.endData(Comment)
            return

        # Handle text nodes
        if name == "#text" or isinstance(node, TextNode):
            data = node.data if hasattr(node, 'data') else ""
            if data:
                self.soup.handle_data(data)
            return

        # Handle elements
        if isinstance(node, (ElementNode, TemplateNode)) or (hasattr(node, 'attrs') and node.attrs is not None):
            # Start tag
            attrs = dict(node.attrs) if node.attrs else {}
            self.soup.handle_starttag(name, None, None, attrs)

            # Handle template content
            if isinstance(node, TemplateNode) and node.template_content:
                self._walk_tree(node.template_content)

            # Process children
            if node.children:
                for child in node.children:
                    self._walk_tree(child)

            # End tag
            self.soup.handle_endtag(name)
            return

        # Fallback for any other node type with children
        if hasattr(node, 'children') and node.children:
            for child in node.children:
                self._walk_tree(child)

    def test_fragment_to_document(self, fragment):
        """Wrap an HTML fragment to make it look like a document."""
        return '<html><head></head><body>%s</body></html>' % fragment


# Register the builder
from bs4.builder import builder_registry
builder_registry.register(JustHTMLTreeBuilder)


if __name__ == "__main__":
    # Quick test
    from bs4 import BeautifulSoup

    html = """
    <!DOCTYPE html>
    <html>
    <head><title>Test Page</title></head>
    <body>
        <div class="container">
            <h1>Hello World!</h1>
            <p>This is a <strong>test</strong> paragraph.</p>
            <!-- This is a comment -->
            <ul>
                <li>Item 1</li>
                <li>Item 2</li>
            </ul>
        </div>
    </body>
    </html>
    """

    print("Testing JustHTML TreeBuilder with BeautifulSoup")
    print("=" * 50)

    soup = BeautifulSoup(html, "justhtml")

    print("\nTitle:", soup.title.string)
    print("H1:", soup.h1.string)
    print("Strong:", soup.strong.string)
    print("\nParagraph classes:", soup.find("div", class_="container"))
    print("\nList items:")
    for li in soup.find_all("li"):
        print(f"  - {li.string}")

    print("\nComment:")
    from bs4 import Comment as BSComment
    comments = soup.find_all(string=lambda text: isinstance(text, BSComment))
    for c in comments:
        print(f"  {c}")

    # Test implicit element insertion
    print("\n" + "=" * 50)
    print("Testing implicit element insertion:")
    html2 = "<title>Test</title><p>Content</p>"
    soup2 = BeautifulSoup(html2, "justhtml")
    print(f"  html tag present: {soup2.html is not None}")
    print(f"  head tag present: {soup2.head is not None}")
    print(f"  body tag present: {soup2.body is not None}")

    print("\n" + "=" * 50)
    print("Test completed successfully!")
