import { runConnector } from "./connector-runner";

const urlInput = document.getElementById("url-input") as HTMLInputElement;
const goBtn = document.getElementById("go-btn") as HTMLButtonElement;
const proxyFrame = document.getElementById("proxy-frame") as HTMLIFrameElement;
const statusEl = document.getElementById("status") as HTMLDivElement;
const connectorSelect = document.getElementById("connector-select") as HTMLSelectElement;
const runBtn = document.getElementById("run-btn") as HTMLButtonElement;
const interceptAllCheckbox = document.getElementById("intercept-all-json") as HTMLInputElement;

/**
 * XOR encode a URL for UV's codec (matches Ultraviolet.codec.xor.encode).
 * XORs odd-indexed characters' char codes with 2, leaves even-indexed unchanged.
 */
export function xorEncode(input: string): string {
  let result = "";
  for (let i = 0; i < input.length; i++) {
    result += i % 2 ? String.fromCharCode(input.charCodeAt(i) ^ 2) : input[i];
  }
  return encodeURIComponent(result);
}

export function setStatus(message: string, type: "info" | "ready" | "error" = "info"): void {
  statusEl.textContent = message;
  statusEl.className = type;
}

export function enableUI(): void {
  urlInput.disabled = false;
  goBtn.disabled = false;
  connectorSelect.disabled = false;
  runBtn.disabled = false;
  setStatus("Ready — enter a URL or select a connector", "ready");
}

function navigateToUrl(url: string): void {
  if (!url.startsWith("http://") && !url.startsWith("https://")) {
    url = "https://" + url;
  }

  try {
    const parsed = new URL(url);
    (globalThis as any).__TARGET_ORIGIN__ = parsed.origin;
  } catch {}

  const encoded = xorEncode(url);
  const proxiedPath = `/service/${encoded}`;

  console.log("[ui] navigating to:", url);
  console.log("[ui] proxied path:", proxiedPath);

  proxyFrame.src = proxiedPath;
}

let connectorRunning = false;

async function runSelectedConnector(): Promise<void> {
  if (connectorRunning) return;

  const selected = connectorSelect.value;
  if (!selected) {
    setStatus("Select a connector first", "error");
    return;
  }

  let script: string;
  try {
    const resp = await fetch(selected);
    script = await resp.text();
  } catch (err) {
    setStatus(`Failed to load connector: ${err}`, "error");
    return;
  }

  connectorRunning = true;
  runBtn.textContent = "Running...";
  runBtn.disabled = true;

  try {
    const result = await runConnector(script, proxyFrame, {
      onData: (key, value) => {
        console.log("[connector] setData:", key, value);
        if (key === "status") {
          setStatus(String(value), "info");
        } else {
          // Dispatch as intercepted data for the data panel
          window.dispatchEvent(
            new MessageEvent("message", {
              data: {
                type: "CONNECTOR_DATA",
                key,
                value,
                timestamp: Date.now(),
              },
            })
          );
        }
      },
      onStatus: (msg) => setStatus(msg, "info"),
    });

    if (result.success) {
      setStatus("Connector completed successfully", "ready");
    } else {
      setStatus(`Connector failed: ${result.error}`, "error");
    }
  } finally {
    connectorRunning = false;
    runBtn.textContent = "Run";
    runBtn.disabled = false;
  }
}

export function setupUI(): void {
  goBtn.addEventListener("click", () => {
    const url = urlInput.value.trim();
    if (url) navigateToUrl(url);
  });

  urlInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      const url = urlInput.value.trim();
      if (url) navigateToUrl(url);
    }
  });

  runBtn.addEventListener("click", () => {
    runSelectedConnector();
  });

  // Intercept all JSON toggle
  interceptAllCheckbox.addEventListener("change", () => {
    const sw = navigator.serviceWorker.controller;
    if (sw) {
      sw.postMessage({
        type: "SET_INTERCEPT_ALL_JSON",
        enabled: interceptAllCheckbox.checked,
      });
    }
    console.log("[ui] intercept all JSON:", interceptAllCheckbox.checked);
  });
}
