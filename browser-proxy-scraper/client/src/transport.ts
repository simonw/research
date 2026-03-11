import { BareMuxConnection } from "@mercuryworkshop/bare-mux";
import type { BareTransport, BareHeaders, TransferrableResponse } from "@mercuryworkshop/bare-mux";

let connection: BareMuxConnection | null = null;
let wasmFetchFn: ((url: string, options?: any) => Promise<any>) | null = null;

/**
 * Create a BareTransport adapter that wraps curl-impersonate WASM's wasmFetch.
 * This routes all HTTP requests through curl-impersonate (Chrome TLS fingerprint)
 * and WebSocket connections through Wisp directly.
 */
function createCurlTransport(
  fetchFn: (url: string, options?: any) => Promise<any>,
  wispUrl: string,
): BareTransport {
  return {
    ready: true,

    async init() {
      // Already initialized during module load
    },

    meta() {
      return {};
    },

    async request(
      remote: URL,
      method: string,
      body: BodyInit | null,
      headers: BareHeaders,
      signal: AbortSignal | undefined,
    ): Promise<TransferrableResponse> {
      // Normalize headers to plain object
      const headersObj: Record<string, string> = {};
      if (headers && typeof headers === "object") {
        if (typeof (headers as any)[Symbol.iterator] === "function") {
          for (const [key, value] of headers as any) {
            headersObj[key] = Array.isArray(value) ? value.join(", ") : value;
          }
        } else {
          for (const [key, value] of Object.entries(headers)) {
            headersObj[key] = Array.isArray(value) ? value.join(", ") : value;
          }
        }
      }

      // Convert body if needed
      let bodyStr: string | null = null;
      if (body) {
        if (body instanceof ArrayBuffer) {
          bodyStr = new TextDecoder().decode(body);
        } else if (body instanceof Blob) {
          bodyStr = new TextDecoder().decode(await body.arrayBuffer());
        } else if (typeof body === "string") {
          bodyStr = body;
        } else if (body instanceof ReadableStream) {
          const response = new Response(body);
          bodyStr = await response.text();
        }
      }

      const res = await fetchFn(remote.href, {
        method,
        headers: headersObj,
        body: bodyStr,
        redirect: "manual",
      });

      // Convert response headers to BareHeaders format
      const responseHeaders: BareHeaders = {};
      if (res.headers) {
        if (res.headers instanceof Headers) {
          res.headers.forEach((value: string, key: string) => {
            responseHeaders[key] = value;
          });
        } else if (typeof res.headers === "object") {
          for (const [key, value] of Object.entries(res.headers)) {
            responseHeaders[key] = value as string;
          }
        }
      }

      // Convert body to ArrayBuffer (transferable)
      let responseBody: ArrayBuffer;
      if (res.body instanceof Uint8Array) {
        responseBody = res.body.buffer.slice(
          res.body.byteOffset,
          res.body.byteOffset + res.body.byteLength,
        );
      } else if (res.body instanceof ArrayBuffer) {
        responseBody = res.body;
      } else {
        const text = typeof res.body === "string" ? res.body : "";
        responseBody = new TextEncoder().encode(text).buffer;
      }

      return {
        body: responseBody,
        headers: responseHeaders,
        status: res.status,
        statusText: res.statusText || "",
      };
    },

    connect(
      url: URL,
      protocols: string[],
      requestHeaders: BareHeaders,
      onopen: (protocol: string) => void,
      onmessage: (data: Blob | ArrayBuffer | string) => void,
      onclose: (code: number, reason: string) => void,
      onerror: (error: string) => void,
    ): [(data: Blob | ArrayBuffer | string) => void, (code: number, reason: string) => void] {
      // WebSocket connections use Wisp directly (curl doesn't handle WS)
      // Open a Wisp TCP stream and speak WebSocket protocol over it
      // For now, fall back to a direct WebSocket through the Wisp relay
      const ws = new WebSocket(wispUrl);
      ws.binaryType = "arraybuffer";

      // We need to create a Wisp stream for this WebSocket target
      // This is a simplified approach: connect via a second Wisp WebSocket
      // that handles the WS upgrade internally
      const targetWsUrl = url.href;

      // Use a bare WebSocket approach: connect to target via Wisp
      // The Wisp relay already handles WebSocket upgrades
      let wispStreamId = 0;
      const nextStreamId = Math.floor(Math.random() * 0x7fffffff) + 1;

      ws.onopen = () => {
        // Send Wisp CONNECT packet for the target host
        const host = url.hostname;
        const port = parseInt(url.port) || (url.protocol === "wss:" ? 443 : 80);
        const hostBytes = new TextEncoder().encode(host);
        const packet = new Uint8Array(5 + 1 + 2 + hostBytes.length);
        packet[0] = 0x01; // CONNECT
        packet[1] = nextStreamId & 0xff;
        packet[2] = (nextStreamId >> 8) & 0xff;
        packet[3] = (nextStreamId >> 16) & 0xff;
        packet[4] = (nextStreamId >> 24) & 0xff;
        packet[5] = 0x01; // TCP
        packet[6] = port & 0xff;
        packet[7] = (port >> 8) & 0xff;
        packet.set(hostBytes, 8);
        ws.send(packet.buffer);
        wispStreamId = nextStreamId;

        // Report open after connect sent
        setTimeout(() => onopen(""), 100);
      };

      ws.onmessage = (event) => {
        const data = new Uint8Array(event.data as ArrayBuffer);
        if (data.length < 5) return;
        const packetType = data[0];
        const streamId =
          data[1] | (data[2] << 8) | (data[3] << 16) | (data[4] << 24);
        if (streamId !== wispStreamId) return;
        const payload = data.slice(5);

        if (packetType === 0x02) {
          // DATA
          onmessage(payload.buffer);
        } else if (packetType === 0x04) {
          // CLOSE
          onclose(1000, "Remote closed");
        }
      };

      ws.onerror = () => onerror("WebSocket error");
      ws.onclose = () => onclose(1000, "Connection closed");

      // Return send and close functions
      const sendFn = (data: Blob | ArrayBuffer | string) => {
        if (ws.readyState !== WebSocket.OPEN) return;
        let payload: Uint8Array;
        if (data instanceof ArrayBuffer) {
          payload = new Uint8Array(data);
        } else if (data instanceof Blob) {
          // Async conversion, best effort
          data.arrayBuffer().then((buf) => {
            const p = new Uint8Array(buf);
            const packet = new Uint8Array(5 + p.length);
            packet[0] = 0x02;
            packet[1] = wispStreamId & 0xff;
            packet[2] = (wispStreamId >> 8) & 0xff;
            packet[3] = (wispStreamId >> 16) & 0xff;
            packet[4] = (wispStreamId >> 24) & 0xff;
            packet.set(p, 5);
            ws.send(packet.buffer);
          });
          return;
        } else {
          payload = new TextEncoder().encode(data);
        }
        const packet = new Uint8Array(5 + payload.length);
        packet[0] = 0x02;
        packet[1] = wispStreamId & 0xff;
        packet[2] = (wispStreamId >> 8) & 0xff;
        packet[3] = (wispStreamId >> 16) & 0xff;
        packet[4] = (wispStreamId >> 24) & 0xff;
        packet.set(payload, 5);
        ws.send(packet.buffer);
      };

      const closeFn = (code: number, reason: string) => {
        if (ws.readyState === WebSocket.OPEN) {
          // Send Wisp CLOSE
          const packet = new Uint8Array(6);
          packet[0] = 0x04;
          packet[1] = wispStreamId & 0xff;
          packet[2] = (wispStreamId >> 8) & 0xff;
          packet[3] = (wispStreamId >> 16) & 0xff;
          packet[4] = (wispStreamId >> 24) & 0xff;
          packet[5] = 0x02;
          ws.send(packet.buffer);
        }
        ws.close();
      };

      return [sendFn, closeFn];
    },
  };
}

/**
 * Initialize the transport layer.
 *
 * Tries curl-impersonate WASM first (Chrome TLS fingerprint via setRemoteTransport).
 * Falls back to epoxy-transport (Rustls) if WASM loading fails.
 */
export async function initTransport(): Promise<void> {
  const wispUrl = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/wisp/`;

  connection = new BareMuxConnection("/bare-mux/worker.js");

  // Try curl-impersonate WASM first (Chrome TLS fingerprint)
  try {
    const modulePath = "/curl-impersonate/curl-wasm-fetch.js";
    const curlWasm = await import(/* @vite-ignore */ modulePath);
    await curlWasm.initCurlWasm(wispUrl);
    wasmFetchFn = curlWasm.wasmFetch;
    console.log("[transport] curl-impersonate WASM loaded");

    // Create a BareTransport adapter and register it via setRemoteTransport
    // This bridges the SharedWorker ↔ main page via MessageChannel
    const transport = createCurlTransport(curlWasm.wasmFetch, wispUrl);
    await connection.setRemoteTransport(transport, "curl-impersonate");
    console.log(
      "[transport] registered curl-impersonate as bare-mux transport (Chrome TLS fingerprint)",
    );
  } catch (e) {
    console.warn(
      "[transport] curl-impersonate WASM not available, falling back to epoxy:",
      e,
    );
    wasmFetchFn = null;
    // Fall back to epoxy-transport (Rustls TLS fingerprint)
    await connection.setTransport("/epoxy/index.mjs", [
      { wisp: wispUrl, disable_certificate_validation: true },
    ]);
    console.log("[transport] bare-mux/epoxy initialized (fallback)");
  }
}

export function getConnection(): BareMuxConnection | null {
  return connection;
}

export function getWasmFetch(): ((url: string, options?: any) => Promise<any>) | null {
  return wasmFetchFn;
}
