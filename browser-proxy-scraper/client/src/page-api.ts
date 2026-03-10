import { xorEncode } from "./ui";
import { sendCommand } from "./puppet";

export interface PageApi {
  goto(url: string): Promise<void>;
  evaluate(jsString: string): Promise<unknown>;
  captureNetwork(opts: {
    urlPattern: string;
    bodyPattern?: string;
    key: string;
  }): Promise<void>;
  getCapturedResponse(key: string): Promise<unknown[]>;
  setData(key: string, value: unknown): Promise<void>;
  sleep(ms: number): Promise<void>;
  showBrowser(url?: string): Promise<void>;
  goHeadless(): Promise<void>;
  closeBrowser(): Promise<void>;
}

export interface PageApiCallbacks {
  onData: (key: string, value: unknown) => void;
  onStatus: (message: string) => void;
}

export function createPageApi(
  iframe: HTMLIFrameElement,
  callbacks: PageApiCallbacks
): PageApi {
  function getSW(): ServiceWorker | null {
    return navigator.serviceWorker.controller;
  }

  return {
    async goto(url: string): Promise<void> {
      if (!url.startsWith("http://") && !url.startsWith("https://")) {
        url = "https://" + url;
      }
      const encoded = xorEncode(url);
      iframe.src = `/service/${encoded}`;

      // Wait for the iframe to load
      await new Promise<void>((resolve) => {
        const onLoad = () => {
          iframe.removeEventListener("load", onLoad);
          resolve();
        };
        iframe.addEventListener("load", onLoad);
        // Timeout after 30s
        setTimeout(resolve, 30000);
      });

      // Give the page a moment to initialize scripts
      await new Promise((r) => setTimeout(r, 500));
    },

    async evaluate(jsString: string): Promise<unknown> {
      return sendCommand("evaluate", { code: jsString });
    },

    async captureNetwork(opts): Promise<void> {
      const sw = getSW();
      if (!sw) throw new Error("Service worker not available");

      return new Promise<void>((resolve) => {
        const handler = (event: MessageEvent) => {
          if (
            event.data?.type === "CAPTURE_REGISTERED" &&
            event.data.key === opts.key
          ) {
            navigator.serviceWorker.removeEventListener("message", handler);
            resolve();
          }
        };
        navigator.serviceWorker.addEventListener("message", handler);

        sw.postMessage({
          type: "CAPTURE_REGISTER",
          urlPattern: opts.urlPattern,
          bodyPattern: opts.bodyPattern || null,
          key: opts.key,
        });

        // Timeout fallback
        setTimeout(() => {
          navigator.serviceWorker.removeEventListener("message", handler);
          resolve();
        }, 3000);
      });
    },

    async getCapturedResponse(key: string): Promise<unknown[]> {
      const sw = getSW();
      if (!sw) throw new Error("Service worker not available");

      return new Promise<unknown[]>((resolve) => {
        const handler = (event: MessageEvent) => {
          if (
            event.data?.type === "CAPTURE_RESULT" &&
            event.data.key === key
          ) {
            navigator.serviceWorker.removeEventListener("message", handler);
            resolve(event.data.responses || []);
          }
        };
        navigator.serviceWorker.addEventListener("message", handler);

        sw.postMessage({
          type: "CAPTURE_GET",
          key,
        });

        // Timeout fallback
        setTimeout(() => {
          navigator.serviceWorker.removeEventListener("message", handler);
          resolve([]);
        }, 5000);
      });
    },

    async setData(key: string, value: unknown): Promise<void> {
      callbacks.onData(key, value);
    },

    async sleep(ms: number): Promise<void> {
      return new Promise((r) => setTimeout(r, ms));
    },

    async showBrowser(url?: string): Promise<void> {
      if (url) await this.goto(url);
    },

    async goHeadless(): Promise<void> {
      // No-op in browser context
    },

    async closeBrowser(): Promise<void> {
      iframe.src = "about:blank";
    },
  };
}
