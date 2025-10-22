"""
Python wrapper module for cmarkgfm-pyodide

This provides a compatible API with the original cmarkgfm package,
but uses a Python C extension instead of CFFI, making it compatible with Pyodide.
"""

from _cmarkgfm import (
    markdown_to_html as _markdown_to_html,
    github_flavored_markdown_to_html as _github_flavored_markdown_to_html,
    CMARK_OPT_DEFAULT,
    CMARK_OPT_SOURCEPOS,
    CMARK_OPT_HARDBREAKS,
    CMARK_OPT_UNSAFE,
    CMARK_OPT_NOBREAKS,
    CMARK_OPT_NORMALIZE,
    CMARK_OPT_VALIDATE_UTF8,
    CMARK_OPT_SMART,
    CMARK_OPT_GITHUB_PRE_LANG,
    CMARK_OPT_LIBERAL_HTML_TAG,
    CMARK_OPT_FOOTNOTES,
    CMARK_OPT_STRIKETHROUGH_DOUBLE_TILDE,
    CMARK_OPT_TABLE_PREFER_STYLE_ATTRIBUTES,
    CMARK_VERSION,
    __version__,
)

__all__ = [
    'markdown_to_html',
    'github_flavored_markdown_to_html',
    'markdown_to_html_with_extensions',
    'Options',
    'CMARK_VERSION',
    '__version__',
]

class Options:
    """Options for markdown rendering"""
    CMARK_OPT_DEFAULT = CMARK_OPT_DEFAULT
    CMARK_OPT_SOURCEPOS = CMARK_OPT_SOURCEPOS
    CMARK_OPT_HARDBREAKS = CMARK_OPT_HARDBREAKS
    CMARK_OPT_UNSAFE = CMARK_OPT_UNSAFE
    CMARK_OPT_NOBREAKS = CMARK_OPT_NOBREAKS
    CMARK_OPT_NORMALIZE = CMARK_OPT_NORMALIZE
    CMARK_OPT_VALIDATE_UTF8 = CMARK_OPT_VALIDATE_UTF8
    CMARK_OPT_SMART = CMARK_OPT_SMART
    CMARK_OPT_GITHUB_PRE_LANG = CMARK_OPT_GITHUB_PRE_LANG
    CMARK_OPT_LIBERAL_HTML_TAG = CMARK_OPT_LIBERAL_HTML_TAG
    CMARK_OPT_FOOTNOTES = CMARK_OPT_FOOTNOTES
    CMARK_OPT_STRIKETHROUGH_DOUBLE_TILDE = CMARK_OPT_STRIKETHROUGH_DOUBLE_TILDE
    CMARK_OPT_TABLE_PREFER_STYLE_ATTRIBUTES = CMARK_OPT_TABLE_PREFER_STYLE_ATTRIBUTES


def markdown_to_html(text, options=0):
    """Render Markdown to HTML.

    Args:
        text (str): Markdown text to render
        options (int): Rendering options (default: 0)

    Returns:
        str: HTML output
    """
    return _markdown_to_html(text, options=options)


def github_flavored_markdown_to_html(text, options=0):
    """Render GitHub Flavored Markdown to HTML.

    This automatically enables GitHub extensions: tables, strikethrough,
    autolink, tagfilter, and task lists.

    Args:
        text (str): Markdown text to render
        options (int): Additional rendering options (default: 0)

    Returns:
        str: HTML output
    """
    return _github_flavored_markdown_to_html(text, options=options)


def markdown_to_html_with_extensions(text, options=0, extensions=None):
    """Render Markdown to HTML with extensions.

    Note: In this Pyodide-compatible version, this function is equivalent to
    github_flavored_markdown_to_html() and ignores the extensions parameter.
    All GFM extensions are always enabled.

    Args:
        text (str): Markdown text to render
        options (int): Rendering options (default: 0)
        extensions (list): Ignored (all GFM extensions always enabled)

    Returns:
        str: HTML output
    """
    # In this simplified version, we always use GFM mode with all extensions
    return _github_flavored_markdown_to_html(text, options=options)
