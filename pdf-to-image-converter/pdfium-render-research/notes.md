# pdfium-render Research Notes

## Date: 2026-03-19

### Sources Consulted
- GitHub repo: https://github.com/ajrcarey/pdfium-render (note: author is ajrcarey, not nickalbs)
- docs.rs: https://docs.rs/pdfium-render/latest/pdfium_render/
- crates.io: https://crates.io/crates/pdfium-render
- Pdfium prebuilt binaries: https://github.com/bblanchon/pdfium-binaries

### Key Findings

1. **Crate version**: Latest is 0.8.37 (as of docs.rs). The `pdfium_latest` feature currently maps to `pdfium_7543`.

2. **Binding approach**: pdfium-render does NOT include Pdfium itself. It binds to an external Pdfium library either at runtime (dynamic) or at compile time (static).

3. **Dynamic binding** is the default and simplest approach:
   - `Pdfium::default()` tries current directory first, then system libraries
   - `Pdfium::bind_to_library(path)` loads from a specific path
   - `Pdfium::bind_to_system_library()` loads from system library paths

4. **Static binding** requires:
   - Enable `static` feature flag
   - Set `PDFIUM_STATIC_LIB_PATH` env var at build time
   - May also need `libstdc++` or `libc++` feature for C++ stdlib linking
   - Call `Pdfium::bind_to_statically_linked_library()` at runtime

5. **Prebuilt pdfium binaries** available from bblanchon/pdfium-binaries:
   - Linux x64: `https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-x64.tgz`
   - Also available: musl variant, macOS, Windows, Android, iOS, WASM
   - Latest version: 147.0.7725

6. **Alternative**: The `pdfium-bind` crate automatically downloads prebuilt binaries during build.

7. **Image rendering** requires the `image` feature (enabled by default via `image_latest`).

8. **The `as_image()` method** on `PdfBitmap` returns a `DynamicImage` from the `image` crate. Need to call `.into_rgb8()` before saving as JPEG (to strip alpha channel).

9. **Thread safety**: Pdfium is NOT thread-safe. The `thread_safe` feature (enabled by default) wraps access behind a mutex. For parallel processing, use separate processes or the recommended approach of parallel processing rather than multi-threading.

10. **WASM support**: Pdfium can be compiled to WASM. File system functions unavailable; use byte slice/vec loading instead.

11. **Minimum Rust version**: 1.80.1 with image feature, 1.61 without.

### PdfRenderConfig Key Methods
- `set_target_width(pixels)` / `set_target_height(pixels)` - scale maintaining aspect ratio
- `set_fixed_width(pixels)` / `set_fixed_height(pixels)` - exact dimensions
- `scale_page_by_factor(f32)` - scale by multiplier
- `set_maximum_width(pixels)` / `set_maximum_height(pixels)` - constrain max dimensions
- `rotate_if_landscape(rotation, do_rotate_constraints)` - auto-rotate landscape pages
- `thumbnail(size)` - generate thumbnail
- `set_clear_color(color)` - set background color before rendering

### Document Loading Methods on Pdfium
- `load_pdf_from_file(path, password)` - from file path (not available in WASM)
- `load_pdf_from_byte_slice(bytes, password)` - from &[u8]
- `load_pdf_from_byte_vec(bytes, password)` - from Vec<u8>, pdfium manages lifetime
- `load_pdf_from_reader(reader, password)` - from Read+Seek, streams content

### Cargo.toml Feature Flags (Notable)
- **Default**: `pdfium_latest`, `image_latest`, `thread_safe`
- `static` - static linking
- `libstdc++` / `libc++` - C++ stdlib linking (requires `static`)
- `core_graphics` - macOS CoreGraphics linking (requires `static`)
- `thread_safe` - mutex-protected access
- `sync` - Send/Sync implementations (requires `thread_safe`)
- `image_025` / `image_024` / `image_023` - specific image crate versions
- `pdfium_use_skia` / `pdfium_use_win32` - rendering backend selection
- `pdfium_enable_xfa` / `pdfium_enable_v8` - optional Pdfium features
