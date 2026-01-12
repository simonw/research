# ASCII Diagram Editor Research

## Task
Research and potentially build a tool for editing ASCII diagrams like architecture diagrams. Requirements:
- Paste in AI-created ASCII diagrams
- Edit them visually (like Miro/tldraw)
- Copy back to markdown
- Features: boxes, arrows, text, etc.
- No signup, simple utility

## Research Phase

### Known Solutions
User mentioned:
- https://asciiflow.com
- https://monodraw.helftone.com

### Research Findings

#### Existing Tools Found:
1. **ASCIIFlow** (asciiflow.com)
   - Most popular option
   - Infinite canvas, save to Google Drive
   - Freeform drawing, export to text/HTML
   - Free, no signup required

2. **Textik** (textik.com)
   - Web-based ASCII diagramming
   - ASCII characters only (not Unicode)
   - Open source
   - Free, no signup

3. **Cascii** (cascii.app)
   - Open source (vanilla JavaScript)
   - Multiple layers with z-index control
   - Undo/redo
   - Text layers, shapes (circles, diamonds, squares, tables)
   - Multiple character sets (Unicode and ASCII)
   - Three themes
   - Desktop only
   - **No signup required**
   - Persists data in local storage

4. **AsciiP** (asciip.dev)
   - Online ASCII diagram editor
   - Could not fetch details (SSL error)

5. **Monodraw** (monodraw.helftone.com)
   - MacOS only
   - Paid (but cheap)
   - Interactive ASCII diagramming

Let me investigate GitHub repos for these tools...

### GitHub Repositories:

1. **ASCIIFlow**
   - Repo: https://github.com/lewish/asciiflow
   - Built with: TypeScript (89%), Bazel build system
   - MIT License (open source)
   - Client-side only, no server dependencies
   - Need to verify import functionality

2. **Cascii**
   - Repo: https://github.com/casparwylie/cascii-core
   - **✅ SUPPORTS PASTE/IMPORT of existing ASCII text**
   - Zero dependencies (vanilla JS, no frameworks)
   - Features:
     - Import/export via base64
     - Area selection, multi-selection
     - Shapes: squares, circles, diamonds, tables
     - Line types, free drawing, erasing
     - Grouping, ordering, duplication
     - Full undo/redo
     - Multiple layers
     - ASCII and Unicode support
     - Auto-save in browser storage
     - Theme customization
   - Can run offline (download cascii.html)
   - Hosted at cascii.app with shareable links

### Key Finding:
**Cascii appears to meet ALL requirements:**
- ✅ Can paste/import existing ASCII diagrams
- ✅ Visual editing like Miro/tldraw
- ✅ Copy back to markdown
- ✅ Tools: boxes, arrows, text, shapes
- ✅ No signup required
- ✅ Easy to use utility
- ✅ Open source

**ASCIIFlow also has import capabilities:**
- ASCIIFlow Infinity version has explicit Import/Export
- Can paste ASCII/text to import onto canvas
- Main asciiflow.com may have varying import UI

## Conclusion

**RECOMMENDATION: Use existing tools rather than building new one**

Both **Cascii** and **ASCIIFlow** already provide the exact functionality needed:
1. Import/paste existing ASCII diagrams
2. Edit visually with tools (boxes, lines, shapes, text)
3. Export/copy back to markdown
4. Free, no signup required
5. Open source

**Best options:**
1. **Cascii.app** - Most feature-rich, explicit import support, can run offline
2. **ASCIIFlow Infinity** - Well-established, also supports import

No need to build a new tool - these solutions are mature and battle-tested.
