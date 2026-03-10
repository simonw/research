const urlInput = document.getElementById("url-input") as HTMLInputElement;
const goBtn = document.getElementById("go-btn") as HTMLButtonElement;
const proxyFrame = document.getElementById("proxy-frame") as HTMLIFrameElement;
const statusEl = document.getElementById("status") as HTMLDivElement;

/**
 * XOR encode a URL for UV's codec (matches Ultraviolet.codec.xor.encode).
 * XORs each character code with 2.
 */
function xorEncode(input: string): string {
  return encodeURIComponent(
    input
      .split("")
      .map((char, i) => (i % 1 === 0 ? String.fromCharCode(char.charCodeAt(0) ^ 2) : char))
      .join("")
  );
}

export function setStatus(message: string, type: "info" | "ready" | "error" = "info"): void {
  statusEl.textContent = message;
  statusEl.className = type;
}

export function enableUI(): void {
  urlInput.disabled = false;
  goBtn.disabled = false;
  setStatus("Ready — enter a URL to proxy", "ready");
}

function navigateToUrl(url: string): void {
  // Ensure URL has protocol
  if (!url.startsWith("http://") && !url.startsWith("https://")) {
    url = "https://" + url;
  }

  const encoded = xorEncode(url);
  const proxiedPath = `/service/${encoded}`;

  console.log("[ui] navigating to:", url);
  console.log("[ui] proxied path:", proxiedPath);

  proxyFrame.src = proxiedPath;
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
}
