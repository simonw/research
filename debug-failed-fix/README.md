# Debug: Failed Cog Code Fix

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Summary

Commit 0dcfad4 attempted to fix cog code being rendered as visible text in the README by avoiding literal `-->` sequences in Python strings. The fix used string concatenation (`"--" + ">"`) which was conceptually correct, but the explanatory comment added to explain the fix contained the very sequence it was trying to avoid.

## The Problem

The README uses cogapp to embed Python code within HTML comments:
```html
<!--[[[cog
... python code ...
]]]-->
```

The HTML/Markdown parser looks for `-->` to close the comment. If `-->` appears anywhere in the raw text before the intended closing tag, the comment ends early and subsequent code becomes visible.

## Root Cause

The failed fix (commit 0dcfad4) added this Python comment:

```python
# Note: we construct these strings to avoid "-->" which would close the HTML comment
```

The quoted `"-->"` in the comment still contains the literal characters `-->`, which the HTML parser interprets as closing the HTML comment.

## The Fix

Changed the comment to avoid the problematic character sequence:

```python
# Note: we construct these marker strings via concatenation to avoid the HTML comment close sequence
```

## Key Lesson

When avoiding specific character sequences in source files that will be processed by multiple parsers (cogapp + HTML/Markdown), you must avoid the sequence everywhere in the raw text - including in comments, strings, and documentation - not just in the "functional" code.
