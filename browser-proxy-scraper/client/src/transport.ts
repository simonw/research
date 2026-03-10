import { BareMuxConnection } from "@mercuryworkshop/bare-mux";

let connection: BareMuxConnection | null = null;

export async function initTransport(): Promise<void> {
  connection = new BareMuxConnection("/bare-mux/worker.js");

  const wispUrl = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/wisp/`;

  await connection.setTransport("/epoxy/index.mjs", [{ wisp: wispUrl, disable_certificate_validation: true }]);

  console.log("[transport] initialized with wisp URL:", wispUrl);
}

export function getConnection(): BareMuxConnection | null {
  return connection;
}
