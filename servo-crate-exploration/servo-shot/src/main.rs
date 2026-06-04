//! servo-shot: render a URL or local HTML file to a PNG using the
//! `servo` crate published in v0.1.0 (2026-04-13).
//!
//! Usage:
//!
//!     servo-shot https://example.com
//!     servo-shot ./page.html --out page.png --width 1280 --height 800
//!
//! The flow (cross-checked against `servo`'s own integration tests in
//! `tests/webview.rs` / `tests/common/mod.rs`):
//!
//! 1. Build a `SoftwareRenderingContext` of the viewport size. Software
//!    GL means no GPU, no X server — which is exactly what we want for
//!    headless / CI / server-side screenshot rendering.
//! 2. Spin up a `Servo` via `ServoBuilder` with an empty proxy
//!    preference (avoids the default system-proxy lookup).
//! 3. Create a `WebView` pointed at the URL. The delegate must forward
//!    `notify_new_frame_ready` to `webview.paint()` — this is the
//!    contract Servo expects, without it the compositor never actually
//!    draws (the public tests do exactly this).
//! 4. Kick off `webview.take_screenshot(None, cb)`. Servo itself
//!    internally waits for:
//!      - every frame's `load` event,
//!      - render-blocking `<link>` stylesheets,
//!      - images to be displayed,
//!      - web fonts,
//!      - `reftest-wait` / `test-wait` classes being removed,
//!      - the rendering pipeline to be up to date.
//!    So we don't need to separately wait for `LoadStatus::Complete`.
//! 5. Spin `servo.spin_event_loop()` until the callback fires, then
//!    save the returned `RgbaImage` as PNG.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::Parser;
use dpi::PhysicalSize;
use euclid::{Box2D, Point2D, Scale};
use image::RgbaImage;
use servo::{
    LoadStatus, RenderingContext, Servo, ServoBuilder, SoftwareRenderingContext, WebView,
    WebViewBuilder, WebViewDelegate,
};
use servo::Preferences;
use url::Url;

/// Render a web page to a PNG, using the Servo engine.
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// URL (https://…) or path to a local HTML file.
    target: String,

    /// Output file path.
    #[arg(short, long, default_value = "shot.png")]
    out: String,

    /// Viewport width in physical pixels.
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// Viewport height in physical pixels.
    #[arg(long, default_value_t = 800)]
    height: u32,

    /// HiDPI / device-pixel-ratio to report to the page.
    #[arg(long, default_value_t = 1.0)]
    dpr: f32,

    /// How long to wait for `take_screenshot` to call back.
    #[arg(long, default_value_t = 60)]
    timeout_secs: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let url = resolve_target(&args.target)?;

    // 1. Rendering context.
    let size = PhysicalSize::new(args.width, args.height);
    let rendering_context: Rc<dyn RenderingContext> = Rc::new(
        SoftwareRenderingContext::new(size)
            // surfman::error::Error doesn't implement std::error::Error,
            // so we map it through a string before handing it to anyhow.
            .map_err(|e| anyhow::anyhow!("creating SoftwareRenderingContext: {e:?}"))?,
    );
    // Servo's own tests require the context be made current before use.
    let _ = rendering_context.make_current();

    // 2. Servo instance. Disable the system HTTP(S) proxy lookup so a
    //    missing / malformed proxy env var can't stall localhost loads.
    let mut prefs = Preferences::default();
    prefs.network_http_proxy_uri = String::new();
    prefs.network_https_proxy_uri = String::new();
    let servo: Servo = ServoBuilder::default().preferences(prefs).build();
    servo.setup_logging();

    // 3. Delegate. The critical part is calling `webview.paint()` in
    //    `notify_new_frame_ready` — Servo's tests do this, and without
    //    it the back buffer is never filled.
    let delegate: Rc<ShotDelegate> = Rc::new(ShotDelegate::default());
    let webview: WebView = WebViewBuilder::new(&servo, rendering_context.clone())
        .url(url)
        .hidpi_scale_factor(Scale::new(args.dpr))
        .delegate(delegate.clone() as Rc<dyn WebViewDelegate>)
        .build();

    // 4. Wait for LoadStatus::Complete, then for at least one
    //    `notify_new_frame_ready` (which our delegate pipes into
    //    `webview.paint()`), then read pixels back from the
    //    rendering context.
    let deadline = Instant::now() + Duration::from_secs(args.timeout_secs);
    while !delegate.loaded.get() {
        if Instant::now() > deadline {
            bail!("timed out waiting for page load after {}s", args.timeout_secs);
        }
        servo.spin_event_loop();
        std::thread::sleep(Duration::from_millis(1));
    }

    // After load, force at least one frame by toggling an inconsequential
    // style on <html> via the JS evaluator. Servo's own test harness
    // does exactly this in `show_webview_and_wait_for_rendering_to_be_ready`.
    let js_done = Rc::new(Cell::new(false));
    {
        let js_done = js_done.clone();
        // Ask for a rAF tick, but don't actually change anything
        // visible — just force the compositor to produce one more
        // frame so we have a guaranteed post-load paint.
        webview.evaluate_javascript(
            "new Promise(r => requestAnimationFrame(() => { \
               document.documentElement.getBoundingClientRect(); r(); \
             }))",
            move |_r| js_done.set(true),
        );
    }
    while !js_done.get() {
        if Instant::now() > deadline {
            bail!("timed out waiting for JS nudge");
        }
        servo.spin_event_loop();
        std::thread::sleep(Duration::from_millis(1));
    }

    // Wait for at least one post-load frame_ready.
    let frames_at_load = delegate.frames.get();
    while delegate.frames.get() <= frames_at_load {
        if Instant::now() > deadline {
            bail!(
                "timed out waiting for a post-load frame (saw {})",
                delegate.frames.get()
            );
        }
        servo.spin_event_loop();
        std::thread::sleep(Duration::from_millis(1));
    }

    // NOTE: do NOT call `rendering_context.present()` here: for
    // `SoftwareRenderingContext` that swaps front and back buffers
    // with `PreserveBuffer::No`, wiping the buffer we just painted.
    // The doc comment on the trait says so explicitly: "once Servo
    // renders to the context, [`read_to_image`] should return those
    // results, even before [`RenderingContext::present`] is called."
    let rect = Box2D::new(
        Point2D::new(0, 0),
        Point2D::new(args.width as i32, args.height as i32),
    );
    let buf: RgbaImage = rendering_context
        .read_to_image(rect)
        .context("read_to_image returned None")?;

    // 5. Save.
    buf.save(&args.out)
        .with_context(|| format!("saving PNG to {}", args.out))?;

    eprintln!(
        "servo-shot: wrote {}x{} -> {}",
        args.width, args.height, args.out
    );
    Ok(())
}

/// Treat bare paths as file URLs so people can do `servo-shot ./index.html`.
fn resolve_target(target: &str) -> Result<Url> {
    if let Ok(url) = Url::parse(target) {
        // `Url::parse` accepts "C:\foo" on Windows as scheme `c`, so
        // only accept schemes that actually make sense here.
        if matches!(url.scheme(), "http" | "https" | "file" | "data" | "about") {
            return Ok(url);
        }
    }
    let path = Path::new(target);
    if path.exists() {
        let abs = path
            .canonicalize()
            .with_context(|| format!("canonicalising {}", target))?;
        return Url::from_file_path(&abs)
            .map_err(|_| anyhow::anyhow!("could not convert {:?} to a file:// URL", abs));
    }
    bail!("{target:?} is neither a URL nor an existing file")
}

/// Minimal delegate. The only thing we actually *must* do is forward
/// `notify_new_frame_ready` to `webview.paint()` — everything else
/// defaults to a no-op.
#[derive(Default)]
struct ShotDelegate {
    loaded: Cell<bool>,
    frames: Cell<u32>,
}

impl WebViewDelegate for ShotDelegate {
    fn notify_load_status_changed(&self, _webview: WebView, status: LoadStatus) {
        if matches!(status, LoadStatus::Complete) {
            self.loaded.set(true);
        }
    }

    fn notify_new_frame_ready(&self, webview: WebView) {
        // REQUIRED: drives the compositor. Servo's own
        // tests/common/mod.rs does exactly this. Without it the
        // `SoftwareRenderingContext`'s framebuffer never gets any
        // actual pixels and you'll get an all-white PNG.
        webview.paint();
        self.frames.set(self.frames.get() + 1);
    }
}
