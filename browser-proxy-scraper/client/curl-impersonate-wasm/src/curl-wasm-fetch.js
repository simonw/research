/**
 * wasmFetch() — fetch()-like API backed by curl-impersonate WASM.
 *
 * Usage:
 *   import { initCurlWasm, wasmFetch } from './curl-wasm-fetch.js';
 *   await initCurlWasm('wss://example.com/wisp/');
 *   const response = await wasmFetch('https://example.com/api', { method: 'GET', headers: {...} });
 */

let Module = null;

// Non-variadic wrapper functions (avoid va_arg issues across JS→WASM boundary)
let curlSetoptString, curlSetoptLong, curlSetoptPtr, curlSetoptCb;
let curlGetinfoLong;

// Standard curl functions (not variadic)
let curlEasyInit, curlEasyPerform, curlEasyCleanup, curlEasyStrerror, curlImpersonateChrome116;
let curlSlistAppend, curlSlistFreeAll;
let curlGlobalInit, curlGlobalCleanup;
let requestQueue = Promise.resolve();

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
const CURLOPT_SSL_VERIFYPEER = 64;
const CURLOPT_SSL_VERIFYHOST = 81;
const CURLOPT_USERAGENT = 10018;
const CURLOPT_NOSIGNAL = 99;

// curl info constants
const CURLINFO_RESPONSE_CODE = 0x200002;

function debugEnabled() {
  return !!(
    (typeof globalThis !== "undefined" && globalThis.__CURL_WASM_DEBUG) ||
    (Module && Module.curlWasmDebug)
  );
}

function debugLog(...args) {
  if (debugEnabled()) {
    console.log("[curl-wasm]", ...args);
  }
}

/**
 * Initialize the curl-impersonate WASM module and connect to Wisp.
 */
export async function initCurlWasm(wispUrl) {
  if (Module) return;

  const { default: CurlImpersonate } = await import('./curl-impersonate.js');
  Module = await CurlImpersonate({ wispUrl });
  Module.wispBridgeDebug = false;
  Module.curlWasmDebug = false;

  // Non-variadic wrappers (safe for string/pointer options)
  curlSetoptString = Module.cwrap('curl_setopt_string', 'number', ['number', 'number', 'number']);
  curlSetoptLong = Module.cwrap('curl_setopt_long', 'number', ['number', 'number', 'number']);
  curlSetoptPtr = Module.cwrap('curl_setopt_ptr', 'number', ['number', 'number', 'number']);
  curlSetoptCb = Module.cwrap('curl_setopt_cb', 'number', ['number', 'number', 'number']);
  curlGetinfoLong = Module.cwrap('curl_getinfo_long', 'number', ['number', 'number', 'number']);

  // Standard curl functions
  curlGlobalInit = Module.cwrap('curl_global_init', 'number', ['number']);
  curlGlobalCleanup = Module.cwrap('curl_global_cleanup', null, []);
  curlEasyInit = Module.cwrap('curl_easy_init', 'number', []);
  curlEasyPerform = Module.cwrap('curl_easy_perform', 'number', ['number'], { async: true });
  curlEasyCleanup = Module.cwrap('curl_easy_cleanup', null, ['number']);
  curlEasyStrerror = Module.cwrap('curl_easy_strerror', 'string', ['number']);
  curlImpersonateChrome116 = Module.cwrap('curl_impersonate_chrome116', 'number', ['number']);
  curlSlistAppend = Module.cwrap('curl_slist_append', 'number', ['number', 'number']);
  curlSlistFreeAll = Module.cwrap('curl_slist_free_all', null, ['number']);

  curlGlobalInit(0); // CURL_GLOBAL_DEFAULT

  // Pre-connect the Wisp WebSocket (plain JS Promise, no Asyncify)
  // This ensures the WebSocket is ready before any curl requests
  await Module.wispBridgeInit(wispUrl);

  debugLog('Initialized with Wisp URL:', wispUrl);
}

/**
 * fetch()-like function using curl-impersonate WASM.
 * Returns a Response-like object with status, headers, body, text(), json(), arrayBuffer().
 */
function enqueueRequest(task) {
  const run = requestQueue.catch(() => {}).then(task);
  requestQueue = run.catch(() => {});
  return run;
}

async function wasmFetchImpl(url, options = {}) {
  if (!Module) throw new Error('curl-wasm not initialized. Call initCurlWasm() first.');

  const method = (options.method || 'GET').toUpperCase();
  const headers = options.headers || {};
  const body = options.body || null;

  const handle = curlEasyInit();
  if (!handle) throw new Error('curl_easy_init failed');

  let urlPtr = 0;
  let bodyPtr = 0;
  let slist = 0;
  let writeCallback = 0;
  let headerCallback = 0;
  let statusPtr = 0;

  try {
    debugLog('request start', { url, method });

    // Impersonate Chrome 116 TLS fingerprint (sets HTTP version, ciphers, extensions, etc.)
    // Uses C wrapper that skips CURLOPT_SSL_CERT_COMPRESSION (causes WASM indirect call crash)
    curlImpersonateChrome116(handle);

    // Set URL (string option — uses non-variadic wrapper)
    urlPtr = Module.stringToNewUTF8(url);
    curlSetoptString(handle, CURLOPT_URL, urlPtr);

    // Enable libcurl verbose output only during local debugging.
    curlSetoptLong(handle, 41, debugEnabled() ? 1 : 0); // CURLOPT_VERBOSE = 41

    // Disable SSL verification — no CA store in WASM environment
    // TODO: implement CURLOPT_CAINFO_BLOB with embedded CA bundle for production
    curlSetoptLong(handle, CURLOPT_SSL_VERIFYPEER, 0);
    curlSetoptLong(handle, CURLOPT_SSL_VERIFYHOST, 0);
    // Disable libcurl signal/alarm timeouts. In the Emscripten build these go
    // through _setitimer_js -> _emscripten_timeout and can trigger callback ABI
    // mismatches during Asyncify rewind.
    curlSetoptLong(handle, CURLOPT_NOSIGNAL, 1);

    // Redirect handling
    if (options.redirect === 'manual') {
      curlSetoptLong(handle, CURLOPT_FOLLOWLOCATION, 0);
    } else {
      curlSetoptLong(handle, CURLOPT_FOLLOWLOCATION, 1);
      curlSetoptLong(handle, CURLOPT_MAXREDIRS, 10);
    }

    // Set method (string option)
    if (method !== 'GET') {
      const methodPtr = Module.stringToNewUTF8(method);
      curlSetoptString(handle, CURLOPT_CUSTOMREQUEST, methodPtr);
      Module._free(methodPtr);
    }

    // Set headers (pointer option)
    const headerEntries = headers instanceof Headers
      ? Array.from(headers.entries())
      : Object.entries(headers);

    const targetOrigin = globalThis.__TARGET_ORIGIN__;

    for (let [key, value] of headerEntries) {
      if (targetOrigin) {
        const lowerKey = key.toLowerCase();
        if (lowerKey === 'origin') value = targetOrigin;
        if (lowerKey === 'referer' && typeof value === 'string' && value.includes(location.origin)) {
          value = targetOrigin + '/';
        }
      }
      const headerStr = Module.stringToNewUTF8(`${key}: ${value}`);
      slist = curlSlistAppend(slist, headerStr);
      Module._free(headerStr);
    }
    if (slist) {
      curlSetoptPtr(handle, CURLOPT_HTTPHEADER, slist);
    }

    // Set request body (string + long options)
    if (body) {
      const bodyStr = typeof body === 'string' ? body : JSON.stringify(body);
      bodyPtr = Module.stringToNewUTF8(bodyStr);
      curlSetoptString(handle, CURLOPT_POSTFIELDS, bodyPtr);
      curlSetoptLong(handle, CURLOPT_POSTFIELDSIZE, bodyStr.length);
    }

    // Write callback to collect response body (callback/function pointer option)
    const responseChunks = [];
    writeCallback = Module.addFunction(function(ptr, size, nmemb, userdata) {
      const totalSize = size * nmemb;
      const chunk = new Uint8Array(Module.HEAPU8.buffer, ptr, totalSize);
      responseChunks.push(new Uint8Array(chunk));
      debugLog('write callback', { chunkBytes: totalSize, chunks: responseChunks.length });
      return totalSize;
    }, 'iiiii');
    curlSetoptCb(handle, CURLOPT_WRITEFUNCTION, writeCallback);

    // Header callback to collect response headers
    const responseHeaders = [];
    headerCallback = Module.addFunction(function(ptr, size, nmemb, userdata) {
      const totalSize = size * nmemb;
      const line = Module.UTF8ToString(ptr, totalSize).trim();
      if (line.includes(':')) {
        const colonIdx = line.indexOf(':');
        responseHeaders.push([
          line.slice(0, colonIdx).trim().toLowerCase(),
          line.slice(colonIdx + 1).trim()
        ]);
      }
      debugLog('header callback', { line });
      return totalSize;
    }, 'iiiii');
    curlSetoptCb(handle, CURLOPT_HEADERFUNCTION, headerCallback);

    // Perform the request (Asyncify suspends WASM until socket I/O completes)
    debugLog('curl_easy_perform start', { url, method });
    const result = await curlEasyPerform(handle);
    debugLog('curl_easy_perform end', { url, method, result });

    if (result !== 0) {
      const errMsg = curlEasyStrerror(result);
      throw new Error(`curl_easy_perform failed (${result}): ${errMsg}`);
    }

    // Get response status code (via non-variadic wrapper)
    statusPtr = Module._malloc(4);
    curlGetinfoLong(handle, CURLINFO_RESPONSE_CODE, statusPtr);
    const status = Module.getValue(statusPtr, 'i32');
    debugLog('status info', { status, headerCount: responseHeaders.length });

    // Build response body
    const totalLen = responseChunks.reduce((sum, c) => sum + c.length, 0);
    const bodyArray = new Uint8Array(totalLen);
    let offset = 0;
    for (const chunk of responseChunks) {
      bodyArray.set(chunk, offset);
      offset += chunk.length;
    }
    debugLog('response assembled', { status, bodyBytes: totalLen, headerCount: responseHeaders.length });

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
    // Tear down the easy handle before freeing resources it still references.
    curlEasyCleanup(handle);
    if (statusPtr) Module._free(statusPtr);
    if (urlPtr) Module._free(urlPtr);
    if (bodyPtr) Module._free(bodyPtr);
    if (slist) curlSlistFreeAll(slist);
    if (writeCallback) Module.removeFunction(writeCallback);
    if (headerCallback) Module.removeFunction(headerCallback);
  }
}

export async function wasmFetch(url, options = {}) {
  return enqueueRequest(() => wasmFetchImpl(url, options));
}
