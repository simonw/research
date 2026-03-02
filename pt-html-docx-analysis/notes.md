# Analysis Notes: pt-html-docx-js Package

## Investigation Process

### 1. Initial Discovery (2025-12-03)
- Located package at `/home/user/research/in-browser-html-to-docx-conversion/packages/pt-html-docx-js`
- Read package.json to understand basic metadata
- Read README.md to understand functionality and approach

### 2. File Inventory
- Used `find` and `ls -l` commands to catalog all files with sizes
- Total package structure includes:
  - Source files in `src/` (CoffeeScript)
  - Build artifacts in `build/` (compiled JavaScript)
  - Bundled browser version in `dist/` (416KB)
  - Test files including sample HTML demo
  - Node modules (only jszip installed)

### 3. Code Analysis

#### Core Architecture
The package uses a clever approach called "altchunks" to convert HTML to DOCX:

**Main Files:**
- `src/api.coffee`: Public API with `asBlob()` and `save()` methods
- `src/internal.coffee`: Document generation and file structure creation
- `src/utils.coffee`: MHT document creation and image processing

**Key Technique:**
- Creates a valid DOCX file (which is a ZIP archive)
- Embeds HTML content as MHT (MHTML - web archive format)
- Uses Word's "altchunk" feature to have Word convert the HTML to native format
- This is a workaround - Word does the actual conversion, not this library

#### Template Files
- `document.tpl`: WordProcessingML document structure with altchunk reference
- `mht_document.tpl`: MHTML wrapper for HTML content
- `mht_part.tpl`: MHTML parts for embedded images

#### Build System
- Uses Gulp for build automation
- Browserify for bundling
- CoffeeScript for source code
- Outputs both Node.js module and browser bundle

### 4. Browser vs Server Support

**Browser:**
- Works in modern browsers with Blob support
- Tested on Chrome 36, Safari 7, IE 10
- Uses Blob API to generate downloadable files
- FileSaver.js polyfill included for saving files

**Server (Node.js):**
- Works on Node.js v0.10.12+
- Uses Buffer instead of Blob
- Can be used in server-side applications

### 5. Features & Capabilities

**Supported:**
- Basic HTML elements (all that browser supports)
- CSS styles (embedded in HTML)
- Images (base64 DATA URI only)
- Page orientation (portrait/landscape)
- Page margins (customizable)
- Relies on Word's HTML interpretation

**Image Handling:**
- Only supports base64-encoded images (DATA URI)
- Sample code shows how to convert regular images to base64
- Images embedded as separate MHTML parts

### 6. Limitations & Issues

**Major Limitations:**
1. **Compatibility Issues:**
   - NOT supported by Microsoft Word for Mac 2008
   - NOT supported by LibreOffice
   - NOT supported by Google Docs
   - Only works with MS Word 2007+ (Windows/modern Mac)

2. **Technical Limitations:**
   - Doesn't actually convert HTML to DOCX - relies on Word to do it
   - Quality depends on Word's HTML interpretation
   - Images must be base64-encoded (no external URLs)
   - Limited control over final output

3. **Deprecated/Old Dependencies:**
   - Uses JSZip 2.7.0 (current is 3.x)
   - Written in CoffeeScript (less common now)
   - Uses old Browserify 4.2.0 (current is 17.x)
   - Gulp 3.x (current is 4.x)
   - No updates since 2016

4. **Development Status:**
   - Last update: May 17, 2016 (version 0.3.1)
   - Likely unmaintained
   - Old tooling and dependencies

### 7. How It Actually Works

The conversion process is NOT a true HTML-to-DOCX converter. Instead:

1. Creates a minimal DOCX file structure (ZIP archive)
2. Embeds the HTML as an MHT document
3. Uses `<w:altChunk>` XML element in document.xml
4. When Word opens the file:
   - Word detects the altchunk reference
   - Word converts the MHT/HTML to native WordProcessingML
   - Word replaces the altchunk with converted content
5. This is why it only works in Word - other programs don't support altchunks

**Pros of this approach:**
- Small library size (only ~1500 lines of actual code)
- Leverages Word's HTML rendering capabilities
- Works for complex HTML/CSS that would be hard to manually convert

**Cons of this approach:**
- Not portable (Word-only)
- User must open in Word for conversion to happen
- No control over conversion quality
- Conversion happens client-side in Word, not in the library

### 8. Package Metadata Summary

- **Name:** pt-html-docx-js
- **Version:** 0.0.1 (package.json) vs 0.3.1 (bower.json)
- **Author:** Artur Nowak <artur.nowak@evidenceprime.com>
- **Contributors:** Ievgen Martynov
- **License:** MIT
- **Repository:** github.com/evidenceprime/html-docx-js
- **Description:** Converts HTML documents to DOCX in the browser

### 9. Use Cases

**Good for:**
- Simple HTML to DOCX conversion where Word availability is guaranteed
- Web applications targeting Microsoft Office users
- Quick exports from rich text editors (like TinyMCE example)
- When you need basic HTML/CSS support without complex conversion logic

**Not good for:**
- Cross-platform document generation
- Server-side generation for non-Word users
- When you need precise control over DOCX output
- When users have LibreOffice or Google Docs only
- Modern projects (outdated dependencies)

### 10. File Size Analysis

**Key files:**
- dist/html-docx.js: 416,073 bytes (bundled for browser)
- Source files: ~3KB total CoffeeScript
- node_modules/jszip: Large dependency (~400KB of the bundle)
- Test sample includes TinyMCE (loaded from CDN)

### 11. Security & Code Quality

**Observations:**
- No obvious malware or security issues
- Simple, readable code
- Good test coverage (both Node and PhantomJS tests)
- Uses fs.readFileSync for templates (blocking I/O)
- Regex for image processing could be improved
- No input validation on HTML content

### 12. Alternatives to Consider

For modern projects, better alternatives might include:
- html-docx-js-typescript (if it exists - updated fork)
- docx npm package (programmatic DOCX generation)
- mammoth.js (docx to html, reverse direction)
- pandoc (command-line, more comprehensive)
- html-to-docx (newer packages on npm)
