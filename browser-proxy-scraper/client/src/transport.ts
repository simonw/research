import { BareMuxConnection } from "@mercuryworkshop/bare-mux";

let connection: BareMuxConnection | null = null;

// curl-impersonate WASM module and its wasmFetch
let curlModule: any = null;
let wasmFetchFn: ((url: string, options?: any) => Promise<any>) | null = null;

/**
 * Initialize the transport layer.
 *
 * Tries to load curl-impersonate WASM (Chrome TLS fingerprint) first.
 * Falls back to epoxy-transport (Rustls) if WASM loading fails.
 */
export async function initTransport(): Promise<void> {
  const wispUrl = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/wisp/`;

  // Try curl-impersonate WASM first (Chrome TLS fingerprint)
  // Use indirect import to avoid Vite's static analysis (this module is optional)
  try {
    const modulePath = "/curl-impersonate/curl-wasm-fetch.js";
    const curlWasm = await import(/* @vite-ignore */ modulePath);
    await curlWasm.initCurlWasm(wispUrl);
    wasmFetchFn = curlWasm.wasmFetch;
    console.log("[transport] initialized with curl-impersonate WASM (Chrome TLS fingerprint)");
  } catch (e) {
    console.warn("[transport] curl-impersonate WASM not available, falling back to epoxy:", e);
    wasmFetchFn = null;
  }

  // Always set up bare-mux + epoxy as the UV transport layer
  // (UV's service worker requires bare-mux for its fetch pipeline)
  connection = new BareMuxConnection("/bare-mux/worker.js");
  await connection.setTransport("/epoxy/index.mjs", [{ wisp: wispUrl, disable_certificate_validation: true }]);

  console.log("[transport] bare-mux/epoxy initialized with wisp URL:", wispUrl);
}

export function getConnection(): BareMuxConnection | null {
  return connection;
}

/**
 * Get the wasmFetch function if curl-impersonate is loaded.
 * Returns null if only epoxy transport is available.
 */
export function getWasmFetch(): ((url: string, options?: any) => Promise<any>) | null {
  return wasmFetchFn;
}
