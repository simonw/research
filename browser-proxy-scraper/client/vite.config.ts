import { defineConfig, type Plugin } from "vite";
import { viteStaticCopy } from "vite-plugin-static-copy";
import { writeFileSync, mkdirSync, readFileSync, existsSync } from "fs";
import { join } from "path";

// UV config — no inject (puppet-agent injection handled in sw.js)
const UV_CONFIG = `/*global Ultraviolet*/
self.__uv$config = {
  prefix: "/service/",
  encodeUrl: Ultraviolet.codec.xor.encode,
  decodeUrl: Ultraviolet.codec.xor.decode,
  handler: "/uv/uv.handler.js",
  client: "/uv/uv.client.js",
  bundle: "/uv/uv.bundle.js",
  config: "/uv/uv.config.js",
  sw: "/uv/uv.sw.js"
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

      // Serve curl-impersonate WASM artifacts during dev
      const curlFiles: Record<string, { path: string; type: string }> = {
        "/curl-impersonate/curl-wasm-fetch.js": {
          path: join(__dirname, "curl-impersonate-wasm/src/curl-wasm-fetch.js"),
          type: "application/javascript",
        },
        "/curl-impersonate/curl-impersonate.js": {
          path: join(__dirname, "curl-impersonate-wasm/dist/curl-impersonate.js"),
          type: "application/javascript",
        },
        "/curl-impersonate/curl-impersonate.wasm": {
          path: join(__dirname, "curl-impersonate-wasm/dist/curl-impersonate.wasm"),
          type: "application/wasm",
        },
      };
      for (const [route, info] of Object.entries(curlFiles)) {
        server.middlewares.use(route, (_req, res) => {
          if (existsSync(info.path)) {
            res.setHeader("Content-Type", info.type);
            res.end(readFileSync(info.path));
          } else {
            res.statusCode = 404;
            res.end("Not found — run link-wasm.sh to build");
          }
        });
      }
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
        // curl-impersonate WASM artifacts (Chrome TLS fingerprint)
        {
          src: "curl-impersonate-wasm/dist/curl-impersonate.js",
          dest: "curl-impersonate",
        },
        {
          src: "curl-impersonate-wasm/dist/curl-impersonate.wasm",
          dest: "curl-impersonate",
        },
        {
          src: "curl-impersonate-wasm/src/curl-wasm-fetch.js",
          dest: "curl-impersonate",
        },
      ],
    }),
    uvConfigPlugin(),
  ],
  server: {
    host: true,
    port: 5173,
    watch: {
      ignored: ["**/curl-impersonate-wasm/emsdk/**", "**/curl-impersonate-wasm/build/**", "**/curl-impersonate-wasm/curl-impersonate-src/**"],
    },
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
