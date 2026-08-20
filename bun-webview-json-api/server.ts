// Bun.WebView JSON API — a minimal shot-scraper-like HTTP service.
//
// Requires Bun >= 1.4 (Bun.WebView). On Linux a Chrome/Chromium binary is
// required (Bun.WebView drives it over the DevTools Protocol); on macOS the
// system WebKit backend is used automatically and no browser install is needed.
//
//   BUN_CHROME_PATH=/path/to/chrome CHROME_EXTRA_ARGS="--no-sandbox" bun server.ts
//
// Endpoints:
//   GET  /            usage
//   GET  /healthz     liveness check (also verifies the browser works)
//   POST /javascript  {url, javascript, wait_ms?} -> {"ok":true,"result":<json>}
//   POST /screenshot  {url, width?, height?, format?, quality?, javascript?, wait_ms?}
//                     -> image bytes (or {"ok":true,"b64":...} with "b64":true)

const PORT = Number(process.env.PORT ?? 8044);

// Extra Chrome flags, e.g. "--no-sandbox --proxy-server=... --ssl-version-max=tls1.2"
const extraArgs = (process.env.CHROME_EXTRA_ARGS ?? "").split(/\s+/).filter(Boolean);

function makeView(width = 1280, height = 800) {
  const backend =
    process.platform === "darwin" && !process.env.BUN_CHROME_PATH
      ? undefined // system WebKit
      : ({
          type: "chrome",
          ...(process.env.BUN_CHROME_PATH ? { path: process.env.BUN_CHROME_PATH } : {}),
          argv: extraArgs,
        } as const);
  return new Bun.WebView({ backend, width, height } as any);
}

interface JsBody {
  url: string;
  javascript?: string;
  wait_ms?: number;
  width?: number;
  height?: number;
  // screenshot only:
  format?: "png" | "jpeg" | "webp";
  quality?: number;
  b64?: boolean;
}

const jsonHeaders = { "Content-Type": "application/json" };
const err = (status: number, message: string) =>
  Response.json({ ok: false, error: message }, { status });

async function withView<T>(
  body: JsBody,
  fn: (view: InstanceType<typeof Bun.WebView>) => Promise<T>,
): Promise<T> {
  const view = makeView(body.width ?? 1280, body.height ?? 800);
  try {
    await view.navigate(body.url);
    if (body.wait_ms) await Bun.sleep(body.wait_ms);
    return await fn(view);
  } finally {
    view.close();
  }
}

const server = Bun.serve({
  port: PORT,
  idleTimeout: 120,
  routes: {
    "/": () =>
      Response.json({
        service: "bun-webview-json-api",
        endpoints: {
          "POST /javascript": "{url, javascript, wait_ms?, width?, height?}",
          "POST /screenshot":
            "{url, width?, height?, format?, quality?, javascript?, wait_ms?, b64?}",
          "GET /healthz": "liveness",
        },
      }),

    "/healthz": async () => {
      const view = makeView(320, 240);
      try {
        await view.navigate("about:blank");
        const two = await view.evaluate("1 + 1");
        return Response.json({ ok: two === 2 });
      } finally {
        view.close();
      }
    },

    "/javascript": {
      POST: async (req) => {
        let body: JsBody;
        try {
          body = await req.json();
        } catch {
          return err(400, "invalid JSON body");
        }
        if (!body.url) return err(400, "missing url");
        if (!body.javascript) return err(400, "missing javascript");
        try {
          const result = await withView(body, (view) => view.evaluate(body.javascript!));
          // evaluate() awaits promises and JSON-serializes, like shot-scraper
          return Response.json({ ok: true, result });
        } catch (e: any) {
          return err(502, e?.message ?? String(e));
        }
      },
    },

    "/screenshot": {
      POST: async (req) => {
        let body: JsBody;
        try {
          body = await req.json();
        } catch {
          return err(400, "invalid JSON body");
        }
        if (!body.url) return err(400, "missing url");
        const format = body.format ?? "png";
        try {
          const blob = await withView(body, async (view) => {
            if (body.javascript) await view.evaluate(body.javascript);
            return view.screenshot(
              format === "png" ? { format } : { format, quality: body.quality ?? 80 },
            );
          });
          if (body.b64) {
            const b64 = Buffer.from(await blob.arrayBuffer()).toString("base64");
            return Response.json({ ok: true, content_type: blob.type, b64 });
          }
          return new Response(blob, { headers: { "Content-Type": blob.type } });
        } catch (e: any) {
          return err(502, e?.message ?? String(e));
        }
      },
    },
  },
});

console.log(`bun-webview-json-api listening on http://localhost:${server.port}`);
