/**
 * wasmFetch() — fetch()-like API backed by curl-impersonate WASM.
 *
 * Usage:
 *   import { initCurlWasm, wasmFetch } from './curl-wasm-fetch.js';
 *   await initCurlWasm('wss://example.com/wisp/');
 *   const response = await wasmFetch('https://example.com/api', { method: 'GET', headers: {...} });
 */

let Module = null;
let curlEasyInit, curlEasySetopt, curlEasyPerform, curlEasyCleanup;
let curlEasyGetinfo, curlEasyStrerror;
let curlSlistAppend, curlSlistFreeAll;
let curlGlobalInit, curlGlobalCleanup;
let wispBridgeInit;

// curl option constants
const CURLOPT_URL = 10002;
const CURLOPT_HTTPHEADER = 10023;
const CURLOPT_CUSTOMREQUEST = 10036;
const CURLOPT_POSTFIELDS = 10015;
const CURLOPT_POSTFIELDSIZE = 60;
const CURLOPT_WRITEFUNCTION = 20011;
const CURLOPT_HEADERFUNCTION = 20079;
const CURLOPT_FOLLOWLOCATION = 52;
const CURLOPT_MAXREDIRS = 68;
const CURLOPT_HTTP_VERSION = 84;
const CURLOPT_SSL_VERIFYPEER = 64;
const CURLOPT_SSL_VERIFYHOST = 81;
const CURLOPT_USERAGENT = 10018;

// curl info constants
const CURLINFO_RESPONSE_CODE = 0x200002;

// curl HTTP version
const CURL_HTTP_VERSION_2_0 = 3;

/**
 * Initialize the curl-impersonate WASM module and connect to Wisp.
 */
export async function initCurlWasm(wispUrl) {
  if (Module) return;

  // Load the Emscripten-generated ES module (built with EXPORT_ES6=1)
  // Pass the Wisp URL via Module config so the socket bridge can auto-connect
  const { default: CurlImpersonate } = await import('./curl-impersonate.js');
  Module = await CurlImpersonate({ wispUrl });

  // Get function wrappers for exported C functions
  curlGlobalInit = Module.cwrap('curl_global_init', 'number', ['number']);
  curlGlobalCleanup = Module.cwrap('curl_global_cleanup', null, []);
  curlEasyInit = Module.cwrap('curl_easy_init', 'number', []);
  curlEasySetopt = Module.cwrap('curl_easy_setopt', 'number', ['number', 'number', 'number']);
  curlEasyPerform = Module.cwrap('curl_easy_perform', 'number', ['number']);
  curlEasyCleanup = Module.cwrap('curl_easy_cleanup', null, ['number']);
  curlEasyGetinfo = Module.cwrap('curl_easy_getinfo', 'number', ['number', 'number', 'number']);
  curlEasyStrerror = Module.cwrap('curl_easy_strerror', 'string', ['number']);
  curlSlistAppend = Module.cwrap('curl_slist_append', 'number', ['number', 'number']);
  curlSlistFreeAll = Module.cwrap('curl_slist_free_all', null, ['number']);

  // Initialize libcurl
  curlGlobalInit(0); // CURL_GLOBAL_DEFAULT

  console.log('[curl-wasm] Initialized with Wisp URL:', wispUrl);
}

/**
 * fetch()-like function using curl-impersonate WASM.
 * Returns a Response-like object with status, headers, body, text(), json(), arrayBuffer().
 */
export async function wasmFetch(url, options = {}) {
  if (!Module) throw new Error('curl-wasm not initialized. Call initCurlWasm() first.');

  const method = (options.method || 'GET').toUpperCase();
  const headers = options.headers || {};
  const body = options.body || null;

  const handle = curlEasyInit();
  if (!handle) throw new Error('curl_easy_init failed');

  try {
    // Set URL
    const urlPtr = Module.stringToNewUTF8(url);
    curlEasySetopt(handle, CURLOPT_URL, urlPtr);

    // Force HTTP/2
    curlEasySetopt(handle, CURLOPT_HTTP_VERSION, CURL_HTTP_VERSION_2_0);

    // Follow redirects (up to 10)
    curlEasySetopt(handle, CURLOPT_FOLLOWLOCATION, 1);
    curlEasySetopt(handle, CURLOPT_MAXREDIRS, 10);

    // Set method
    if (method !== 'GET') {
      const methodPtr = Module.stringToNewUTF8(method);
      curlEasySetopt(handle, CURLOPT_CUSTOMREQUEST, methodPtr);
      Module._free(methodPtr);
    }

    // Set headers
    let slist = 0;
    const headerEntries = headers instanceof Headers
      ? Array.from(headers.entries())
      : Object.entries(headers);

    for (const [key, value] of headerEntries) {
      const headerStr = Module.stringToNewUTF8(`${key}: ${value}`);
      slist = curlSlistAppend(slist, headerStr);
      Module._free(headerStr);
    }
    if (slist) {
      curlEasySetopt(handle, CURLOPT_HTTPHEADER, slist);
    }

    // Set request body
    let bodyPtr = 0;
    if (body) {
      const bodyStr = typeof body === 'string' ? body : JSON.stringify(body);
      bodyPtr = Module.stringToNewUTF8(bodyStr);
      curlEasySetopt(handle, CURLOPT_POSTFIELDS, bodyPtr);
      curlEasySetopt(handle, CURLOPT_POSTFIELDSIZE, bodyStr.length);
    }

    // Set up write callback to collect response body
    const responseChunks = [];
    const writeCallback = Module.addFunction(function(ptr, size, nmemb, userdata) {
      const totalSize = size * nmemb;
      const chunk = new Uint8Array(Module.HEAPU8.buffer, ptr, totalSize);
      responseChunks.push(new Uint8Array(chunk)); // Copy
      return totalSize;
    }, 'iiiii');
    curlEasySetopt(handle, CURLOPT_WRITEFUNCTION, writeCallback);

    // Set up header callback to collect response headers
    const responseHeaders = [];
    const headerCallback = Module.addFunction(function(ptr, size, nmemb, userdata) {
      const totalSize = size * nmemb;
      const line = Module.UTF8ToString(ptr, totalSize).trim();
      if (line.includes(':')) {
        const colonIdx = line.indexOf(':');
        responseHeaders.push([
          line.slice(0, colonIdx).trim().toLowerCase(),
          line.slice(colonIdx + 1).trim()
        ]);
      }
      return totalSize;
    }, 'iiiii');
    curlEasySetopt(handle, CURLOPT_HEADERFUNCTION, headerCallback);

    // Perform the request (Asyncify suspends until complete)
    const result = curlEasyPerform(handle);

    if (result !== 0) {
      const errMsg = curlEasyStrerror(result);
      throw new Error(`curl_easy_perform failed (${result}): ${errMsg}`);
    }

    // Get response status code
    const statusPtr = Module._malloc(4);
    curlEasyGetinfo(handle, CURLINFO_RESPONSE_CODE, statusPtr);
    const status = Module.getValue(statusPtr, 'i32');
    Module._free(statusPtr);

    // Build response body
    const totalLen = responseChunks.reduce((sum, c) => sum + c.length, 0);
    const bodyArray = new Uint8Array(totalLen);
    let offset = 0;
    for (const chunk of responseChunks) {
      bodyArray.set(chunk, offset);
      offset += chunk.length;
    }

    // Clean up
    Module._free(urlPtr);
    if (bodyPtr) Module._free(bodyPtr);
    if (slist) curlSlistFreeAll(slist);
    Module.removeFunction(writeCallback);
    Module.removeFunction(headerCallback);

    // Build Response-like object
    const headersObj = new Headers(responseHeaders);
    return {
      ok: status >= 200 && status < 300,
      status,
      statusText: '',
      headers: headersObj,
      url,
      body: bodyArray,
      async text() { return new TextDecoder().decode(bodyArray); },
      async json() { return JSON.parse(new TextDecoder().decode(bodyArray)); },
      async arrayBuffer() { return bodyArray.buffer; },
      clone() {
        return {
          ok: status >= 200 && status < 300,
          status,
          statusText: '',
          headers: new Headers(responseHeaders),
          url,
          body: new Uint8Array(bodyArray),
          text: async () => new TextDecoder().decode(bodyArray),
          json: async () => JSON.parse(new TextDecoder().decode(bodyArray)),
          arrayBuffer: async () => bodyArray.buffer.slice(0),
        };
      },
    };
  } finally {
    curlEasyCleanup(handle);
  }
}
