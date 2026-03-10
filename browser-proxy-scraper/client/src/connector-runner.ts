import { createPageApi, type PageApi, type PageApiCallbacks } from "./page-api";

export interface ConnectorResult {
  success: boolean;
  data?: unknown;
  error?: string;
}

export async function runConnector(
  script: string,
  iframe: HTMLIFrameElement,
  callbacks: PageApiCallbacks
): Promise<ConnectorResult> {
  const page = createPageApi(iframe, callbacks);

  try {
    callbacks.onStatus("Running connector...");

    // Wrap the script so `page` is available as a variable
    const wrappedScript = `
      return (async (page) => {
        ${script}
      })(page);
    `;

    const fn = new Function("page", wrappedScript);
    const result = await fn(page);

    callbacks.onStatus("Connector finished");
    return { success: true, data: result };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    callbacks.onStatus(`Connector error: ${message}`);
    console.error("[connector-runner]", err);
    return { success: false, error: message };
  }
}
