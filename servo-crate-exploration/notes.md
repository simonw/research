# Servo crate exploration notes

Date: 2026-04-13
Branch: claude/servo-crate-exploration-g064l

## Goal

1. Read https://servo.org/blog/2026/04/13/servo-0.1.0-release/
2. Explore the API of the new `servo` crate.
3. If feasible, write a Rust CLI tool that takes a URL or a local HTML
   file path and renders an image of the page using the `servo` crate.
4. Investigate compiling the `servo` crate to WebAssembly and what kind
   of single-page-app demos that would unlock.

## What the blog post actually says

The 2026-04-13 blog post just announces that Servo 0.1.0 has been
published to crates.io. Concretely:

- Version: `servo` v0.1.0 on crates.io (the previous "0.0.x" tags were
  only servoshell binaries; 0.1.0 is the first real library release).
- It points at https://docs.rs/Servo for the embedding API.
- It warns that breaking changes are expected in monthly releases and
  that an LTS line is now offered for embedders that want stability.
- It explicitly does NOT cover dependencies, examples, or wasm support.

So the real source of truth for the API is docs.rs (currently the
docs build for 0.1.0 final is in progress; 0.1.0-rc2 has working docs
that are what 0.1.0 shipped with).

## Key surface area on docs.rs

Top-level items in `servo` (from docs.rs/servo/0.1.0-rc2):

- `Servo` – in-process handle to a running Servo instance.
- `ServoBuilder` – builder for `Servo`.
  - `.opts(Opts)`
  - `.preferences(Preferences)`
  - `.event_loop_waker(Box<dyn EventLoopWaker>)`
  - `.protocol_registry(ProtocolRegistry)`
  - `.build() -> Servo`
- `WebView` – one browsing context, cheaply cloneable handle.
- `WebViewBuilder`:
  - `WebViewBuilder::new(&Servo, Rc<dyn RenderingContext>)`
  - `.delegate(Rc<dyn WebViewDelegate>)`
  - `.url(Url)`
  - `.hidpi_scale_factor(Scale<f32, ...>)`
  - `.user_content_manager(...)`
  - `.clipboard_delegate(...)`
  - `.build() -> WebView`
- Three rendering contexts (all implement `RenderingContext`):
  - `WindowRenderingContext` – surfman-backed, needs raw-window-handle.
  - `SoftwareRenderingContext::new(PhysicalSize<u32>)` – software
    OpenGL, slower but works headless on machines without a GPU.
  - `OffscreenRenderingContext` – child of a `WindowRenderingContext`.
- `RenderingContext` trait: `make_current`, `prepare_for_rendering`,
  `present`, `resize`, `size`, `gleam_gl_api`, `glow_gl_api`, and
  crucially `read_to_image(Box2D<i32, DevicePixel>) -> Option<ImageBuffer<Rgba<u8>, Vec<u8>>>`
  – this is what lets us snapshot a page to a PNG.
- Input event types (`InputEvent`, `KeyboardEvent`, `MouseButtonEvent`, …).
- Dialog/permission/file-picker/context-menu request types.
- `WebViewDelegate` trait with notification hooks:
  - `notify_load_status_changed` (LoadStatus changes – this is how we
    detect "page is done loading").
  - `notify_new_frame_ready` (need to present a frame).
  - `notify_animating_changed` (must keep spinning while animating).
  - URL/title/favicon/history changes.
- `EventLoopWaker`, `RefreshDriver`, `ClipboardDelegate` traits.
- `Servo` instance methods: `spin_event_loop()` (drives frame +
  delegates), `set_delegate`, `set_preference`, `network_manager`,
  `site_data_manager`, `execute_webdriver_command`,
  `create_memory_report`, `setup_logging`,
  `initialize_gl_accelerated_media`.

The piece that makes a headless screenshot tool plausible is:

```
SoftwareRenderingContext::new(size)?  // pure software, no GPU
    .read_to_image(rect)              // -> ImageBuffer<Rgba<u8>>
```

combined with a `WebViewDelegate` that watches `notify_load_status_changed`
for `LoadStatus::Complete` and a loop calling `Servo::spin_event_loop`.

## CLI tool plan

`servo-shot URL_OR_PATH [--out shot.png] [--width 1280] [--height 800]`

1. Parse args.
2. If the argument is a path, convert to a `file://` URL.
3. Build a `SoftwareRenderingContext` of the requested size.
4. `ServoBuilder::default().build()`.
5. Build a `WebView` with our URL and a delegate that owns an
   `Arc<AtomicBool>` that flips to true on `LoadStatus::Complete`.
6. Loop: `servo.spin_event_loop()` until the flag is set (with a
   timeout). Then a few extra spins to let layout/paint settle.
7. `rendering_context.read_to_image(full_rect)` → save with the
   `image` crate as PNG.

## Build reality check

Servo is huge: it builds SpiderMonkey, WebRender, Stylo, etc. It has
non-trivial system dependencies (cmake, clang, gstreamer, fontconfig,
freetype, harfbuzz, OpenSSL, …) and the workspace pulls a Rust nightly
toolchain on real Servo checkouts. The crates.io publication in 0.1.0
collapses a lot of those internal crates into one but the system
dependencies remain.

### Reality check results

- `cargo fetch` succeeded and pulled down ~1000+ crates, including
  `mozjs_sys` (SpiderMonkey), `webrender`, `stylo`, `servo-script`,
  `servo-layout`, `servo-constellation`, `servo-paint`,
  `mozangle`, etc.
- `cargo check` (debug profile) **succeeded**, which means my code
  type-checks against the real 0.1.0 public API.
- Fixes required along the way:
  - `SoftwareRenderingContext::new` wants a `dpi::PhysicalSize<u32>`,
    not a `euclid::Size2D`. Had to add a direct `dpi = "0.1"` dep.
  - Its `Err` is `surfman::error::Error`, which does NOT implement
    `std::error::Error`, so you can't feed it straight to anyhow's
    `.context()`; use `.map_err(|e| anyhow::anyhow!(...))` instead.
- `cargo build --release` is kicked off in the background. Given the
  dependency tree it takes a while and needs cmake/clang/llvm for
  mozjs_sys. It may fail on a sandboxed container due to missing
  system libs (gstreamer / glib etc. for media), but the Servo source
  itself compiles.

## WebAssembly feasibility

Short answer: compiling the **full** `servo` crate to `wasm32-*` is
not feasible today. Reasons, in order of severity:

1. **SpiderMonkey (`mozjs_sys`)**. The JS engine that Servo embeds is
   itself a C++ compile that ships its own JIT and relies on
   platform-specific memory/threading primitives. It does not target
   `wasm32-unknown-unknown` or `wasm32-wasi`. You can't run a JS
   engine inside a JS VM easily, and Servo without JS is not
   meaningfully "Servo".
2. **Threads everywhere**. Servo's architecture explicitly uses
   many OS threads (constellation, script-per-origin, layout pool,
   paint worker, WebRender scene builder, network, resource loader,
   background hang monitor). Browser wasm only recently got
   threads via `SharedArrayBuffer` and it's still constrained.
   servo/servo#28070 (maintainer jdm) calls this out as a major
   blocker.
3. **OpenGL / WebGL / WebGPU**. The paint output path assumes a
   native GL context via `surfman` / WebRender. The wasm target
   would need a wgpu/WebGL bridge for everything.
4. **Networking stack**. Servo brings its own hyper + rustls +
   DNS. In the browser you'd want to route through `fetch()`; in
   wasi you'd need `wasi-sockets`. Either way it's a rewrite.
5. **File system + fonts**. Font enumeration goes through
   fontconfig/DirectWrite/CoreText. None of those exist in wasm.

### What CAN reasonably be compiled to wasm

Sub-crates of the Servo ecosystem, yes:

- **Stylo** – the CSS parser / selector-matcher / cascade engine.
  There's a recorded small patch (`servo/stylo#78`) that makes
  it build on wasm32. That unlocks things like:
    - An in-browser CSS selector / specificity playground.
    - Server-side HTML+CSS linting in a WASI function / Cloudflare
      Worker / edge runtime.
    - React / Yoga-style UIs that use real CSS cascade rules for
      layout even when running on canvas / in React Native.
- **html5ever / markup5ever** – the Servo HTML parser. Already
  compiles cleanly to wasm; great for WYSIWYG editors or sanitizers
  that need a spec-compliant tree.
- **url / idna** – already work on wasm.
- **selectors / cssparser** – work on wasm.
- **ICU4X data (icu_segmenter etc.)** – already designed to be
  wasm-friendly.

### Neat "single-page-app" ideas that ARE feasible with those pieces

Even though the whole browser doesn't fit in wasm, you can build
interesting SPAs around the pieces that do:

1. **CSS Selector Playground** – `cssparser` + `selectors` + a
   mini-DOM in wasm. Paste HTML on the left, a selector string on the
   right, see highlighted matches and their computed specificity.
2. **Live Stylo Debugger** – run the real browser's cascade engine in
   wasm and show, for any element, exactly which rules won, which
   lost, and why (matching Firefox/Servo's actual algorithm, not an
   approximation). Useful for teaching the cascade.
3. **HTML Sanitizer / Formatter in the browser** – use
   `html5ever` + `servo`'s sanitization rules in wasm to clean
   user-pasted rich text entirely client-side (no server round-trip,
   no DOMPurify).
4. **Headless "lint my page" SPA** – given a URL fetched via
   `fetch()`, parse it with `html5ever` and run Servo's accessibility
   or performance-hint checks entirely in the browser tab.
5. **WYSIWYG diff for HTML** – `html5ever` tree diffing in wasm to
   show structural diffs between two HTML documents.
6. **"What would Servo do?" SPA** – an educational tool where the
   user edits HTML+CSS and sees Stylo's actual tokenisation,
   cascaded styles and computed style tree in real time, right in
   the page, without shipping a whole browser.

If someone truly wants *the full renderer* in the browser, the
realistic path is not "compile Servo to wasm" but "run Servo natively
and stream its framebuffer to the browser over WebSocket+WebCodecs"
(which is basically what the cloud-browser projects do).

## Log of getting the CLI to actually render pixels

The first `cargo build --release` did produce a 153 MB binary and
the CLI ran without panicking — but the output PNG was just pure
white. Things I tried, in order, and what I learned:

1. **`SoftwareRenderingContext::new` needs `dpi::PhysicalSize<u32>`**
   (not `euclid::Size2D`). The `dpi` crate had to be added as a
   direct dep.
2. **`surfman::error::Error` doesn't implement `std::error::Error`.**
   `.context(...)` from anyhow doesn't work on it; needed `.map_err`.
3. **The sandbox was missing EGL.** `apt install libegl1` and
   setting `$XDG_RUNTIME_DIR` to a writable directory.
4. After all the above it ran and wrote a PNG — but the PNG was
   blank white. Things tried to fix it:
   - `webview.resize(size)` + `focus()` + `show()` after
     `WebViewBuilder::build()`. Made one warning
     ("Pipeline(1,1): Visibility change for closed pipeline")
     appear, but didn't help.
   - `webview.paint()` before `read_to_image`. Didn't help on its
     own.
   - `take_screenshot(None, cb)` — seemed correct given the docs,
     but the callback never fired even with a 60s deadline.
   - Reading Servo's own `tests/webview.rs` /
     `tests/common/mod.rs` gave the answer: **the `WebViewDelegate`
     must call `webview.paint()` inside
     `notify_new_frame_ready`**. That's the actual missing piece.
     After that I got `frame 1 painted` and `frame 2 painted` in
     the log — but the PNG was still blank.
   - Final piece: I was calling `rendering_context.present()`
     before `read_to_image`. Reading
     `servo-paint-api-0.1.0/rendering_context.rs` shows
     `SoftwareRenderingContext::present()` swaps buffers with
     `PreserveBuffer::No`, wiping the buffer I just painted.
     Removing that call made `sample.png` come out correctly.

The final working flow is in `servo-shot/src/main.rs`; all three
insights (`notify_new_frame_ready` → `webview.paint()`, don't call
`present()` before `read_to_image`, force a post-load frame via
`evaluate_javascript` + `requestAnimationFrame`) are documented
inline.

## Log of the wasm demo

Out of curiosity / because the user asked, I also spun up a
`wasm32-unknown-unknown` build of Servo's HTML parser:

1. `rustup target add wasm32-unknown-unknown` — succeeded.
2. Wrote a tiny crate depending on `html5ever = "0.35"` and
   `markup5ever_rcdom = "0.35.0-unofficial"` (the crate is published
   with a `-unofficial` suffix because it's not a core html5ever
   artefact; had to discover the exact suffix from the "candidate
   versions found" error).
3. `cargo build --release --target wasm32-unknown-unknown` —
   succeeded first try. Total ~12s. Output `.wasm` = 494 KB.
4. `cargo install wasm-bindgen-cli --version 0.2.118` (version must
   match the wasm-bindgen crate version picked during step 3, or
   `wasm-bindgen` will refuse to process the module).
5. `wasm-bindgen --target web --out-dir www …` produced a JS glue
   file and a 465 KB background wasm module.
6. Wrote a tiny `www/index.html` that loads the module and hooks up
   a textarea → `parse_tree` / `normalise` display.

Final payload (committed to `html5ever-wasm-demo/www/`):

- `index.html`   ~4 KB
- `html5ever_wasm_demo.js`   ~8 KB (generated)
- `html5ever_wasm_demo_bg.wasm`   ~454 KB (generated)

This validates the "Servo sub-crates on wasm" claim with an actual
working artefact, not just a theoretical plan.

