#!/usr/bin/env node
/**
 * JS API Tagger - Analyzes HTML files to detect JavaScript API usage
 *
 * Uses:
 * - Cheerio for HTML parsing
 * - Acorn for JavaScript AST parsing
 * - Walks AST to detect Web API usage
 * - Extracts CDN URLs from script tags and ES imports
 */

import * as cheerio from 'cheerio';
import * as acorn from 'acorn';
import * as walk from 'acorn-walk';
import { readFileSync, readdirSync, writeFileSync } from 'fs';
import { join, basename } from 'path';

// Web APIs organized by category
const API_CATEGORIES = Object.assign(Object.create(null), {
  // === Storage ===
  'localStorage': 'Storage:localStorage',
  'sessionStorage': 'Storage:sessionStorage',
  'indexedDB': 'Storage:IndexedDB',
  'openDatabase': 'Storage:WebSQL',

  // === Audio/Video ===
  'AudioContext': 'Media:WebAudio',
  'webkitAudioContext': 'Media:WebAudio',
  'MediaRecorder': 'Media:MediaRecorder',
  'getUserMedia': 'Media:getUserMedia',
  'mediaDevices': 'Media:MediaDevices',
  'HTMLMediaElement': 'Media:HTMLMediaElement',
  'MediaSource': 'Media:MediaSource',
  'MediaStream': 'Media:MediaStream',

  // === Canvas/Graphics ===
  'getContext': 'Graphics:Canvas',
  'CanvasRenderingContext2D': 'Graphics:Canvas2D',
  'WebGLRenderingContext': 'Graphics:WebGL',
  'WebGL2RenderingContext': 'Graphics:WebGL2',
  'OffscreenCanvas': 'Graphics:OffscreenCanvas',
  'ImageData': 'Graphics:ImageData',
  'createImageBitmap': 'Graphics:ImageBitmap',

  // === Network ===
  'fetch': 'Network:Fetch',
  'XMLHttpRequest': 'Network:XMLHttpRequest',
  'WebSocket': 'Network:WebSocket',
  'EventSource': 'Network:ServerSentEvents',
  'Beacon': 'Network:Beacon',
  'sendBeacon': 'Network:Beacon',

  // === Workers ===
  'Worker': 'Workers:WebWorker',
  'SharedWorker': 'Workers:SharedWorker',
  'ServiceWorker': 'Workers:ServiceWorker',

  // === Location/Navigation ===
  'geolocation': 'Location:Geolocation',
  'getCurrentPosition': 'Location:Geolocation',
  'watchPosition': 'Location:Geolocation',
  'pushState': 'Navigation:History',
  'replaceState': 'Navigation:History',

  // === Clipboard ===
  'clipboard': 'Clipboard:Clipboard',
  'ClipboardItem': 'Clipboard:Clipboard',
  'writeText': 'Clipboard:Clipboard',
  'readText': 'Clipboard:Clipboard',
  'execCommand': 'Clipboard:execCommand',

  // === File APIs ===
  'FileReader': 'File:FileReader',
  'FileList': 'File:FileList',
  'createObjectURL': 'File:ObjectURL',
  'revokeObjectURL': 'File:ObjectURL',
  'Blob': 'File:Blob',
  'File': 'File:File',
  'showOpenFilePicker': 'File:FileSystemAccess',
  'showSaveFilePicker': 'File:FileSystemAccess',
  'showDirectoryPicker': 'File:FileSystemAccess',

  // === DOM Observers ===
  'MutationObserver': 'Observer:MutationObserver',
  'IntersectionObserver': 'Observer:IntersectionObserver',
  'ResizeObserver': 'Observer:ResizeObserver',
  'PerformanceObserver': 'Observer:PerformanceObserver',
  'ReportingObserver': 'Observer:ReportingObserver',

  // === Drag and Drop ===
  'dataTransfer': 'DragDrop:DataTransfer',
  'setDragImage': 'DragDrop:DragImage',
  'dropEffect': 'DragDrop:DropEffect',

  // === Notifications ===
  'Notification': 'Notification:Notification',
  'requestPermission': 'Notification:Permission',

  // === Performance ===
  'performance': 'Performance:Performance',
  'now': 'Performance:PerformanceNow',
  'mark': 'Performance:PerformanceMark',
  'measure': 'Performance:PerformanceMeasure',
  'getEntries': 'Performance:PerformanceEntries',

  // === Crypto ===
  'crypto': 'Crypto:WebCrypto',
  'subtle': 'Crypto:SubtleCrypto',
  'getRandomValues': 'Crypto:RandomValues',
  'randomUUID': 'Crypto:RandomUUID',

  // === Speech ===
  'SpeechRecognition': 'Speech:Recognition',
  'webkitSpeechRecognition': 'Speech:Recognition',
  'SpeechSynthesis': 'Speech:Synthesis',
  'speechSynthesis': 'Speech:Synthesis',
  'SpeechSynthesisUtterance': 'Speech:Synthesis',

  // === Animation ===
  'requestAnimationFrame': 'Animation:RAF',
  'cancelAnimationFrame': 'Animation:RAF',
  'animate': 'Animation:WebAnimations',
  'Animation': 'Animation:WebAnimations',
  'KeyframeEffect': 'Animation:WebAnimations',

  // === Fullscreen ===
  'requestFullscreen': 'Fullscreen:Fullscreen',
  'exitFullscreen': 'Fullscreen:Fullscreen',
  'fullscreenElement': 'Fullscreen:Fullscreen',

  // === Share ===
  'share': 'Share:WebShare',
  'canShare': 'Share:WebShare',

  // === Broadcast Channel ===
  'BroadcastChannel': 'Messaging:BroadcastChannel',
  'MessageChannel': 'Messaging:MessageChannel',
  'postMessage': 'Messaging:PostMessage',

  // === WebRTC ===
  'RTCPeerConnection': 'WebRTC:PeerConnection',
  'RTCDataChannel': 'WebRTC:DataChannel',
  'RTCSessionDescription': 'WebRTC:SessionDescription',

  // === Payment ===
  'PaymentRequest': 'Payment:PaymentRequest',

  // === Pointer/Touch ===
  'PointerEvent': 'Input:PointerEvents',
  'setPointerCapture': 'Input:PointerCapture',
  'TouchEvent': 'Input:TouchEvents',

  // === Gamepad ===
  'Gamepad': 'Input:Gamepad',
  'getGamepads': 'Input:Gamepad',

  // === Vibration ===
  'vibrate': 'Device:Vibration',

  // === Battery ===
  'getBattery': 'Device:Battery',

  // === Screen/Display ===
  'matchMedia': 'Display:MediaQuery',
  'devicePixelRatio': 'Display:DevicePixelRatio',
  'screenOrientation': 'Display:ScreenOrientation',

  // === History ===
  'history': 'Navigation:History',
  'popstate': 'Navigation:History',

  // === URL ===
  'URL': 'URL:URL',
  'URLSearchParams': 'URL:URLSearchParams',

  // === Selection ===
  'getSelection': 'Selection:Selection',
  'Selection': 'Selection:Selection',

  // === Console (for debugging detection) ===
  'console': 'Debug:Console',

  // === JSON ===
  'JSON': 'Data:JSON',

  // === FormData ===
  'FormData': 'Data:FormData',

  // === Encoding ===
  'TextEncoder': 'Encoding:TextEncoder',
  'TextDecoder': 'Encoding:TextDecoder',
  'atob': 'Encoding:Base64',
  'btoa': 'Encoding:Base64',

  // === Compression ===
  'CompressionStream': 'Compression:Streams',
  'DecompressionStream': 'Compression:Streams',

  // === Intl ===
  'Intl': 'Intl:Internationalization',
  'DateTimeFormat': 'Intl:DateTimeFormat',
  'NumberFormat': 'Intl:NumberFormat',
});

// Script types we should NOT parse as JavaScript
const NON_JS_SCRIPT_TYPES = new Set([
  'importmap',
  'application/json',
  'application/ld+json',
  'text/template',
  'text/x-template',
  'text/html',
  'text/perl',
  'text/python',
  'text/x-handlebars-template',
  'text/x-jsrender',
]);

// Script types that are JSX/Babel (we'll try to parse but may fail)
const BABEL_SCRIPT_TYPES = new Set([
  'text/babel',
  'text/jsx',
]);

function extractAPIsFromAST(ast) {
  const apis = new Set();
  const cdnImports = new Set();

  walk.simple(ast, {
    ImportDeclaration(node) {
      const src = node.source.value;
      if (src.startsWith('http://') || src.startsWith('https://')) {
        cdnImports.add(src);
      }
    },

    ImportExpression(node) {
      if (node.source.type === 'Literal' && typeof node.source.value === 'string') {
        const src = node.source.value;
        if (src.startsWith('http://') || src.startsWith('https://')) {
          cdnImports.add(src);
        }
      }
    },

    NewExpression(node) {
      if (node.callee.type === 'Identifier') {
        const name = node.callee.name;
        if (API_CATEGORIES[name]) {
          apis.add(API_CATEGORIES[name]);
        }
      }
      if (node.callee.type === 'MemberExpression') {
        const prop = node.callee.property?.name;
        if (prop && API_CATEGORIES[prop]) {
          apis.add(API_CATEGORIES[prop]);
        }
      }
    },

    MemberExpression(node) {
      let propName = null;
      if (node.property?.type === 'Identifier') {
        propName = node.property.name;
      } else if (node.property?.type === 'Literal' && typeof node.property.value === 'string') {
        propName = node.property.value;
      }

      if (propName && API_CATEGORIES[propName]) {
        apis.add(API_CATEGORIES[propName]);
      }

      if (node.object.type === 'Identifier') {
        const objName = node.object.name;
        if (objName && API_CATEGORIES[objName]) {
          apis.add(API_CATEGORIES[objName]);
        }
      }
    },

    CallExpression(node) {
      if (node.callee.type === 'Identifier') {
        const name = node.callee.name;
        if (API_CATEGORIES[name]) {
          apis.add(API_CATEGORIES[name]);
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
  const scriptSources = [];
  const cssUrls = [];
  const parseErrors = [];
  const skippedScripts = [];

  // Extract external script sources
  $('script[src]').each((_, el) => {
    const src = $(el).attr('src');
    if (src) {
      scriptSources.push(src);
      if (src.startsWith('http://') || src.startsWith('https://')) {
        allCdnUrls.add(src);
      }
    }
  });

  // Extract CSS URLs
  $('link[rel="stylesheet"][href]').each((_, el) => {
    const href = $(el).attr('href');
    if (href) {
      cssUrls.push(href);
      if (href.startsWith('http://') || href.startsWith('https://')) {
        allCdnUrls.add(href);
      }
    }
  });

  // Process inline scripts
  $('script').each((_, el) => {
    const scriptEl = $(el);
    const src = scriptEl.attr('src');
    if (src) return;

    const scriptType = scriptEl.attr('type') || '';
    const code = scriptEl.text().trim();
    if (!code) return;

    // Skip non-JS script types
    if (NON_JS_SCRIPT_TYPES.has(scriptType)) {
      skippedScripts.push({ type: scriptType, reason: 'non-js type' });
      return;
    }

    // Note Babel/JSX scripts (we try but may fail)
    const isBabel = BABEL_SCRIPT_TYPES.has(scriptType);

    const isModule = scriptType === 'module';
    const ast = parseJavaScript(code, isModule);

    if (ast) {
      const { apis, cdnImports } = extractAPIsFromAST(ast);
      apis.forEach(api => allApis.add(api));
      cdnImports.forEach(url => allCdnUrls.add(url));
    } else {
      parseErrors.push({
        type: scriptType || 'default',
        isBabel,
        snippet: code.substring(0, 50).replace(/\n/g, ' ')
      });
    }
  });

  // Organize APIs by category
  const apisByCategory = {};
  for (const api of allApis) {
    const [category, name] = api.split(':');
    if (!apisByCategory[category]) {
      apisByCategory[category] = [];
    }
    apisByCategory[category].push(name);
  }

  // Extract CDN domains
  const cdnDomains = new Set();
  for (const url of allCdnUrls) {
    try {
      cdnDomains.add(new URL(url).hostname);
    } catch {}
  }

  return {
    file: basename(filePath),
    path: filePath,
    apis: Array.from(allApis).sort(),
    apisByCategory,
    cdnUrls: Array.from(allCdnUrls).sort(),
    cdnDomains: Array.from(cdnDomains).sort(),
    scriptSources: scriptSources.filter(s => !s.startsWith('http')),
    parseErrors,
    skippedScripts,
  };
}

function main() {
  const toolsDir = process.argv[2] || '/tmp/tools';
  const outputFormat = process.argv[3] || 'summary'; // summary, json, or detailed

  const htmlFiles = readdirSync(toolsDir)
    .filter(f => f.endsWith('.html'))
    .map(f => join(toolsDir, f));

  console.error(`Analyzing ${htmlFiles.length} HTML files from ${toolsDir}...\n`);

  const results = htmlFiles.map(file => analyzeHtmlFile(file));

  if (outputFormat === 'json') {
    console.log(JSON.stringify(results, null, 2));
    return;
  }

  // Aggregate statistics
  const apiCounts = {};
  const categoryCounts = {};
  const cdnDomainCounts = {};

  for (const r of results) {
    for (const api of r.apis) {
      apiCounts[api] = (apiCounts[api] || 0) + 1;
      const category = api.split(':')[0];
      categoryCounts[category] = (categoryCounts[category] || 0) + 1;
    }
    for (const domain of r.cdnDomains) {
      cdnDomainCounts[domain] = (cdnDomainCounts[domain] || 0) + 1;
    }
  }

  console.log('=== API Categories Summary ===\n');
  const sortedCategories = Object.entries(categoryCounts).sort((a, b) => b[1] - a[1]);
  for (const [cat, count] of sortedCategories) {
    console.log(`${cat}: ${count} files`);
  }

  console.log('\n=== Detailed API Usage ===\n');
  const sortedApis = Object.entries(apiCounts).sort((a, b) => b[1] - a[1]);
  for (const [api, count] of sortedApis) {
    console.log(`${api}: ${count} files`);
  }

  console.log('\n=== CDN Domains ===\n');
  const sortedDomains = Object.entries(cdnDomainCounts).sort((a, b) => b[1] - a[1]);
  for (const [domain, count] of sortedDomains) {
    console.log(`${domain}: ${count} files`);
  }

  // Files with most APIs
  console.log('\n=== Files with Most API Usage ===\n');
  const byApiCount = [...results].sort((a, b) => b.apis.length - a.apis.length).slice(0, 10);
  for (const r of byApiCount) {
    console.log(`${r.file}: ${r.apis.length} APIs`);
    if (outputFormat === 'detailed') {
      console.log(`  ${r.apis.join(', ')}`);
    }
  }

  // Parse errors
  const filesWithErrors = results.filter(r => r.parseErrors.length > 0);
  if (filesWithErrors.length > 0) {
    console.log(`\n=== Files with Parse Errors (${filesWithErrors.length}) ===\n`);
    for (const r of filesWithErrors) {
      console.log(`${r.file}: ${r.parseErrors.map(e => e.type + (e.isBabel ? ' (JSX)' : '')).join(', ')}`);
    }
  }

  // Write JSON output file
  const jsonPath = join(process.cwd(), 'results.json');
  writeFileSync(jsonPath, JSON.stringify(results, null, 2));
  console.error(`\nFull results written to ${jsonPath}`);
}

main();
