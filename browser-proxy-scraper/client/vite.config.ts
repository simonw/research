import { defineConfig, type Plugin } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";
import { writeFileSync, mkdirSync } from "fs";
import { join } from "path";

// Custom UV config with correct paths
const UV_CONFIG = `/*global Ultraviolet*/
self.__uv$config = {
  prefix: "/service/",
  encodeUrl: Ultraviolet.codec.xor.encode,
  decodeUrl: Ultraviolet.codec.xor.decode,
  handler: "/uv/uv.handler.js",
  client: "/uv/uv.client.js",
  bundle: "/uv/uv.bundle.js",
  config: "/uv/uv.config.js",
  sw: "/uv/uv.sw.js",
};
`;

// Write our custom UV config after static copy
function uvConfigPlugin(): Plugin {
  return {
    name: "uv-config-override",
    closeBundle() {
      const uvDir = join("dist", "uv");
      mkdirSync(uvDir, { recursive: true });
      writeFileSync(join(uvDir, "uv.config.js"), UV_CONFIG);
    },
    configureServer(server) {
      // Serve our custom config during dev
      server.middlewares.use("/uv/uv.config.js", (_req, res) => {
        res.setHeader("Content-Type", "application/javascript");
        res.end(UV_CONFIG);
      });
    },
  };
}

export default defineConfig({
  plugins: [
    viteStaticCopy({
      targets: [
        {
          src: "node_modules/@titaniumnetwork-dev/ultraviolet/dist/*",
          dest: "uv",
        },
        {
          src: "node_modules/@mercuryworkshop/bare-mux/dist/worker.js",
          dest: "bare-mux",
        },
        {
          src: "node_modules/@mercuryworkshop/bare-mux/dist/worker.js.map",
          dest: "bare-mux",
        },
        {
          src: "node_modules/@mercuryworkshop/epoxy-transport/dist/index.mjs",
          dest: "epoxy",
        },
        {
          src: "node_modules/@mercuryworkshop/epoxy-tls/full/*",
          dest: "epoxy",
        },
      ],
    }),
    uvConfigPlugin(),
  ],
  server: {
    port: 5173,
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
    proxy: {
      "/wisp": {
        target: "ws://localhost:3000",
        ws: true,
      },
    },
  },
  build: {
    target: "esnext",
  },
});
