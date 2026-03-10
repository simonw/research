const dataEntries = document.getElementById("data-entries") as HTMLDivElement;
const dataCount = document.getElementById("data-count") as HTMLSpanElement;
const clearBtn = document.getElementById("clear-data-btn") as HTMLButtonElement;

let count = 0;

interface InterceptedMessage {
  type: "INTERCEPTED_DATA";
  url: string;
  timestamp: number;
  payload: unknown;
}

interface ConnectorDataMessage {
  type: "CONNECTOR_DATA";
  key: string;
  value: unknown;
  timestamp: number;
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

  dataEntries.insertBefore(entry, dataEntries.firstChild);
}

function addConnectorEntry(data: ConnectorDataMessage): void {
  count++;
  dataCount.textContent = `(${count})`;

  const entry = document.createElement("div");
  entry.className = "data-entry";

  const time = new Date(data.timestamp).toLocaleTimeString();
  const valueStr = JSON.stringify(data.value, null, 2);
  const truncated = valueStr.length > 5000 ? valueStr.slice(0, 5000) + "\n..." : valueStr;

  entry.innerHTML = `
    <div class="data-key">${escapeHtml(data.key)}</div>
    <div class="time">${time}</div>
    <pre>${escapeHtml(truncated)}</pre>
  `;

  dataEntries.insertBefore(entry, dataEntries.firstChild);
}

function escapeHtml(str: string): string {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

export function setupDataPanel(): void {
  // Listen for SW messages (intercepted network responses)
  navigator.serviceWorker.addEventListener("message", (event) => {
    if (event.data?.type === "INTERCEPTED_DATA") {
      console.log("[data-panel] intercepted:", event.data.url);
      addEntry(event.data as InterceptedMessage);
    }
  });

  // Listen for postMessage (from puppet agent or connector runner)
  window.addEventListener("message", (event) => {
    if (event.data?.type === "INTERCEPTED_DATA") {
      console.log("[data-panel] intercepted (postMessage):", event.data.url);
      addEntry(event.data as InterceptedMessage);
    }
    if (event.data?.type === "CONNECTOR_DATA") {
      console.log("[data-panel] connector data:", event.data.key);
      addConnectorEntry(event.data as ConnectorDataMessage);
    }
  });

  // Clear button
  clearBtn.addEventListener("click", () => {
    dataEntries.innerHTML = "";
    count = 0;
    dataCount.textContent = "(0)";
  });
}
