# Debug Investigation: Failed Cog Code Fix

## Initial Problem

Commit 0dcfad4 ("Fix cog code rendering by escaping HTML comment terminators (#54)") was supposed to fix an issue where Python code inside cog blocks was being rendered as visible text in the GitHub README.

The fix attempted to avoid having `-->` in the source by using string concatenation:

```python
AI_NOTE_START = "<!-- AI-GENERATED-NOTE --" + ">"
AI_NOTE_END = "<!-- /AI-GENERATED-NOTE --" + ">"
```

## Investigation

Searched for all occurrences of `-->` in README.md:

```
Line 121: # Note: we construct these strings to avoid "-->" which would close the HTML comment
Line 162: ]]]-->
Line 592: <!--[[[end]]]-->
Line 620: ]]]-->
Line 622: <!--[[[end]]]-->
```

## Root Cause Found!

**Line 121** - The explanatory Python comment itself contains the literal `-->` sequence:

```python
# Note: we construct these strings to avoid "-->" which would close the HTML comment
```

The HTML parser doesn't distinguish between Python code, Python strings, or Python comments - it just scans for `-->` in the raw text. When it encounters `-->` on line 121, it interprets that as closing the `<!--[[[cog` HTML comment that started on line 9.

Everything after that `-->` is then rendered as visible text.

## The Irony

The fix was conceptually correct (using string concatenation), but the comment explaining WHY they did it contains the very sequence they were trying to avoid!

## Solution

Remove or rewrite the comment to avoid the literal `-->` sequence. Options:
1. Remove the comment entirely
2. Use string concatenation in the comment too: `"--" + ">"`
3. Use a different phrasing: "avoid the HTML comment terminator sequence"

## Fix Applied

Rewrote line 121 to avoid the problematic sequence.
