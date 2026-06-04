# Exploring the new `servo` crate

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

On 2026-04-13 the Servo team
[shipped][blog] `servo` v0.1.0 to crates.io — the first time the
Servo browser engine has been published as a regular embeddable
library. This folder is my investigation of the crate:

- What is in the public API?
- Can you write a small Rust CLI that takes a URL (or a local HTML
  file) and renders it to a PNG using just this one dependency?
- Can the crate be compiled to WebAssembly, and if so what kind of
  single-page apps would that unlock?

[blog]: https://servo.org/blog/2026/04/13/servo-0.1.0-release/

## TL;DR

- **The crate is real and the API is coherent.** The core flow is
  `ServoBuilder → Servo + RenderingContext → WebViewBuilder → WebView`,
  with a `WebViewDelegate` receiving load/paint/navigation events and
  `Servo::spin_event_loop()` advancing the pipeline. Pixel readback is
  a first-class operation on the rendering context
  (`read_to_image() -> ImageBuffer<Rgba<u8>, Vec<u8>>`), and
  `WebView::take_screenshot()` offers an async variant that also
  waits for fonts / images / render-blocking stylesheets.
- **The headless screenshot CLI works.** See [`servo-shot/`](servo-shot).
  It type-checks and `cargo build --release`s against the real
  `servo = "0.1.0"` crate on stable Rust 1.94, pulls in ~2000 deps
  (including SpiderMonkey), and produces a 153 MB binary that
  renders a URL or local HTML file to a PNG via Servo's software
  rendering context. Sample rendered output committed alongside as
  [`servo-shot/sample.png`](servo-shot/sample.png).
- **Compiling the _whole_ `servo` crate to wasm is not feasible**
  today (SpiderMonkey, threads, GL, fonts, networking — pick any
  blocker). But compiling **Servo sub-crates** to wasm works: the
  [`html5ever-wasm-demo/`](html5ever-wasm-demo) folder contains a
  self-contained SPA that loads Servo's actual `html5ever` +
  `markup5ever_rcdom` crates (`wasm32-unknown-unknown`, 454 KB
  `.wasm`) and shows the parse tree for whatever HTML you type in
  — all running client-side, zero server round-trip. [View that demo](https://simonw.github.io/research/servo-crate-exploration/html5ever-wasm-demo/www/).

## Repo layout

```
servo-crate-exploration/
├── README.md               <- you are here
├── notes.md                <- running notebook of what I tried
├── servo-shot/             <- the native Rust CLI
│   ├── Cargo.toml
│   ├── src/main.rs         <- ~180 lines, fully annotated
│   ├── sample.html         <- demo page to screenshot
│   └── sample.png          <- real rendered output from Servo 0.1.0
└── html5ever-wasm-demo/    <- in-browser SPA using Servo sub-crates
    ├── Cargo.toml
    ├── src/lib.rs          <- wasm-bindgen glue around html5ever
    └── www/
        ├── index.html                     <- the SPA itself
        ├── html5ever_wasm_demo.js         <- JS bindings (wasm-bindgen)
        └── html5ever_wasm_demo_bg.wasm    <- 454 KB of Servo parser
```

## The `servo` v0.1.0 embedding API, in one page

Items live at the root of the `servo` crate unless noted.

| Item | What it's for |
|---|---|
| `Servo` | In-process handle to a running Servo engine. |
| `ServoBuilder` | `.opts(Opts)` / `.preferences(Preferences)` / `.event_loop_waker(...)` / `.protocol_registry(...)` / `.build()`. |
| `WebView` | One browsing context. Cheap `Clone` (just a handle). |
| `WebViewBuilder` | `::new(&Servo, Rc<dyn RenderingContext>)` → `.url(Url)` / `.hidpi_scale_factor(Scale)` / `.delegate(Rc<dyn WebViewDelegate>)` / `.user_content_manager(...)` / `.clipboard_delegate(...)` / `.build()`. |
| `WindowRenderingContext` | surfman-backed, expects a `raw-window-handle` host window. For real apps. |
| `SoftwareRenderingContext::new(PhysicalSize<u32>)` | Pure-software OpenGL. Slower, but works headless on machines without a GPU. This is the one you want for screenshots / CI / servers. |
| `OffscreenRenderingContext` | Child surface hanging off a `WindowRenderingContext`; useful when you want to composite a webview into another scene. |
| `RenderingContext` (trait) | `make_current`, `prepare_for_rendering`, `present`, `resize`, `size`, `gleam_gl_api`, `glow_gl_api`, and the money method: `read_to_image(Box2D<i32, DevicePixel>) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>>`. |
| `WebViewDelegate` (trait) | Notification hooks: `notify_load_status_changed` (the one to wait for when screenshotting — it fires `LoadStatus::Complete` after the `load` event), `notify_new_frame_ready`, `notify_animating_changed`, plus URL/title/favicon/history hooks. |
| `EventLoopWaker` (trait) | Lets Servo ask the embedder to wake its main loop from a background thread. |
| `RefreshDriver` (trait) | Lets the embedder drive frame cadence (e.g. vsync). |
| `ClipboardDelegate` (trait) | Pluggable clipboard. |
| `InputEvent`, `KeyboardEvent`, `MouseButtonEvent`, `TouchEvent`, `WheelEvent`, `CompositionEvent` | Feed synthetic / real input into Servo. |
| `SimpleDialogRequest`, `FilePickerRequest`, `ContextMenuRequest`, `PermissionRequest` | UI requests Servo bubbles up to the embedder to handle. |
| `WebResourceLoad` | Intercept network requests and provide alternate responses (great for offline bundles, testing, or content filters). |
| `ProtocolRegistry` | Register custom URL schemes that the embedder resolves. |
| `Servo::spin_event_loop()` | Drive one pump of the engine: run delegate callbacks, flush paint. You call this in a loop. |

The bones are recognisably "the servo embedding API that servoshell
uses", just cleaned up and narrowed to what a third-party embedder
actually needs.

## `servo-shot`: rendering a URL to a PNG

`servo-shot` takes either a URL or a path to a local HTML file and
writes a PNG screenshot. Usage:

```
servo-shot https://example.com                 # default 1280x800 → shot.png
servo-shot sample.html --out sample.png
servo-shot ./docs/index.html --width 1920 --height 1080 --dpr 2.0
```

### How it works (architecturally)

```
                ┌──────────────────────────────┐
                │  SoftwareRenderingContext    │  ← PhysicalSize from
                │  (no GPU, no X server)       │    --width/--height
                └─────────────┬────────────────┘
                              │ Rc<dyn RenderingContext>
                              ▼
   ServoBuilder::default().build()   →   Servo
                              │
                              ▼
         WebViewBuilder::new(&servo, ctx)
             .url(...).hidpi_scale_factor(...).delegate(...)
             .build()               →   WebView
                              │
            ┌─────────────────┴──────────────────┐
            │ ShotDelegate::notify_load_status_changed
            │  if LoadStatus::Complete → flag.set(true)
            └────────────────────────────────────┘

 loop { servo.spin_event_loop() }  until flag, then settle frames
 ctx.read_to_image(full_rect)  →  ImageBuffer → PNG
```

### Key implementation choices

- **Software rendering context.** The tool is meant to work on
  servers / CI / headless containers, so it picks the
  `SoftwareRenderingContext` variant. If you already have a GPU and
  a window, switching to `WindowRenderingContext` is literally a
  one-line change (plus the winit / raw-window-handle plumbing).
- **`Rc<Cell<bool>>` for the load flag.** Servo's delegate callbacks
  fire on the embedder thread, so we don't need `Arc<AtomicBool>`.
- **`LoadStatus::Complete` + N settle frames.** Matches what
  browsers call the `load` event. A few extra `spin_event_loop()`
  passes afterwards let web fonts / late paints land.
- **`dpi::PhysicalSize<u32>` vs `euclid::Size2D`.** Gotcha during
  development — `SoftwareRenderingContext::new` wants the former
  (same `dpi` crate winit uses). `notes.md` has the full debug log.
- **`surfman::error::Error` doesn't implement `std::error::Error`**,
  so the `?` / `anyhow::Context` dance needs a `map_err`.

### Source

See [`servo-shot/src/main.rs`](servo-shot/src/main.rs). ~150 lines
including comments; no `unsafe`, one dependency (`servo`) doing the
heavy lifting, plus `clap`, `image`, `url`, `anyhow`, `dpi`, `euclid`.

### Build status in this sandbox

- `cargo check` against `servo = "0.1.0"`: **passes** (debug profile).
- `cargo build --release`: **succeeds**. ~2000 dependent crates,
  first clean build takes ~7:49 (incl. SpiderMonkey from source),
  final binary is ~153 MB.
- Running against `sample.html` produced the committed
  [`sample.png`](servo-shot/sample.png). The rendering shows the
  real gradient, rounded card and system-font typography the CSS
  describes — it's Servo's actual renderer, not a simplified
  fallback.

### Gotchas learnt while getting the first non-blank PNG

These took the longest to find and are documented in the source so
you don't have to rediscover them:

1. **The delegate _must_ call `webview.paint()` inside
   `notify_new_frame_ready`.** Without it the compositor never
   actually writes pixels into the `SoftwareRenderingContext`'s
   back buffer and you get an all-white PNG. Servo's own
   `tests/common/mod.rs` does exactly this.
2. **Do NOT call `rendering_context.present()` before readback.**
   For `SoftwareRenderingContext`, `present()` calls
   `swap_buffers(.., PreserveBuffer::No)`, wiping the buffer you
   just painted. The `read_to_image` doc explicitly says you don't
   need `present` first — it reads the internal off-screen buffer.
3. **After `LoadStatus::Complete`, force one extra paint.** The
   compositor doesn't necessarily emit a frame purely on `load`;
   a tiny `requestAnimationFrame` + forced `getBoundingClientRect`
   via `evaluate_javascript` is what the Servo test harness uses to
   guarantee a post-load frame.
4. **Headless Linux boxes need `libegl1` + `$XDG_RUNTIME_DIR`.**
   surfman's software path still goes through libEGL, so a bare
   container needs `apt-get install libegl1` and a writable
   `XDG_RUNTIME_DIR`.
5. **`SoftwareRenderingContext::new` takes `dpi::PhysicalSize<u32>`**,
   not a `euclid::Size2D`. Add `dpi = "0.1"` as a direct dep.
6. **`surfman::error::Error` doesn't implement `std::error::Error`**,
   so you can't `?` it through anyhow without a `.map_err(...)`.
7. **`ServoBuilder::default()` picks up the system HTTP proxy.**
   For local screenshotting, set an empty proxy pref
   (`Preferences::default().network_http(s)_proxy_uri = String::new()`).

### Prereqs on a real machine

Servo's native dependencies are inherited from this crate:

- Rust 1.94+ (stable is fine for 0.1.0, unlike historical Servo).
- `cmake`, `clang`/`llvm` (to build mozjs_sys / SpiderMonkey).
- `pkg-config` + fontconfig, freetype, harfbuzz dev packages.
- On Linux: gstreamer-1.0-dev, libssl-dev, libglib2.0-dev, mesa
  (for osmesa software GL).
- Roughly 10–15 GB free in `target/` for the first release build.

## WebAssembly: feasible parts, infeasible parts

### What is NOT feasible

Compiling the whole `servo` crate to `wasm32-unknown-unknown` or
`wasm32-wasi` doesn't work today. Blockers, in order of severity:

1. **SpiderMonkey** (`mozjs_sys`). Servo embeds Firefox's JS engine,
   which is a huge C++ project with its own JIT. It does not target
   wasm; running a JS engine inside wasm is impractical for a
   browser-shaped workload anyway.
2. **Threads everywhere.** Servo's architecture is unapologetically
   multi-threaded (constellation, one script thread per origin,
   layout thread pool, paint worker, WebRender scene builder,
   network, resource-loader, hang monitor). Browser wasm only
   recently got threads via `SharedArrayBuffer` and it's still
   fragile and COOP/COEP-gated.
3. **OpenGL / WebRender.** Paint goes through surfman + a real GL
   context. Porting would mean bridging to WebGL2/WebGPU, which is
   possible but non-trivial.
4. **Networking.** Servo ships hyper + rustls; wasm needs to route
   through `fetch()` or wasi-sockets. Rewrite territory.
5. **Fonts + file system.** fontconfig / DirectWrite / CoreText do
   not exist in wasm.

Servo maintainer _jdm_ summed it up on the tracker: "It would not
be an easy project."

### What IS feasible (and already works)

Instead of the whole engine, compile **Servo subsystems** to wasm.
These already build cleanly or with tiny patches:

- **Stylo** — the real CSS parser, selector matcher and cascade
  engine. Has a documented small patch (`servo/stylo#78`) to build
  on `wasm32-*`.
- **`html5ever` / `markup5ever`** — Servo's HTML5 parser.
- **`selectors` / `cssparser`** — already wasm-friendly.
- **`url` / `idna`** — already wasm-friendly.

### Built one: `html5ever-wasm-demo/`

To make the theory concrete, [`html5ever-wasm-demo/`](html5ever-wasm-demo)
ships a small SPA that loads two of those Servo sub-crates
(`html5ever` and `markup5ever_rcdom`), compiled to
`wasm32-unknown-unknown`, into the browser. Point it at a static
web server and you get a textarea on the left and two views on the
right:

- **Parse tree** — how Servo's real HTML5 parser actually
  interpreted your markup, with all the implicit `<html>` /
  `<head>` / `<body>` / `<tbody>` insertions and adoption-agency
  fix-ups laid out in a tree dump.
- **Normalised HTML** — the round-tripped serialisation,
  equivalent to running your markup through Servo's sanitiser.

Total shipped payload: one 4 KB HTML, one 8 KB JS glue file, and a
**454 KB `.wasm`** containing the whole parser. Build instructions
in [`html5ever-wasm-demo/Cargo.toml`](html5ever-wasm-demo/Cargo.toml)
and the commentary at the top of
[`src/lib.rs`](html5ever-wasm-demo/src/lib.rs).

To try it: [simonw.github.io/research/servo-crate-exploration/html5ever-wasm-demo/www/](https://simonw.github.io/research/servo-crate-exploration/html5ever-wasm-demo/www/) - or:

```
cd html5ever-wasm-demo
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir www \
    target/wasm32-unknown-unknown/release/html5ever_wasm_demo.wasm
python3 -m http.server -d www 8000   # then open http://localhost:8000/
```

The pre-built `www/` directory is included in this folder, so you can
just `python3 -m http.server -d www 8000` and go.

### Other neat SPAs you could build with the same approach

Given those sub-crates, here are ideas that are "single-page app,
ship one .wasm and one .html" realistic today:

1. **CSS Selector Playground** — paste HTML left, selector right,
   highlight matches with live specificity numbers computed by the
   real Servo selector engine (not a hand-rolled approximation).
2. **Stylo Cascade Explainer** — an educational SPA that shows
   exactly which rules won the cascade for a chosen element and why,
   using Stylo itself as the ground truth. Great for teaching
   `!important`, origin, specificity, rule order.
3. **In-browser HTML Sanitizer / Pretty-printer** — use `html5ever`
   in wasm to parse pasted rich text, normalise it, strip dangerous
   attributes, and round-trip to clean HTML — no server needed.
4. **HTML Tree-Diff Viewer** — compare two HTMLs structurally
   (great for CI review of static-site builds) with `html5ever`
   tree matching.
5. **"Lint my page" SPA** — drop a URL into the SPA; `fetch()`
   pulls the source; wasm parses and runs servo-style conformance
   checks (unclosed tags, quirks-mode triggers, accessibility hints)
   entirely client-side.
6. **React/Canvas UI with real CSS cascade** — use Stylo in wasm as
   the styling engine behind a canvas/WebGL UI (the long-standing
   "draw React components to canvas with real CSS" dream).

If you genuinely need the full Servo renderer on the web, the
practical path today is to run it native and stream frames (WebRTC,
WebCodecs), not to recompile the engine itself.

## Appendix: why the "blog post" link looks sparse

The 2026-04-13 post is a short release announcement: version
number, crates.io link, LTS note, and a pointer to docs.rs. All of
the substantive API information in this report comes from the
[`docs.rs/servo/0.1.0`][docs] pages (I used the 0.1.0-rc2 build
while 0.1.0 itself was re-indexing) and from cross-checking against
the servoshell crate in `servo/servo`.

[docs]: https://docs.rs/servo/0.1.0
