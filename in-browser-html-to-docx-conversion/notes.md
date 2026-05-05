# HTML to DOCX Conversion Libraries - Research Notes

## Project Start
Date: 2025-12-03

## Objective
Review and compare NPM packages for HTML to DOCX conversion, identifying the best options for both browser-based and server-side conversion.

## Methodology
1. Install all packages using bun
2. Extract packages from node_modules to individual folders
3. Analyze each package individually
4. Compare packages and categorize them
5. Produce final report with recommendations

## Packages Reviewed (27 total)
- html-docx
- @types/html-docx-js
- html-docx-fixed
- html-docx-js
- html-docx-js-typescript
- html-docx-ts
- html-docx-js-version-updated
- html-docx-js-extends
- html-docx-js-a13
- html-docx-js-typescript-papersize-thenn
- html-to-docx
- @turbodocx/html-to-docx
- html-docx-typescript
- html-docx-js-typescript-modify
- @packback/html-to-docx
- @mark-beeby/html-to-docx
- @ckeditor/ckeditor5-export-word
- html-docx-js-extension-typescript
- html-to-docx-lite
- yk-html-to-docx
- pt-html-docx-js
- @gc123/html-docx-js
- html-docx-ts-improve
- html-to-docx-typescript
- @adalat-ai/html-to-docx
- prosemirror-docx
- @kaipeng/html-docx-js

## Progress Log

### Step 1: Package Installation
All 27 packages installed successfully:
- 19 non-scoped packages
- 8 scoped packages (@adalat-ai, @gc123, @kaipeng, @mark-beeby, @packback, @turbodocx, @types, @ckeditor)

All packages extracted to `/packages/` folder for analysis.

### Step 2: Package Categories Identified

**Category A: html-docx-js Family (Original)**
- html-docx-js (v0.3.1) - Original by Evidence Prime, CoffeeScript-based, uses jszip
- html-docx-fixed - Fork with bug fixes
- html-docx-js-typescript - TypeScript port
- html-docx-js-a13, html-docx-js-extends, html-docx-js-version-updated - Various forks
- @gc123/html-docx-js, @kaipeng/html-docx-js - Scoped forks

**Category B: html-to-docx Family (Modern)**
- html-to-docx (v1.8.0) - By privateOmega, modern architecture, UMD/ESM builds
- @turbodocx/html-to-docx (v1.18.1) - Most actively maintained fork, TypeScript types, RTL support
- @adalat-ai/html-to-docx, @mark-beeby/html-to-docx - Other forks
- html-to-docx-typescript, html-to-docx-lite - TypeScript variants

**Category C: TypeScript-First Libraries**
- html-docx-ts (v0.0.5) - Clean TypeScript implementation, header/footer support
- html-docx-typescript - Another TS implementation

**Category D: docx-Library Based**
- @packback/html-to-docx (v1.4.2) - Uses `docx` library, TypeScript, has CLI
- prosemirror-docx (v0.6.1) - For ProseMirror editor, uses `docx` library

**Category E: Commercial/Editor-Specific**
- @ckeditor/ckeditor5-export-word - Requires CKEditor license

**Category F: Deprecated/Minimal**
- html-docx - Placeholder/minimal
- @types/html-docx-js - Just TypeScript type definitions
- Various other forks with minimal maintenance

### Step 3: Key Package Analysis

#### html-docx-js (v0.3.1)
- **Author**: Evidence Prime
- **Last Updated**: ~2016 (old)
- **Browser Support**: Yes (primary target)
- **Dependencies**: jszip, lodash.escape, lodash.merge
- **Approach**: Templates-based, generates OOXML directly
- **Pros**: Simple, browser-native, minimal deps
- **Cons**: Old, limited features, no TypeScript, uses CoffeeScript

#### html-to-docx (v1.8.0)
- **Author**: privateOmega
- **Repository**: github.com/privateOmega/html-to-docx
- **Dependencies**: jszip, xmlbuilder2, image-size, lodash, nanoid
- **Approach**: VDOM-based conversion with xmlbuilder2
- **Pros**: Modern, active contributors, good documentation
- **Cons**: Larger bundle, more dependencies

#### @turbodocx/html-to-docx (v1.18.1)
- **Author**: TurboDocx, Inc.
- **Repository**: github.com/turbodocx/html-to-docx
- **TypeScript Support**: Full types (index.d.ts)
- **RTL Support**: Yes (Hebrew, Arabic)
- **Extra Features**: LRU caching, axios for images, unit tests
- **Pros**: Most actively maintained, TypeScript types, RTL, better testing
- **Cons**: Includes postinstall script (marketing)

#### @packback/html-to-docx (v1.4.2)
- **Architecture**: Uses `docx` library as dependency
- **TypeScript**: Full TypeScript implementation
- **CLI Support**: Yes (`html-to-docx` command)
- **Node Requirement**: >= 18.0.0
- **Pros**: Clean architecture, leverages mature `docx` library, CLI tool
- **Cons**: Requires jsdom for server-side (optional peer dep)

#### html-docx-ts (v0.0.5)
- **Author**: SG-CRT
- **Features**: Header/footer support
- **Dependencies**: jszip, browser-or-node, tslib
- **Pros**: Clean TypeScript, minimal deps, header/footer support
- **Cons**: Less popular, fewer features

#### prosemirror-docx (v0.6.1)
- **Author**: Curvenote (Rowan Cockett)
- **Purpose**: ProseMirror document to DOCX
- **Peer Dependency**: docx ^8.5.0 || ^9.0.0
- **Pros**: Great for ProseMirror-based editors
- **Cons**: Specific to ProseMirror, not general HTML

### Step 4: Browser vs Server-Side Analysis

**For In-Browser Conversion:**
1. Must work without Node.js APIs
2. Should have small bundle size
3. Should handle images via base64 or URL fetching

Best candidates:
- @turbodocx/html-to-docx - UMD build available, tested in browser
- html-to-docx - UMD build, browser examples
- html-docx-ts - Designed for browser/node via browser-or-node package

**For Server-Side (Node.js):**
1. Can use Node.js APIs (fs, path, etc.)
2. Better image handling options
3. Can leverage heavier dependencies

Best candidates:
- @packback/html-to-docx - Uses robust `docx` library
- @turbodocx/html-to-docx - Works in Node.js with full features
- html-to-docx - Has Node.js examples

### Step 5: Final Recommendations

**WINNER: @turbodocx/html-to-docx**
- Most actively maintained (v1.18.1 vs v1.8.0 for original)
- TypeScript support with types
- RTL language support (Hebrew, Arabic)
- Both browser and Node.js support
- Good test coverage

**RUNNER-UP (for docx-library approach): @packback/html-to-docx**
- If you need the power of the `docx` library
- Better for complex document generation
- Has CLI tool for scripting
- TypeScript-first

**FOR PROSEMIRROR EDITORS: prosemirror-docx**
- Specifically designed for ProseMirror
- Uses modern `docx` library

**LEGACY/SIMPLE NEEDS: html-docx-js**
- Minimal dependencies
- Works but limited features
- Good for very simple HTML

## Conclusion
The HTML-to-DOCX NPM ecosystem has evolved significantly. The original html-docx-js spawned many forks, but the modern html-to-docx family (especially @turbodocx/html-to-docx) offers the best combination of features, maintenance, and compatibility for both browser and server-side use cases.
