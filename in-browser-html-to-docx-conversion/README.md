# HTML to DOCX Conversion Libraries: A Comprehensive Analysis

## Executive Summary

This research analyzes 27 NPM packages that convert HTML to Microsoft Word DOCX format. The packages were evaluated for browser compatibility, server-side support, maintenance status, TypeScript support, and feature completeness.

**Key Finding:** The ecosystem has evolved from the original `html-docx-js` (2014) to modern solutions like `@turbodocx/html-to-docx`, which offers the best balance of features, maintenance, and cross-platform support.

## Quick Recommendations

| Use Case | Recommended Package | Version |
|----------|---------------------|---------|
| **Browser + Server (General)** | `@turbodocx/html-to-docx` | 1.18.1 |
| **Server-side with `docx` library** | `@packback/html-to-docx` | 1.4.2 |
| **ProseMirror Editors** | `prosemirror-docx` | 0.6.1 |
| **Legacy/Simple HTML** | `html-docx-js` | 0.3.1 |

## Packages Analyzed

### Category A: html-docx-js Family (Original)
The original approach from Evidence Prime (~2014). Template-based OOXML generation.

| Package | Version | TypeScript | Notes |
|---------|---------|------------|-------|
| html-docx-js | 0.3.1 | No | Original, CoffeeScript |
| html-docx-fixed | - | No | Bug fixes fork |
| html-docx-js-typescript | - | Yes | TypeScript port |
| html-docx-js-a13 | - | No | Fork |
| html-docx-js-extends | - | No | Extended fork |
| @gc123/html-docx-js | - | No | Scoped fork |
| @kaipeng/html-docx-js | - | No | Scoped fork |

### Category B: html-to-docx Family (Modern)
Modern architecture with VDOM-based conversion and xmlbuilder2.

| Package | Version | TypeScript | Notes |
|---------|---------|------------|-------|
| html-to-docx | 1.8.0 | No (has @types) | By privateOmega |
| **@turbodocx/html-to-docx** | **1.18.1** | **Yes** | **Best maintained, RTL support** |
| @adalat-ai/html-to-docx | - | Yes | Fork |
| @mark-beeby/html-to-docx | - | No | Fork |
| html-to-docx-typescript | - | Yes | TS variant |
| html-to-docx-lite | - | No | Lightweight variant |

### Category C: TypeScript-First Libraries

| Package | Version | Notes |
|---------|---------|-------|
| html-docx-ts | 0.0.5 | Header/footer support |
| html-docx-typescript | - | TS implementation |

### Category D: docx-Library Based
These packages leverage the mature `docx` library for document generation.

| Package | Version | Notes |
|---------|---------|-------|
| **@packback/html-to-docx** | **1.4.2** | **Has CLI, TypeScript** |
| prosemirror-docx | 0.6.1 | For ProseMirror editors |

### Category E: Commercial/Editor-Specific

| Package | Notes |
|---------|-------|
| @ckeditor/ckeditor5-export-word | Requires CKEditor license |

## Detailed Analysis

### Winner: @turbodocx/html-to-docx (v1.18.1)

**Why it's the best choice:**

1. **Active Maintenance**: Latest version 1.18.1 vs original's 1.8.0
2. **TypeScript Support**: Full type definitions included
3. **RTL Languages**: Hebrew and Arabic support
4. **Dual Environment**: Works in both browser and Node.js
5. **Modern Dependencies**: Uses htmlparser2, axios, LRU cache
6. **Test Coverage**: Includes Jest unit tests

**Installation:**
```bash
npm install @turbodocx/html-to-docx
```

**Usage (Browser):**
```javascript
import HTMLtoDOCX from '@turbodocx/html-to-docx';

const htmlContent = '<h1>Hello World</h1><p>This is a test.</p>';
const blob = await HTMLtoDOCX(htmlContent, null, {
  table: { row: { cantSplit: true } },
  footer: true,
  pageNumber: true,
});
```

**Usage (Node.js):**
```javascript
const HTMLtoDOCX = require('@turbodocx/html-to-docx');
const fs = require('fs');

const buffer = await HTMLtoDOCX('<h1>Hello</h1>', null, {});
fs.writeFileSync('output.docx', buffer);
```

### Runner-up: @packback/html-to-docx (v1.4.2)

**Why consider it:**

1. **docx Library**: Leverages the mature and feature-rich `docx` package
2. **CLI Tool**: Includes command-line interface
3. **TypeScript-First**: Written entirely in TypeScript
4. **Clean Architecture**: Simpler codebase to understand

**Installation:**
```bash
npm install @packback/html-to-docx
# For server-side:
npm install jsdom
```

**CLI Usage:**
```bash
html-to-docx input.html output.docx
```

### For ProseMirror: prosemirror-docx (v0.6.1)

**When to use:**
- Building with ProseMirror/TipTap editors
- Need direct ProseMirror document export

**Installation:**
```bash
npm install prosemirror-docx docx
```

## Feature Comparison

| Feature | @turbodocx | @packback | html-docx-js |
|---------|------------|-----------|--------------|
| Browser Support | Yes | Yes | Yes |
| Node.js Support | Yes | Yes | Yes |
| TypeScript | Yes | Yes | No |
| RTL Languages | Yes | No | No |
| Images | Yes (URL/base64) | Yes | Limited |
| Tables | Yes | Yes | Basic |
| Headers/Footers | Yes | Via docx lib | No |
| Page Numbers | Yes | Via docx lib | No |
| CLI | No | Yes | No |
| Bundle Size | ~150KB | ~200KB | ~30KB |

## Architecture Approaches

### Approach 1: Direct OOXML Generation (html-docx-js family)
- Generates OOXML XML directly using templates
- Pros: Smaller bundle, simpler
- Cons: Limited features, harder to extend

### Approach 2: VDOM + xmlbuilder2 (html-to-docx family)
- Parses HTML to virtual DOM, builds XML with xmlbuilder2
- Pros: More flexible, better HTML support
- Cons: Larger bundle

### Approach 3: docx Library Wrapper (@packback, prosemirror-docx)
- Converts HTML elements to docx library objects
- Pros: Full docx feature access, well-maintained base
- Cons: Additional dependency, potentially larger bundle

## Browser Considerations

For in-browser usage, ensure:

1. **Bundle Format**: Use UMD or ESM builds
2. **Image Handling**: Base64 encoding or CORS-enabled URLs
3. **File Download**: Use FileSaver.js or native Blob APIs

```javascript
// Download the generated document
const blob = await HTMLtoDOCX(html);
const url = URL.createObjectURL(blob);
const a = document.createElement('a');
a.href = url;
a.download = 'document.docx';
a.click();
```

## Server-Side Considerations

For Node.js usage:

1. **File System**: Direct buffer/stream writing
2. **Image Handling**: Fetch from URLs or read from filesystem
3. **Memory**: Consider streaming for large documents

## Conclusion

The HTML-to-DOCX NPM ecosystem offers options for every use case:

- **For most projects**: Use `@turbodocx/html-to-docx` for its balance of features, maintenance, and compatibility
- **For complex documents**: Consider `@packback/html-to-docx` to leverage the full `docx` library
- **For ProseMirror editors**: Use `prosemirror-docx`
- **For minimal needs**: The original `html-docx-js` still works for simple HTML

The key differentiator is maintenance: the TurboDocx fork has diverged significantly from the original with improvements in TypeScript support, RTL languages, and general reliability.

## References

- [@turbodocx/html-to-docx](https://github.com/turbodocx/html-to-docx)
- [html-to-docx (original)](https://github.com/privateOmega/html-to-docx)
- [@packback/html-to-docx](https://www.npmjs.com/package/@packback/html-to-docx)
- [prosemirror-docx](https://github.com/curvenote/prosemirror-docx)
- [docx library](https://github.com/dolanmiu/docx)
