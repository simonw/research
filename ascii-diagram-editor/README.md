# ASCII Diagram Editor Research

## Objective
Research existing tools for editing ASCII diagrams with the following requirements:
- Paste in AI-created ASCII diagrams
- Edit them visually (like Miro/tldraw)
- Copy back to markdown
- Features: boxes, arrows, text, etc.
- No signup, simple utility

## Executive Summary

**Recommendation: Use existing tools - no need to build a new one.**

Several mature, open-source ASCII diagram editors already exist that fully meet all requirements. The two best options are:

1. **[Cascii](https://cascii.app)** - Most feature-rich with explicit import support
2. **[ASCIIFlow](https://asciiflow.com)** - Well-established and widely used

## Detailed Findings

### 1. Cascii (RECOMMENDED)

**URL:** https://cascii.app
**GitHub:** https://github.com/casparwylie/cascii-core
**License:** Open source

#### Features:
- ✅ **Import/Export:** Explicit "Paste / import text" feature for existing ASCII diagrams
- ✅ **Drawing Tools:** Shapes (squares, circles, diamonds), lines, free drawing, tables
- ✅ **Selection & Manipulation:** Area selection, multi-selection, resizing, moving, grouping
- ✅ **Layer Management:** Multiple layers with z-index control
- ✅ **History:** Full undo/redo support
- ✅ **Character Sets:** Both ASCII and Unicode support
- ✅ **Persistence:** Auto-save in browser storage, base64 import/export
- ✅ **Themes:** Multiple theme options
- ✅ **Offline Support:** Can download cascii.html and run locally
- ✅ **No Signup Required:** Works immediately

#### Technical Details:
- Built with vanilla JavaScript
- Zero dependencies (no frameworks, libraries, or build tools)
- Client-side only, no server required
- Desktop only (not optimized for mobile)

#### Why Cascii is Best:
- Explicitly designed for importing existing ASCII text
- Most feature-rich option
- Can run completely offline
- Zero dependencies = simple and fast

---

### 2. ASCIIFlow

**URL:** https://asciiflow.com
**GitHub:** https://github.com/lewish/asciiflow
**License:** MIT (open source)

#### Features:
- ✅ **Import/Export:** ASCIIFlow Infinity version supports importing ASCII text
- ✅ **Drawing Tools:** Freeform drawing, boxes, lines, arrows
- ✅ **Canvas:** Infinite canvas
- ✅ **Persistence:** Save to Google Drive (optional)
- ✅ **Export:** Direct export to text/HTML
- ✅ **No Signup Required:** Works immediately

#### Technical Details:
- Built with TypeScript (89%)
- Uses Bazel build system
- Client-side only

#### Why ASCIIFlow:
- Most popular and widely recognized
- Well-established and battle-tested
- Clean, simple interface

---

### 3. Other Tools Found

#### Textik (https://textik.com)
- Open source
- ASCII only (no Unicode)
- Basic features
- Less polished than Cascii/ASCIIFlow

#### AsciiP (https://asciip.dev)
- Limited information available
- Could not verify all features

#### Monodraw (https://monodraw.helftone.com)
- ❌ MacOS only
- ❌ Paid (though cheap)
- Otherwise excellent features

---

## Comparison Matrix

| Feature | Cascii | ASCIIFlow | Textik | Monodraw |
|---------|--------|-----------|--------|----------|
| Import existing diagrams | ✅ Yes | ✅ Yes* | ❓ Unknown | ✅ Yes |
| Visual editing | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes |
| Export to text | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes |
| Drawing tools | ✅ Rich | ✅ Good | ⚠️ Basic | ✅ Rich |
| No signup | ✅ Yes | ✅ Yes | ✅ Yes | ✅ Yes |
| Free | ✅ Yes | ✅ Yes | ✅ Yes | ❌ No |
| Open source | ✅ Yes | ✅ Yes | ✅ Yes | ❌ No |
| Cross-platform | ✅ Web | ✅ Web | ✅ Web | ❌ Mac only |
| Offline support | ✅ Yes | ⚠️ Partial | ❓ Unknown | ✅ Yes |
| Layers | ✅ Yes | ❌ No | ❌ No | ✅ Yes |
| Undo/Redo | ✅ Yes | ✅ Yes | ❓ Unknown | ✅ Yes |

*ASCIIFlow Infinity version

---

## Recommendation

### Use Cascii for this use case

**Primary recommendation:** [Cascii.app](https://cascii.app)

**Reasons:**
1. **Explicitly designed for the use case** - Has "Paste / import text" feature specifically for existing ASCII diagrams
2. **Most feature-rich** - Layers, grouping, multiple shapes, undo/redo
3. **Zero dependencies** - Simple, fast, reliable
4. **Open source** - Can self-host or modify if needed
5. **Offline support** - Download and use locally
6. **No signup** - Start using immediately

**Alternative:** [ASCIIFlow.com](https://asciiflow.com) if you prefer a simpler, more established tool

---

## Testing Recommendation

If you want to verify before committing to a tool:

1. Visit https://cascii.app
2. Try the "Paste / import text" feature with your architecture diagram
3. Test editing with the various tools
4. Export and verify the output

No development needed - the tool is ready to use immediately.

---

## Sources

- [Cascii GitHub Repository](https://github.com/casparwylie/cascii-core)
- [ASCIIFlow GitHub Repository](https://github.com/lewish/asciiflow)
- [ASCIIFlow website](https://asciiflow.com)
- [Cascii app](https://cascii.app)
- [Textik](https://textik.com)
- [Monodraw](https://monodraw.helftone.com)
