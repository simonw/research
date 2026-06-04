#!/usr/bin/env node
/**
 * Multi-dimensional HTML Tool Categorizer
 *
 * Beyond JS APIs, this extracts multiple signals for categorization:
 * 1. UI Framework detection (React, Vue, etc.)
 * 2. HTML semantic elements and ARIA
 * 3. Form elements and input types
 * 4. External dependencies (libraries)
 * 5. Code complexity metrics
 * 6. Data format handling
 * 7. Interaction patterns
 */

import * as cheerio from 'cheerio';
import { readFileSync, readdirSync, writeFileSync } from 'fs';
import { join, basename } from 'path';

// Known frameworks/libraries detected by CDN patterns
const LIBRARY_PATTERNS = {
  // Frameworks
  'react': ['react.production', 'react.development', 'react@', 'react.min'],
  'vue': ['vue@', 'vue.js', 'vue.min', 'vue.global'],
  'angular': ['angular.min', 'angular@'],
  'svelte': ['svelte@', 'svelte/'],
  'preact': ['preact@', 'preact.min'],
  'alpine': ['alpinejs', 'alpine.min'],
  'htmx': ['htmx.org', 'htmx@', 'htmx.min'],
  'jquery': ['jquery.min', 'jquery@', 'jquery-'],

  // Build tools / Transpilers
  'babel': ['babel-standalone', '@babel/'],
  'typescript': ['typescript@'],

  // CSS Frameworks
  'tailwind': ['tailwindcss', 'tailwind.'],
  'bootstrap': ['bootstrap.min', 'bootstrap@'],

  // Visualization
  'd3': ['d3.min', 'd3@', '/d3/'],
  'chart.js': ['chart.js', 'chart@'],
  'plotly': ['plotly.min', 'plotly@'],
  'three.js': ['three.min', 'three@', 'three.module'],
  'leaflet': ['leaflet.js', 'leaflet@', 'leaflet.min'],

  // Utilities
  'lodash': ['lodash.min', 'lodash@'],
  'moment': ['moment.min', 'moment@'],
  'dayjs': ['dayjs.min', 'dayjs@'],
  'luxon': ['luxon.min', 'luxon@'],
  'date-fns': ['date-fns@'],

  // Markdown/Text
  'marked': ['marked@', 'marked.min'],
  'markdown-it': ['markdown-it@'],
  'showdown': ['showdown@', 'showdown.min'],
  'prism': ['prism.min', 'prism@', 'prismjs@'],
  'highlight.js': ['highlight.min', 'highlight@', 'hljs'],

  // UI Components
  'monaco-editor': ['monaco-editor@'],
  'codemirror': ['codemirror@', 'codemirror.min'],
  'ace': ['ace.js', 'ace-builds'],

  // Data/Fetch
  'axios': ['axios.min', 'axios@'],

  // AI/ML
  'tensorflow': ['tensorflow', 'tfjs'],
  'pyodide': ['pyodide@', 'pyodide.js'],
  'tesseract': ['tesseract@', 'tesseract.min'],

  // Compression
  'pako': ['pako@', 'pako.min', 'pako.esm'],
  'jszip': ['jszip@', 'jszip.min'],
  'fflate': ['fflate@'],

  // PDF
  'pdf.js': ['pdf.js', 'pdfjs@'],
  'jspdf': ['jspdf@', 'jspdf.min'],

  // Media
  'howler': ['howler@', 'howler.min'],
  'wavesurfer': ['wavesurfer@', 'wavesurfer.min'],

  // SQL/DB
  'sql.js': ['sql.js', 'sql-wasm'],

  // Crypto
  'crypto-js': ['crypto-js@'],
};

// HTML elements that indicate functionality
const SEMANTIC_ELEMENTS = {
  interactive: ['button', 'input', 'select', 'textarea', 'form', 'details', 'dialog'],
  media: ['audio', 'video', 'canvas', 'img', 'picture', 'svg'],
  structural: ['header', 'footer', 'nav', 'main', 'article', 'section', 'aside'],
  data: ['table', 'thead', 'tbody', 'tr', 'td', 'th', 'dl', 'dt', 'dd'],
  text: ['pre', 'code', 'blockquote', 'figure', 'figcaption'],
};

// Input type categories
const INPUT_TYPES = {
  text: ['text', 'search', 'tel', 'url', 'email', 'password'],
  numeric: ['number', 'range'],
  datetime: ['date', 'month', 'week', 'time', 'datetime-local'],
  selection: ['checkbox', 'radio'],
  file: ['file'],
  color: ['color'],
  special: ['hidden', 'submit', 'reset', 'button', 'image'],
};

function detectLibraries(urls) {
  const detected = new Set();

  for (const url of urls) {
    const lowerUrl = url.toLowerCase();
    for (const [lib, patterns] of Object.entries(LIBRARY_PATTERNS)) {
      if (patterns.some(p => lowerUrl.includes(p))) {
        detected.add(lib);
      }
    }
  }

  return Array.from(detected);
}

function analyzeHtmlStructure($) {
  const elements = {};

  // Count semantic elements
  for (const [category, tags] of Object.entries(SEMANTIC_ELEMENTS)) {
    const counts = {};
    for (const tag of tags) {
      const count = $(tag).length;
      if (count > 0) {
        counts[tag] = count;
      }
    }
    if (Object.keys(counts).length > 0) {
      elements[category] = counts;
    }
  }

  // Analyze input types
  const inputTypes = {};
  $('input').each((_, el) => {
    const type = $(el).attr('type') || 'text';
    inputTypes[type] = (inputTypes[type] || 0) + 1;
  });

  // Categorize inputs
  const inputCategories = {};
  for (const [category, types] of Object.entries(INPUT_TYPES)) {
    const count = types.reduce((sum, t) => sum + (inputTypes[t] || 0), 0);
    if (count > 0) {
      inputCategories[category] = count;
    }
  }

  return { elements, inputTypes, inputCategories };
}

function detectAccessibility($) {
  const a11y = {
    hasAriaLabels: $('[aria-label]').length > 0,
    hasAriaDescriptions: $('[aria-describedby]').length > 0,
    hasAriaRoles: $('[role]').length > 0,
    hasAriaLive: $('[aria-live]').length > 0,
    hasAltText: $('img[alt]').length,
    imagesWithoutAlt: $('img:not([alt])').length,
    hasLangAttr: $('html[lang]').length > 0,
    formLabels: $('label').length,
    formsWithoutLabels: $('input:not([type="hidden"]):not([type="submit"]):not([type="button"])').filter((_, el) => {
      const id = $(el).attr('id');
      return !id || $(`label[for="${id}"]`).length === 0;
    }).length,
  };

  return a11y;
}

function analyzeJavaScriptComplexity($) {
  let totalLines = 0;
  let totalChars = 0;
  let scriptCount = 0;
  let hasAsyncAwait = false;
  let hasClasses = false;
  let hasArrowFunctions = false;
  let hasTemplateStrings = false;
  let hasDestructuring = false;
  let hasSpreadOperator = false;

  $('script').each((_, el) => {
    const code = $(el).text();
    if (!code.trim()) return;

    scriptCount++;
    totalLines += code.split('\n').length;
    totalChars += code.length;

    // Detect modern JS features (heuristic, not AST-based)
    if (/\basync\b|\bawait\b/.test(code)) hasAsyncAwait = true;
    if (/\bclass\s+\w+/.test(code)) hasClasses = true;
    if (/=>\s*[{(]?/.test(code)) hasArrowFunctions = true;
    if (/`[^`]*\$\{/.test(code)) hasTemplateStrings = true;
    if (/(?:const|let|var)\s*\{/.test(code) || /(?:const|let|var)\s*\[/.test(code)) hasDestructuring = true;
    if (/\.\.\.[\w$]/.test(code)) hasSpreadOperator = true;
  });

  return {
    scriptCount,
    totalLines,
    totalChars,
    avgLinesPerScript: scriptCount > 0 ? Math.round(totalLines / scriptCount) : 0,
    modernFeatures: {
      asyncAwait: hasAsyncAwait,
      classes: hasClasses,
      arrowFunctions: hasArrowFunctions,
      templateStrings: hasTemplateStrings,
      destructuring: hasDestructuring,
      spreadOperator: hasSpreadOperator,
    },
  };
}

function detectInteractionPatterns($, code) {
  const patterns = {
    // Event types
    dragAndDrop: /ondrag|ondrop|dragstart|dragend|dragover|dragleave|dragenter/.test(code),
    fileUpload: $('input[type="file"]').length > 0 || /files?\[|FileReader|dataTransfer/.test(code),
    clipboard: /clipboard|execCommand.*copy|navigator\.clipboard/.test(code),
    keyboard: /keydown|keyup|keypress|onkey/.test(code),
    touch: /touchstart|touchend|touchmove|ontouchstart/.test(code),
    scroll: /onscroll|scrollTo|scrollIntoView|IntersectionObserver/.test(code),
    resize: /onresize|ResizeObserver/.test(code),

    // Data patterns
    urlParams: /URLSearchParams|location\.search|window\.location/.test(code),
    hashRouting: /location\.hash|hashchange|window\.hash/.test(code),
    formSubmit: $('form').length > 0 || /submit|FormData/.test(code),

    // Output patterns
    download: /download=|blob.*url|createObjectURL/.test(code),
    print: /window\.print|@media print/.test(code),
    share: /navigator\.share|Web Share/.test(code),

    // Real-time
    polling: /setInterval|setTimeout.*fetch/.test(code),
    websocket: /WebSocket|socket\.on|socket\.emit/.test(code),
    sse: /EventSource/.test(code),
  };

  return patterns;
}

function detectDataFormats($, code) {
  return {
    json: /JSON\.parse|JSON\.stringify|application\/json/.test(code),
    csv: /text\/csv|\.csv|parseCSV|csv2/.test(code),
    xml: /DOMParser|XMLSerializer|text\/xml/.test(code),
    markdown: /marked|markdown|\.md|showdown/.test(code),
    yaml: /js-yaml|yaml\.parse|\.yaml|\.yml/.test(code),
    base64: /btoa|atob|base64/.test(code),
    binary: /ArrayBuffer|Uint8Array|DataView|Blob/.test(code),
    svg: $('svg').length > 0 || /createElementNS.*svg/.test(code),
    pdf: /pdf\.js|jspdf|application\/pdf/.test(code),
  };
}

function inferPurpose(analysis) {
  const purposes = [];

  // Based on APIs
  if (analysis.libraries.includes('chart.js') || analysis.libraries.includes('d3') || analysis.libraries.includes('plotly')) {
    purposes.push('visualization');
  }
  if (analysis.libraries.includes('monaco-editor') || analysis.libraries.includes('codemirror') || analysis.libraries.includes('ace')) {
    purposes.push('code-editor');
  }
  if (analysis.libraries.includes('leaflet')) {
    purposes.push('mapping');
  }
  if (analysis.libraries.includes('three.js')) {
    purposes.push('3d-graphics');
  }
  if (analysis.libraries.includes('tesseract')) {
    purposes.push('ocr');
  }
  if (analysis.libraries.includes('pyodide')) {
    purposes.push('python-runtime');
  }

  // Based on HTML
  if (analysis.structure.elements?.media?.canvas > 0) {
    purposes.push('canvas-based');
  }
  if (analysis.structure.elements?.media?.video > 0 || analysis.structure.elements?.media?.audio > 0) {
    purposes.push('media-player');
  }
  if (analysis.structure.inputCategories?.file > 0 || analysis.interactions.fileUpload) {
    purposes.push('file-processor');
  }

  // Based on interactions
  if (analysis.interactions.clipboard) {
    purposes.push('clipboard-utility');
  }
  if (analysis.interactions.download) {
    purposes.push('generator');
  }
  if (analysis.interactions.websocket || analysis.interactions.sse) {
    purposes.push('realtime');
  }

  // Based on data formats
  if (analysis.dataFormats.json && analysis.dataFormats.csv) {
    purposes.push('data-converter');
  }
  if (analysis.dataFormats.markdown) {
    purposes.push('markdown-tool');
  }

  return purposes.length > 0 ? purposes : ['utility'];
}

function analyzeHtmlFile(filePath) {
  const html = readFileSync(filePath, 'utf-8');
  const $ = cheerio.load(html);

  // Get all script sources (CDN and local)
  const scriptUrls = [];
  $('script[src]').each((_, el) => {
    const src = $(el).attr('src');
    if (src) scriptUrls.push(src);
  });

  // Get link URLs (CSS)
  $('link[href]').each((_, el) => {
    const href = $(el).attr('href');
    if (href) scriptUrls.push(href);
  });

  // Get all inline script code combined
  let allCode = '';
  $('script').each((_, el) => {
    allCode += $(el).text() + '\n';
  });

  const structure = analyzeHtmlStructure($);
  const accessibility = detectAccessibility($);
  const complexity = analyzeJavaScriptComplexity($);
  const libraries = detectLibraries(scriptUrls);
  const interactions = detectInteractionPatterns($, allCode);
  const dataFormats = detectDataFormats($, allCode);

  const analysis = {
    file: basename(filePath),
    libraries,
    structure,
    accessibility,
    complexity,
    interactions,
    dataFormats,
  };

  analysis.inferredPurposes = inferPurpose(analysis);

  return analysis;
}

function main() {
  const toolsDir = process.argv[2] || '/tmp/tools';

  const htmlFiles = readdirSync(toolsDir)
    .filter(f => f.endsWith('.html'))
    .map(f => join(toolsDir, f));

  console.error(`Categorizing ${htmlFiles.length} HTML files...\n`);

  const results = htmlFiles.map(file => analyzeHtmlFile(file));

  // Library usage summary
  const libCounts = {};
  for (const r of results) {
    for (const lib of r.libraries) {
      libCounts[lib] = (libCounts[lib] || 0) + 1;
    }
  }

  console.log('=== Libraries Used ===\n');
  const sortedLibs = Object.entries(libCounts).sort((a, b) => b[1] - a[1]);
  for (const [lib, count] of sortedLibs) {
    console.log(`${lib}: ${count} files`);
  }

  // Purpose inference
  const purposeCounts = {};
  for (const r of results) {
    for (const p of r.inferredPurposes) {
      purposeCounts[p] = (purposeCounts[p] || 0) + 1;
    }
  }

  console.log('\n=== Inferred Purposes ===\n');
  const sortedPurposes = Object.entries(purposeCounts).sort((a, b) => b[1] - a[1]);
  for (const [purpose, count] of sortedPurposes) {
    console.log(`${purpose}: ${count} files`);
  }

  // Interaction patterns
  const interactionCounts = {};
  for (const r of results) {
    for (const [pattern, active] of Object.entries(r.interactions)) {
      if (active) {
        interactionCounts[pattern] = (interactionCounts[pattern] || 0) + 1;
      }
    }
  }

  console.log('\n=== Interaction Patterns ===\n');
  const sortedInteractions = Object.entries(interactionCounts).sort((a, b) => b[1] - a[1]);
  for (const [pattern, count] of sortedInteractions) {
    console.log(`${pattern}: ${count} files`);
  }

  // Data formats
  const formatCounts = {};
  for (const r of results) {
    for (const [fmt, used] of Object.entries(r.dataFormats)) {
      if (used) {
        formatCounts[fmt] = (formatCounts[fmt] || 0) + 1;
      }
    }
  }

  console.log('\n=== Data Formats ===\n');
  const sortedFormats = Object.entries(formatCounts).sort((a, b) => b[1] - a[1]);
  for (const [fmt, count] of sortedFormats) {
    console.log(`${fmt}: ${count} files`);
  }

  // Accessibility summary
  let withAriaLabels = 0;
  let withLangAttr = 0;
  let withAltText = 0;
  for (const r of results) {
    if (r.accessibility.hasAriaLabels || r.accessibility.hasAriaRoles) withAriaLabels++;
    if (r.accessibility.hasLangAttr) withLangAttr++;
    if (r.accessibility.hasAltText > 0) withAltText++;
  }

  console.log('\n=== Accessibility ===\n');
  console.log(`Files with ARIA attributes: ${withAriaLabels}`);
  console.log(`Files with lang attribute: ${withLangAttr}`);
  console.log(`Files with image alt text: ${withAltText}`);

  // Complexity stats
  const complexities = results.map(r => r.complexity.totalLines).filter(x => x > 0);
  console.log('\n=== Code Complexity ===\n');
  console.log(`Total files with inline JS: ${complexities.length}`);
  console.log(`Average lines per file: ${Math.round(complexities.reduce((a, b) => a + b, 0) / complexities.length)}`);
  console.log(`Max lines: ${Math.max(...complexities)}`);
  console.log(`Min lines: ${Math.min(...complexities)}`);

  // Modern JS features
  let withAsync = 0, withClasses = 0, withArrow = 0;
  for (const r of results) {
    if (r.complexity.modernFeatures.asyncAwait) withAsync++;
    if (r.complexity.modernFeatures.classes) withClasses++;
    if (r.complexity.modernFeatures.arrowFunctions) withArrow++;
  }
  console.log(`\nModern JS Features:`);
  console.log(`  async/await: ${withAsync} files`);
  console.log(`  classes: ${withClasses} files`);
  console.log(`  arrow functions: ${withArrow} files`);

  // Write JSON
  const jsonPath = join(process.cwd(), 'categories.json');
  writeFileSync(jsonPath, JSON.stringify(results, null, 2));
  console.error(`\nFull results written to ${jsonPath}`);
}

main();
