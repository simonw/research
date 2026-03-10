import http from "node:http";
import { config } from "./config.js";
import { configureWisp, handleUpgrade } from "./wisp-server.js";

configureWisp();

const server = http.createServer((req, res) => {
  if (req.url === "/health") {
    res.writeHead(200, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ status: "ok", uptime: process.uptime() }));
    return;
  }

  res.writeHead(404);
  res.end("Not found");
});

server.on("upgrade", (request, socket, head) => {
  const url = new URL(request.url || "/", `http://${request.headers.host}`);

  if (url.pathname.startsWith("/wisp")) {
    handleUpgrade(request, socket, head);
  } else {
    socket.destroy();
  }
});

server.listen(config.port, () => {
  console.log(`[relay] listening on http://localhost:${config.port}`);
  console.log(`[relay] wisp endpoint: ws://localhost:${config.port}/wisp/`);
  console.log(`[relay] health check: http://localhost:${config.port}/health`);
});
