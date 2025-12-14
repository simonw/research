#!/usr/bin/env node
/**
 * JS API Tagger using Cheerio (HTML) + Acorn (JavaScript)
 *
 * This approach:
 * 1. Uses cheerio to parse HTML and extract <script> tag contents
 * 2. Uses acorn to parse JavaScript into an AST
 * 3. Walks the AST to find API usage (MemberExpressions, CallExpressions, NewExpressions)
 * 4. Extracts CDN URLs from <script src="..."> and ES module imports
 */

import * as cheerio from 'cheerio';
import * as acorn from 'acorn';
import * as walk from 'acorn-walk';
import { readFileSync, readdirSync } from 'fs';
import { join, basename } from 'path';

// Known Web APIs to detect (using Object.create(null) to avoid prototype pollution)
const KNOWN_APIS = Object.assign(Object.create(null), {
  // Storage APIs
  'localStorage': 'Web Storage',
  'sessionStorage': 'Web Storage',
  'indexedDB': 'IndexedDB',

  // Audio/Media APIs
  'AudioContext': 'Web Audio',
  'webkitAudioContext': 'Web Audio',
  'MediaRecorder': 'MediaRecorder',
  'getUserMedia': 'getUserMedia',
  'mediaDevices': 'MediaDevices',

  // Canvas/Graphics
  'getContext': 'Canvas',
  'CanvasRenderingContext2D': 'Canvas',
  'WebGLRenderingContext': 'WebGL',

  // Network APIs
  'fetch': 'Fetch API',
  'XMLHttpRequest': 'XMLHttpRequest',
  'WebSocket': 'WebSocket',
  'EventSource': 'Server-Sent Events',

  // Workers
  'Worker': 'Web Workers',
  'SharedWorker': 'Shared Workers',
  'ServiceWorker': 'Service Workers',

  // Location/Navigation
  'geolocation': 'Geolocation',
  'history': 'History API',
  'pushState': 'History API',
  'replaceState': 'History API',

  // Clipboard
  'clipboard': 'Clipboard API',
  'ClipboardItem': 'Clipboard API',

  // File APIs
  'FileReader': 'File API',
  'Blob': 'Blob API',
  'URL.createObjectURL': 'URL API',
  'createObjectURL': 'URL API',

  // DOM APIs
  'MutationObserver': 'MutationObserver',
  'IntersectionObserver': 'IntersectionObserver',
  'ResizeObserver': 'ResizeObserver',

  // Drag and Drop
  'dataTransfer': 'Drag and Drop',
  'setDragImage': 'Drag and Drop',

  // Notifications
  'Notification': 'Notifications',

  // Performance
  'performance': 'Performance API',
  'PerformanceObserver': 'Performance API',

  // Crypto
  'crypto': 'Web Crypto',
  'subtle': 'Web Crypto',

  // Speech
  'SpeechRecognition': 'Speech Recognition',
  'webkitSpeechRecognition': 'Speech Recognition',
  'SpeechSynthesis': 'Speech Synthesis',
  'speechSynthesis': 'Speech Synthesis',

  // Animation
  'requestAnimationFrame': 'requestAnimationFrame',
  'Animation': 'Web Animations',

  // Fullscreen
  'requestFullscreen': 'Fullscreen API',
  'exitFullscreen': 'Fullscreen API',

  // Share
  'share': 'Web Share API',

  // Broadcast Channel
  'BroadcastChannel': 'Broadcast Channel',

  // WebRTC
  'RTCPeerConnection': 'WebRTC',
  'RTCDataChannel': 'WebRTC',

  // Payment
  'PaymentRequest': 'Payment Request',

  // Screen
  'screenX': 'Screen API',
  'screenY': 'Screen API',
  'screen': 'Screen API',

  // Pointer Events
  'PointerEvent': 'Pointer Events',
  'setPointerCapture': 'Pointer Events',
});

function extractAPIsFromAST(ast, sourceCode) {
  const apis = new Set();
  const cdnImports = new Set();

  walk.simple(ast, {
    // Handle ES module imports: import x from 'https://...'
    ImportDeclaration(node) {
      const src = node.source.value;
      if (src.startsWith('http://') || src.startsWith('https://')) {
        cdnImports.add(src);
      }
    },

    // Handle dynamic imports: import('https://...')
    ImportExpression(node) {
      if (node.source.type === 'Literal' && typeof node.source.value === 'string') {
        const src = node.source.value;
        if (src.startsWith('http://') || src.startsWith('https://')) {
          cdnImports.add(src);
        }
      }
    },

    // Handle: new AudioContext(), new WebSocket(), etc.
    NewExpression(node) {
      if (node.callee.type === 'Identifier') {
        const name = node.callee.name;
        if (KNOWN_APIS[name]) {
          apis.add(KNOWN_APIS[name]);
        }
      }
      // Handle: new (window.AudioContext || window.webkitAudioContext)()
      if (node.callee.type === 'MemberExpression') {
        const prop = node.callee.property?.name || node.callee.property?.value;
        if (KNOWN_APIS[prop]) {
          apis.add(KNOWN_APIS[prop]);
        }
      }
    },

    // Handle member expressions: navigator.mediaDevices, localStorage.setItem
    MemberExpression(node) {
      // Get the property name (only if it's a string identifier, not computed)
      let propName = null;
      if (node.property?.type === 'Identifier') {
        propName = node.property.name;
      } else if (node.property?.type === 'Literal' && typeof node.property.value === 'string') {
        propName = node.property.value;
      }

      if (propName && typeof propName === 'string' && KNOWN_APIS[propName]) {
        apis.add(KNOWN_APIS[propName]);
      }

      // Check the object too for things like localStorage, indexedDB
      if (node.object.type === 'Identifier') {
        const objName = node.object.name;
        if (objName && typeof objName === 'string' && KNOWN_APIS[objName]) {
          apis.add(KNOWN_APIS[objName]);
        }
      }
    },

    // Handle direct function calls: fetch(), requestAnimationFrame()
    CallExpression(node) {
      if (node.callee.type === 'Identifier') {
        const name = node.callee.name;
        if (KNOWN_APIS[name]) {
          apis.add(KNOWN_APIS[name]);
        }
      }
    }
  });

  return { apis: Array.from(apis), cdnImports: Array.from(cdnImports) };
}

function parseJavaScript(code, isModule = false) {
  try {
    return acorn.parse(code, {
      ecmaVersion: 'latest',
      sourceType: isModule ? 'module' : 'script',
      allowHashBang: true,
      allowAwaitOutsideFunction: true,
      allowImportExportEverywhere: true,
    });
  } catch (e) {
    // If module parse fails, try as script
    if (isModule) {
      try {
        return acorn.parse(code, {
          ecmaVersion: 'latest',
          sourceType: 'script',
        });
      } catch (e2) {
        return null;
      }
    }
    return null;
  }
}

function analyzeHtmlFile(filePath) {
  const html = readFileSync(filePath, 'utf-8');
  const $ = cheerio.load(html);

  const allApis = new Set();
  const allCdnUrls = new Set();
  const parseErrors = [];

  // Extract CDN URLs from <script src="...">
  $('script[src]').each((_, el) => {
    const src = $(el).attr('src');
    if (src && (src.startsWith('http://') || src.startsWith('https://'))) {
      allCdnUrls.add(src);
    }
  });

  // Extract CDN URLs from <link href="..."> (for CSS, but interesting to track)
  $('link[href]').each((_, el) => {
    const href = $(el).attr('href');
    if (href && (href.startsWith('http://') || href.startsWith('https://'))) {
      allCdnUrls.add(href);
    }
  });

  // Process inline <script> tags
  $('script').each((_, el) => {
    const scriptEl = $(el);
    const src = scriptEl.attr('src');

    // Skip external scripts (we've already captured their URL)
    if (src) return;

    const code = scriptEl.text().trim();
    if (!code) return;

    const isModule = scriptEl.attr('type') === 'module';
    const ast = parseJavaScript(code, isModule);

    if (ast) {
      const { apis, cdnImports } = extractAPIsFromAST(ast, code);
      apis.forEach(api => allApis.add(api));
      cdnImports.forEach(url => allCdnUrls.add(url));
    } else {
      parseErrors.push('Failed to parse inline script');
    }
  });

  return {
    file: basename(filePath),
    apis: Array.from(allApis).sort(),
    cdnUrls: Array.from(allCdnUrls).sort(),
    parseErrors,
  };
}

function main() {
  const toolsDir = '/tmp/tools';
  const htmlFiles = readdirSync(toolsDir)
    .filter(f => f.endsWith('.html'))
    .map(f => join(toolsDir, f));

  console.log(`Analyzing ${htmlFiles.length} HTML files...\n`);

  const results = [];

  for (const file of htmlFiles) {
    const result = analyzeHtmlFile(file);
    results.push(result);
  }

  // Summary by API
  const apiCounts = {};
  const cdnCounts = {};

  for (const r of results) {
    for (const api of r.apis) {
      apiCounts[api] = (apiCounts[api] || 0) + 1;
    }
    for (const url of r.cdnUrls) {
      // Extract domain from URL
      try {
        const domain = new URL(url).hostname;
        cdnCounts[domain] = (cdnCounts[domain] || 0) + 1;
      } catch {}
    }
  }

  console.log('=== API Usage Summary ===\n');
  const sortedApis = Object.entries(apiCounts).sort((a, b) => b[1] - a[1]);
  for (const [api, count] of sortedApis) {
    console.log(`${api}: ${count} files`);
  }

  console.log('\n=== CDN Domain Summary ===\n');
  const sortedCdns = Object.entries(cdnCounts).sort((a, b) => b[1] - a[1]);
  for (const [domain, count] of sortedCdns) {
    console.log(`${domain}: ${count} files`);
  }

  // Output detailed JSON
  console.log('\n=== Sample Detailed Results ===\n');
  const interesting = results.filter(r => r.apis.length > 0).slice(0, 5);
  for (const r of interesting) {
    console.log(`${r.file}:`);
    console.log(`  APIs: ${r.apis.join(', ')}`);
    if (r.cdnUrls.length > 0) {
      console.log(`  CDNs: ${r.cdnUrls.slice(0, 3).join(', ')}${r.cdnUrls.length > 3 ? '...' : ''}`);
    }
    console.log();
  }

  // Count parse errors
  const filesWithErrors = results.filter(r => r.parseErrors.length > 0).length;
  console.log(`\nParse errors in ${filesWithErrors} files`);

  return results;
}

main();
