# Analysis Report: pt-html-docx-js Package

## Overview

This report provides a comprehensive analysis of the `pt-html-docx-js` npm package, a browser-based HTML to DOCX converter that uses Microsoft Word's "altchunks" feature.

## Executive Summary

**pt-html-docx-js** is a lightweight library that converts HTML documents to DOCX format in the browser. Rather than performing true HTML-to-DOCX conversion, it cleverly embeds HTML content as MHT (MHTML) within a DOCX file structure and relies on Microsoft Word to perform the actual conversion using the "altchunk" feature.

**Key Findings:**
- ✅ Small, simple codebase (~1500 LOC)
- ✅ Works in both browser and Node.js environments
- ⚠️ **Only compatible with Microsoft Word 2007+** (Windows/modern Mac)
- ⚠️ **NOT compatible** with LibreOffice, Google Docs, or Word for Mac 2008
- ❌ Outdated dependencies (last updated 2016)
- ❌ Uses deprecated tooling (CoffeeScript, old Browserify, Gulp 3.x)

## Comprehensive Analysis (JSON Format)

```json
{
  "package_overview": {
    "name": "pt-html-docx-js",
    "version": "0.0.1",
    "bower_version": "0.3.1",
    "author": "Artur Nowak <artur.nowak@evidenceprime.com>",
    "contributors": [
      {
        "name": "Ievgen Martynov",
        "email": "ievgen.martynov@gmail.com"
      }
    ],
    "license": "MIT",
    "description": "Converts HTML documents to DOCX in the browser",
    "repository": {
      "type": "git",
      "url": "git://github.com/evidenceprime/html-docx-js.git"
    },
    "main_entry": "build/api.js",
    "browser_bundle": "dist/html-docx.js",
    "last_updated": "2016-05-17",
    "current_status": "Unmaintained (no updates since 2016)"
  },

  "file_inventory": {
    "total_files": 67,
    "source_files": [
      {
        "path": "src/api.coffee",
        "size_bytes": 320,
        "purpose": "Public API - exports asBlob() and save() methods"
      },
      {
        "path": "src/internal.coffee",
        "size_bytes": 1516,
        "purpose": "Document generation, file structure creation"
      },
      {
        "path": "src/utils.coffee",
        "size_bytes": 1302,
        "purpose": "MHT document creation and image processing"
      },
      {
        "path": "src/templates/document.tpl",
        "size_bytes": 2194,
        "purpose": "WordProcessingML document template with altchunk"
      },
      {
        "path": "src/templates/mht_document.tpl",
        "size_bytes": 337,
        "purpose": "MHTML wrapper template"
      },
      {
        "path": "src/templates/mht_part.tpl",
        "size_bytes": 170,
        "purpose": "MHTML part template for images"
      }
    ],
    "build_files": [
      {
        "path": "build/api.js",
        "size_bytes": 412,
        "purpose": "Compiled API module for Node.js"
      },
      {
        "path": "build/internal.js",
        "size_bytes": 1985,
        "purpose": "Compiled internal module"
      },
      {
        "path": "build/utils.js",
        "size_bytes": 1748,
        "purpose": "Compiled utils module"
      },
      {
        "path": "build/FileSaver.js",
        "size_bytes": 7383,
        "purpose": "FileSaver.js library for browser downloads"
      }
    ],
    "distribution_files": [
      {
        "path": "dist/html-docx.js",
        "size_bytes": 416073,
        "purpose": "Browserified standalone bundle (UMD)"
      }
    ],
    "asset_files": [
      {
        "path": "src/assets/rels.xml",
        "size_bytes": 330,
        "purpose": "DOCX relationships file"
      },
      {
        "path": "src/assets/content_types.xml",
        "size_bytes": 465,
        "purpose": "DOCX content types definition"
      },
      {
        "path": "src/assets/document.xml.rels",
        "size_bytes": 306,
        "purpose": "Document relationships file"
      }
    ],
    "test_files": [
      {
        "path": "test/sample.html",
        "size_bytes": 3445,
        "purpose": "Browser demo with TinyMCE integration"
      },
      {
        "path": "test/index.coffee",
        "size_bytes": 5374,
        "purpose": "Test suite"
      },
      {
        "path": "test/cat.jpg",
        "size_bytes": 45913,
        "purpose": "Test image for image conversion demo"
      }
    ],
    "config_files": [
      {
        "path": "package.json",
        "size_bytes": 1268
      },
      {
        "path": "bower.json",
        "size_bytes": 620
      },
      {
        "path": "gulpfile.coffee",
        "size_bytes": 2720
      },
      {
        "path": "coffeelint.json",
        "size_bytes": 1448
      }
    ],
    "documentation": [
      {
        "path": "README.md",
        "size_bytes": 3538
      },
      {
        "path": "LICENSE",
        "size_bytes": 1081
      },
      {
        "path": "CHANGELOG.md",
        "size_bytes": 766
      }
    ]
  },

  "code_analysis": {
    "architecture": {
      "type": "Altchunk-based conversion",
      "description": "Does NOT perform true HTML-to-DOCX conversion. Instead, embeds HTML as MHT within DOCX structure and relies on Microsoft Word to perform conversion when file is opened.",
      "conversion_flow": [
        "1. Create JSZip instance for DOCX structure",
        "2. Add required DOCX XML files ([Content_Types].xml, .rels)",
        "3. Convert HTML to MHT format with embedded images",
        "4. Create document.xml with <w:altChunk> reference to MHT",
        "5. Package everything into ZIP/DOCX format",
        "6. Return as Blob (browser) or Buffer (Node.js)",
        "7. When opened in Word, Word converts MHT to native format"
      ]
    },

    "api_surface": {
      "methods": [
        {
          "name": "asBlob",
          "signature": "asBlob(html: string, options?: object): Blob|Buffer",
          "description": "Converts HTML string to DOCX Blob/Buffer",
          "parameters": [
            {
              "name": "html",
              "type": "string",
              "required": true,
              "description": "Complete HTML document including DOCTYPE, html, and body tags"
            },
            {
              "name": "options",
              "type": "object",
              "required": false,
              "properties": {
                "orientation": {
                  "type": "string",
                  "values": ["portrait", "landscape"],
                  "default": "portrait"
                },
                "margins": {
                  "type": "object",
                  "properties": {
                    "top": {"type": "number", "default": 1440, "unit": "twentieths of a point"},
                    "right": {"type": "number", "default": 1440},
                    "bottom": {"type": "number", "default": 1440},
                    "left": {"type": "number", "default": 1440},
                    "header": {"type": "number", "default": 720},
                    "footer": {"type": "number", "default": 720},
                    "gutter": {"type": "number", "default": 0}
                  }
                }
              }
            }
          ],
          "returns": "Blob in browser, Buffer in Node.js"
        },
        {
          "name": "save",
          "signature": "save(converted: Blob, name: string): void",
          "description": "Saves converted Blob to file using FileSaver.js",
          "parameters": [
            {
              "name": "converted",
              "type": "Blob",
              "required": true
            },
            {
              "name": "name",
              "type": "string",
              "required": true,
              "description": "Filename for download"
            }
          ]
        }
      ],

      "module_format": "UMD (Universal Module Definition)",
      "exports": {
        "commonjs": "require('html-docx')",
        "amd": "define(['html-docx'], ...)",
        "global": "window.htmlDocx"
      }
    },

    "core_functions": {
      "generateDocument": {
        "file": "src/internal.coffee",
        "purpose": "Generates final DOCX as Blob or Buffer",
        "implementation": "Uses JSZip.generate() to create arraybuffer, wraps in Blob or Buffer"
      },
      "renderDocumentFile": {
        "file": "src/internal.coffee",
        "purpose": "Renders document.xml from template with page settings",
        "features": ["Page size", "Orientation", "Margins", "AltChunk reference"]
      },
      "addFiles": {
        "file": "src/internal.coffee",
        "purpose": "Adds all required files to DOCX ZIP structure",
        "files_added": [
          "[Content_Types].xml",
          "_rels/.rels",
          "word/document.xml",
          "word/afchunk.mht",
          "word/_rels/document.xml.rels"
        ]
      },
      "getMHTdocument": {
        "file": "src/utils.coffee",
        "purpose": "Converts HTML to MHT format",
        "features": [
          "Escapes = signs as =3D for MHT encoding",
          "Processes base64 images",
          "Creates multipart MIME document"
        ]
      },
      "_prepareImageParts": {
        "file": "src/utils.coffee",
        "purpose": "Extracts base64 images and creates MHT parts",
        "regex": "/(\"data:(\\w+\\/\\w+);(\\w+),(\\S+)\")/g",
        "process": "Replaces DATA URIs with fake file:/// paths, creates MIME parts"
      }
    }
  },

  "browser_vs_server_support": {
    "browser": {
      "supported": true,
      "requirements": [
        "Modern browser with Blob support",
        "OR Blob.js polyfill for older browsers"
      ],
      "tested_on": [
        "Google Chrome 36+",
        "Safari 7+",
        "Internet Explorer 10+"
      ],
      "features": [
        "Client-side DOCX generation",
        "In-browser file download (FileSaver.js)",
        "Works with rich text editors (TinyMCE demo included)"
      ],
      "limitations": [
        "Safari file saving requires Flash workaround (not included in demo)",
        "Demo sample.html doesn't work in Safari without Downloadify"
      ]
    },

    "server": {
      "supported": true,
      "requirements": [
        "Node.js v0.10.12 or higher"
      ],
      "features": [
        "Server-side DOCX generation",
        "Uses Buffer instead of Blob",
        "Can save to filesystem or stream to client"
      ],
      "sample_code_available": "github.com/evidenceprime/html-docx-js-node-sample"
    },

    "universal": {
      "module_format": "UMD (Browserify standalone)",
      "works_in": [
        "Browser (global variable)",
        "Node.js (CommonJS)",
        "AMD loaders (RequireJS, etc.)"
      ]
    }
  },

  "key_features": {
    "html_element_support": {
      "type": "Indirect (via Word's HTML parser)",
      "description": "Supports any HTML elements that Microsoft Word can interpret",
      "common_elements": [
        "Paragraphs (<p>)",
        "Headings (<h1>-<h6>)",
        "Lists (<ul>, <ol>, <li>)",
        "Tables (<table>, <tr>, <td>)",
        "Text formatting (<b>, <i>, <u>, <strong>, <em>)",
        "Links (<a>)",
        "Images (<img> with base64 DATA URI only)",
        "Line breaks (<br>)",
        "Divs and spans with styles"
      ],
      "note": "Actual support depends on Word's HTML rendering capabilities"
    },

    "css_style_support": {
      "type": "Indirect (via Word's CSS parser)",
      "description": "Styles embedded in HTML are interpreted by Word",
      "supported_approaches": [
        "Inline styles (style attribute)",
        "Style tags (<style> in <head>)",
        "Class-based styling"
      ],
      "common_styles": [
        "Font family, size, color",
        "Text alignment",
        "Text decoration (bold, italic, underline)",
        "Background colors",
        "Margins and padding (interpreted by Word)",
        "Borders"
      ],
      "limitations": [
        "Complex CSS may not convert well",
        "Word's CSS support is limited",
        "No guarantee of pixel-perfect conversion",
        "Flexbox/Grid not supported",
        "Modern CSS features unlikely to work"
      ]
    },

    "image_support": {
      "supported_formats": [
        "PNG (via DATA URI)",
        "JPEG (via DATA URI)",
        "GIF (via DATA URI)",
        "Any format supported as base64 DATA URI"
      ],
      "requirements": [
        "Images must be base64-encoded",
        "Must use DATA URI format: data:image/png;base64,..."
      ],
      "helper_code_provided": {
        "function": "convertImagesToBase64()",
        "location": "test/sample.html",
        "purpose": "Converts regular <img> sources to base64 using Canvas API"
      },
      "process": [
        "Extracts DATA URI images from HTML",
        "Creates separate MHTML parts for each image",
        "Replaces DATA URI with fake file:///C:/fake/imageN.ext path",
        "Embeds image data in MHT as base64"
      ]
    },

    "page_configuration": {
      "orientation": {
        "values": ["portrait", "landscape"],
        "default": "portrait"
      },
      "page_size": {
        "portrait": {"width": 12240, "height": 15840},
        "landscape": {"width": 15840, "height": 12240},
        "unit": "twentieths of a point (1/1440 inch)"
      },
      "margins": {
        "configurable": true,
        "default_values": {
          "top": "1440 (2.54 cm / 1 inch)",
          "right": "1440 (2.54 cm / 1 inch)",
          "bottom": "1440 (2.54 cm / 1 inch)",
          "left": "1440 (2.54 cm / 1 inch)",
          "header": "720 (1.27 cm / 0.5 inch)",
          "footer": "720 (1.27 cm / 0.5 inch)",
          "gutter": "0"
        }
      }
    },

    "docx_structure": {
      "format": "Office Open XML (OOXML)",
      "container": "ZIP archive",
      "required_files": [
        "[Content_Types].xml - defines content types in package",
        "_rels/.rels - package relationships",
        "word/document.xml - main document with altchunk reference",
        "word/afchunk.mht - embedded MHT content",
        "word/_rels/document.xml.rels - document relationships"
      ],
      "altchunk_mechanism": {
        "element": "<w:altChunk r:id='htmlChunk' />",
        "relationship_id": "htmlChunk",
        "relationship_type": "http://schemas.openxmlformats.org/officeDocument/2006/relationships/aFChunk",
        "target": "afchunk.mht",
        "description": "Tells Word to import and convert external content"
      }
    }
  },

  "limitations_and_issues": {
    "compatibility_issues": [
      {
        "severity": "CRITICAL",
        "issue": "Microsoft Word for Mac 2008 not supported",
        "reason": "This version does not support altchunks",
        "workaround": "None - users must use newer Word version"
      },
      {
        "severity": "CRITICAL",
        "issue": "LibreOffice not supported",
        "reason": "LibreOffice does not implement altchunk feature",
        "impact": "Files open but show error or empty content",
        "workaround": "None - must use Microsoft Word"
      },
      {
        "severity": "CRITICAL",
        "issue": "Google Docs not supported",
        "reason": "Google Docs does not support altchunks",
        "impact": "Cannot open or convert files properly",
        "workaround": "None - must use Microsoft Word"
      },
      {
        "severity": "HIGH",
        "issue": "Only works with Microsoft Word 2007 and later",
        "reason": "Altchunk feature introduced in Word 2007",
        "compatible_versions": [
          "Microsoft Word 2007 (Windows)",
          "Microsoft Word 2010 (Windows)",
          "Microsoft Word 2013 (Windows)",
          "Microsoft Word 2016 (Windows/Mac)",
          "Microsoft Word 2019 (Windows/Mac)",
          "Microsoft 365 Word (Windows/Mac)"
        ]
      }
    ],

    "technical_limitations": [
      {
        "limitation": "Not a true HTML-to-DOCX converter",
        "description": "Library does not parse HTML or generate WordProcessingML. It embeds HTML and lets Word do the conversion.",
        "implications": [
          "No control over conversion quality",
          "Cannot preview conversion result without Word",
          "Conversion happens when user opens file, not when generated",
          "Different Word versions may convert differently"
        ]
      },
      {
        "limitation": "Images must be base64-encoded",
        "description": "External image URLs not supported",
        "workaround": "Convert images to base64 using Canvas API (example provided)",
        "impact": "Larger file sizes, must download images first"
      },
      {
        "limitation": "Requires complete HTML document",
        "description": "Must include DOCTYPE, <html>, <head>, <body> tags",
        "reason": "MHT format expects complete document",
        "impact": "Cannot convert HTML snippets directly"
      },
      {
        "limitation": "No validation or error handling",
        "description": "Library accepts any string as HTML, no validation",
        "risk": "Invalid HTML may produce broken DOCX or Word errors"
      },
      {
        "limitation": "Conversion quality depends on Word",
        "description": "Complex CSS or modern HTML may not convert well",
        "examples": [
          "CSS Grid/Flexbox not supported",
          "CSS animations ignored",
          "Complex selectors may not work",
          "JavaScript in HTML ignored"
        ]
      }
    ],

    "dependency_issues": [
      {
        "severity": "HIGH",
        "dependency": "jszip",
        "installed_version": "2.7.0",
        "current_version": "3.10.1",
        "issue": "Major version behind (v2 vs v3)",
        "risks": [
          "Security vulnerabilities",
          "Missing features",
          "Compatibility issues with modern JS"
        ],
        "breaking_changes": "JSZip v3 has different API"
      },
      {
        "severity": "MEDIUM",
        "dependency": "browserify",
        "installed_version": "4.2.0",
        "current_version": "17.0.0",
        "issue": "Extremely outdated (2014 version)",
        "impact": "Build process may not work on modern Node.js"
      },
      {
        "severity": "MEDIUM",
        "dependency": "gulp",
        "installed_version": "3.8.5",
        "current_version": "4.0.2",
        "issue": "Using Gulp 3.x, current is 4.x with breaking changes",
        "impact": "Build tasks may fail on modern systems"
      },
      {
        "severity": "LOW",
        "dependency": "CoffeeScript",
        "issue": "Source code written in CoffeeScript",
        "impact": "Harder to maintain, less popular language, requires compilation",
        "note": "CoffeeScript is still maintained but declining in popularity"
      },
      {
        "severity": "MEDIUM",
        "dependency": "lodash",
        "installed_version": "Partial (lodash.escape 3.0.0, lodash.merge 3.2.0)",
        "current_version": "4.17.21",
        "issue": "Using old individual lodash packages",
        "note": "These still work but are outdated"
      }
    ],

    "maintenance_status": {
      "last_update": "2016-05-17",
      "years_since_update": 8.5,
      "status": "UNMAINTAINED",
      "evidence": [
        "No commits since 2016",
        "Dependencies not updated",
        "Issues/PRs likely unaddressed",
        "No response to security issues"
      ],
      "risks": [
        "Security vulnerabilities in dependencies",
        "Incompatibility with modern browsers/Node.js",
        "No bug fixes or improvements",
        "May break with future browser updates"
      ]
    },

    "security_concerns": [
      {
        "issue": "No input sanitization",
        "description": "HTML input not validated or sanitized",
        "risk": "If user-supplied HTML, could contain malicious content",
        "mitigation": "Validate/sanitize HTML before passing to library"
      },
      {
        "issue": "Outdated dependencies",
        "description": "Old versions may have known vulnerabilities",
        "specific": "JSZip 2.7.0 is from 2015, may have security issues",
        "recommendation": "Audit dependencies for CVEs"
      },
      {
        "issue": "eval-like operations",
        "description": "Template processing might use eval",
        "status": "Low risk - uses lodash micro templates",
        "note": "Should verify template system is safe"
      }
    ],

    "code_quality_issues": [
      {
        "issue": "Blocking filesystem operations",
        "location": "src/internal.coffee:37-43",
        "code": "fs.readFileSync(__dirname + '/assets/...')",
        "problem": "Uses synchronous fs.readFileSync",
        "impact": "Blocks event loop in Node.js",
        "severity": "MEDIUM"
      },
      {
        "issue": "Limited regex for images",
        "location": "src/utils.coffee:14",
        "regex": "/\"data:(\\w+\\/\\w+);(\\w+),(\\S+)\"/g",
        "problem": "May not match all valid DATA URIs",
        "examples": [
          "Single quotes not supported",
          "No quotes not supported",
          "Complex content types might fail"
        ],
        "severity": "LOW"
      },
      {
        "issue": "No TypeScript types",
        "impact": "No autocomplete or type safety in TypeScript projects",
        "workaround": "Would need to create .d.ts file manually",
        "severity": "LOW"
      },
      {
        "issue": "Global variable modification",
        "location": "gulpfile.coffee:62",
        "code": "global.isWatching = true",
        "problem": "Modifies global state for build configuration",
        "severity": "LOW"
      }
    ]
  },

  "dependencies": {
    "production": {
      "jszip": {
        "version_specified": "^2.3.0",
        "version_installed": "2.7.0",
        "purpose": "Creating and manipulating ZIP archives (DOCX is a ZIP)",
        "size_contribution": "~90% of bundle size",
        "current_latest": "3.10.1",
        "note": "Major version behind"
      },
      "lodash.escape": {
        "version": "^3.0.0",
        "purpose": "Escaping strings for templates",
        "current_latest": "4.0.1"
      },
      "lodash.merge": {
        "version": "^3.2.0",
        "purpose": "Deep merging objects for options",
        "current_latest": "4.6.2"
      }
    },

    "development": {
      "note": "Many outdated development dependencies",
      "major_tools": [
        "gulp: ^3.8.5 (current: 4.0.2)",
        "browserify: ^4.2.0 (current: 17.0.0)",
        "coffeeify: ^0.6.0 (current: 3.0.1)",
        "mocha: ^1.20.1 (current: 10.2.0)",
        "chai: ^1.9.1 (current: 4.3.10)"
      ],
      "impact": "Build process may fail on modern Node.js versions"
    }
  },

  "alternatives": {
    "for_modern_projects": [
      {
        "name": "docx",
        "npm": "docx",
        "approach": "Programmatic DOCX generation",
        "pros": [
          "Active maintenance",
          "Full control over output",
          "Works everywhere (no Word required)",
          "TypeScript support"
        ],
        "cons": [
          "Must programmatically construct document",
          "No HTML parsing included",
          "Steeper learning curve"
        ]
      },
      {
        "name": "html-docx-js (forks)",
        "approach": "Updated forks of this library",
        "note": "Check npm for forks with updated dependencies",
        "search": "Search npm for 'html-docx' to find maintained forks"
      },
      {
        "name": "pandoc",
        "approach": "Command-line document converter",
        "pros": [
          "Very comprehensive conversion",
          "Supports many formats",
          "True HTML-to-DOCX conversion"
        ],
        "cons": [
          "Requires system installation",
          "Not pure JavaScript",
          "Server-side only"
        ]
      },
      {
        "name": "mammoth.js",
        "npm": "mammoth",
        "approach": "DOCX to HTML (reverse direction)",
        "note": "For reading DOCX, not creating it"
      }
    ],

    "when_to_use_this_library": [
      "Quick prototypes where Word compatibility is guaranteed",
      "Internal tools for Microsoft Office environments",
      "Simple HTML to DOCX needs with minimal requirements",
      "When you specifically need the altchunk approach",
      "Legacy projects already using it"
    ],

    "when_not_to_use": [
      "Production applications (outdated dependencies)",
      "Cross-platform document generation",
      "When users might have LibreOffice/Google Docs",
      "New projects (use modern alternatives)",
      "When you need reliable, tested output",
      "Server-side generation for unknown clients"
    ]
  },

  "recommendations": {
    "for_continued_use": [
      {
        "priority": "HIGH",
        "action": "Audit for security vulnerabilities",
        "details": "Check JSZip 2.7.0 and other dependencies for CVEs"
      },
      {
        "priority": "HIGH",
        "action": "Test with target Word versions",
        "details": "Verify compatibility with Word versions your users have"
      },
      {
        "priority": "MEDIUM",
        "action": "Consider forking and updating",
        "details": "Update to JSZip 3.x, modern build tools, ES6+",
        "effort": "Moderate - JSZip 3.x has breaking changes"
      },
      {
        "priority": "MEDIUM",
        "action": "Add input validation",
        "details": "Validate HTML input before processing"
      },
      {
        "priority": "LOW",
        "action": "Add TypeScript definitions",
        "details": "Create .d.ts file for TypeScript projects"
      }
    ],

    "for_new_projects": [
      {
        "recommendation": "Use alternative library",
        "reason": "This package is unmaintained with outdated dependencies",
        "suggested_alternatives": [
          "docx npm package for programmatic generation",
          "Look for maintained forks on npm",
          "Pandoc for server-side conversion",
          "Custom solution with modern tools"
        ]
      }
    ]
  },

  "test_coverage": {
    "test_files": [
      "test/index.coffee (5374 bytes)",
      "test/sample.html (browser demo)",
      "test/testbed.html (PhantomJS test runner)"
    ],
    "test_types": [
      "Node.js tests (Mocha)",
      "Browser tests (PhantomJS + Mocha)"
    ],
    "commands": {
      "node_tests": "gulp test-node",
      "browser_tests": "gulp test-phantomjs",
      "watch_mode": "gulp test-node-watch"
    },
    "note": "PhantomJS is deprecated - modern alternative would be Puppeteer/Playwright"
  },

  "build_process": {
    "source_language": "CoffeeScript",
    "bundler": "Browserify 4.2.0",
    "task_runner": "Gulp 3.8.5",
    "transforms": [
      "coffeeify - Compiles CoffeeScript to JavaScript",
      "brfs - Inlines fs.readFileSync() calls at build time",
      "jstify - Compiles templates (lodash-micro)"
    ],
    "outputs": {
      "node_build": "build/*.js (compiled CoffeeScript)",
      "browser_bundle": "dist/html-docx.js (browserified standalone)",
      "test_bundle": "build/tests.js (test bundle)"
    },
    "commands": {
      "build": "gulp build",
      "build_node": "gulp build-node",
      "watch": "gulp watch",
      "test": "npm test (runs gulp test-node)",
      "prepublish": "gulp clean; gulp build-node"
    }
  }
}
```

## Detailed Findings

### How It Really Works

The library uses a clever workaround rather than true HTML-to-DOCX conversion:

1. **Creates a minimal DOCX structure** - A DOCX file is just a ZIP archive with specific XML files
2. **Embeds HTML as MHT** - Converts HTML to MHTML (web archive) format
3. **Uses altchunk feature** - Inserts `<w:altChunk>` element in document.xml that references the MHT file
4. **Word does the conversion** - When the file is opened in Word, Word detects the altchunk and converts the MHT/HTML to native WordProcessingML format

**This is why:**
- It only works in Microsoft Word (other apps don't support altchunks)
- The library is so small (just creates the container)
- Conversion quality depends on Word's HTML parser
- Different Word versions may produce different results

### Critical Compatibility Issue

**This library ONLY works with Microsoft Word 2007+ (Windows/Mac).** It does NOT work with:
- ❌ LibreOffice Writer
- ❌ Google Docs
- ❌ Apple Pages
- ❌ Microsoft Word for Mac 2008
- ❌ Any other DOCX reader

If your users might use these applications, **do not use this library**.

### Modernization Needed

The library hasn't been updated since 2016 and uses very outdated dependencies:

| Dependency | Installed | Current | Age |
|------------|-----------|---------|-----|
| JSZip | 2.7.0 | 3.10.1 | ~8 years behind |
| Browserify | 4.2.0 | 17.0.0 | ~9 years behind |
| Gulp | 3.8.5 | 4.0.2 | ~8 years behind |
| Mocha | 1.20.1 | 10.2.0 | ~9 years behind |

**Security Risk:** Old dependencies may have known vulnerabilities.

### Use Cases

**Good for:**
- Internal tools in Microsoft Office environments
- Quick prototypes
- Simple HTML export from rich text editors
- When you specifically need the altchunk approach

**Not good for:**
- Production web applications
- Cross-platform document generation
- Users with LibreOffice/Google Docs
- Modern TypeScript projects
- When you need reliable, tested output

## Recommendations

### If you must use this library:

1. **Audit dependencies** for security vulnerabilities
2. **Test thoroughly** with your target Word versions
3. **Add input validation** for HTML content
4. **Consider forking** and updating to modern dependencies
5. **Warn users** that files only work in Microsoft Word

### For new projects:

**Use a modern alternative instead:**

- **`docx` npm package** - Programmatic DOCX generation with full control
- **Pandoc** - Comprehensive document converter (server-side)
- **Look for maintained forks** - Search npm for updated versions
- **Custom solution** - Build with modern tools if you have specific needs

## Conclusion

`pt-html-docx-js` is a clever library that solves a specific problem using an ingenious workaround. However, it is:

- ✅ Small and simple
- ✅ Works in browser and Node.js
- ❌ **Unmaintained** (8+ years without updates)
- ❌ **Outdated dependencies** (security risk)
- ❌ **Limited compatibility** (Word-only)
- ❌ **Not a true converter** (relies on Word)

**For new projects in 2025, use a modern alternative.** If you have an existing project using this library, consider migrating or at minimum updating the dependencies.
