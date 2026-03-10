const dataEntries = document.getElementById("data-entries") as HTMLDivElement;
const dataCount = document.getElementById("data-count") as HTMLSpanElement;

let count = 0;

interface InterceptedMessage {
  type: "INTERCEPTED_DATA";
  url: string;
  timestamp: number;
  payload: unknown;
}

function addEntry(data: InterceptedMessage): void {
  count++;
  dataCount.textContent = `(${count})`;

  const entry = document.createElement("div");
  entry.className = "data-entry";

  const time = new Date(data.timestamp).toLocaleTimeString();
  const payloadStr = JSON.stringify(data.payload, null, 2);
  const truncated = payloadStr.length > 2000 ? payloadStr.slice(0, 2000) + "\n..." : payloadStr;

  entry.innerHTML = `
    <div class="url">${escapeHtml(data.url)}</div>
    <div class="time">${time}</div>
    <pre>${escapeHtml(truncated)}</pre>
  `;

  // Insert at top
  dataEntries.insertBefore(entry, dataEntries.firstChild);
}

function escapeHtml(str: string): string {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

export function setupDataPanel(): void {
  navigator.serviceWorker.addEventListener("message", (event) => {
    if (event.data?.type === "INTERCEPTED_DATA") {
      console.log("[data-panel] intercepted:", event.data.url);
      addEntry(event.data as InterceptedMessage);
    }
  });

  // Also listen for postMessage from puppet agent
  window.addEventListener("message", (event) => {
    if (event.data?.type === "INTERCEPTED_DATA") {
      console.log("[data-panel] intercepted (postMessage):", event.data.url);
      addEntry(event.data as InterceptedMessage);
    }
  });
}
