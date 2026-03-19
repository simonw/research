# pdfium-render Crate Research

Research into the `pdfium-render` Rust crate for rendering PDF pages to JPEG images.

- **Crate**: [pdfium-render on crates.io](https://crates.io/crates/pdfium-render)
- **Repository**: [ajrcarey/pdfium-render on GitHub](https://github.com/ajrcarey/pdfium-render)
- **Docs**: [docs.rs/pdfium-render](https://docs.rs/pdfium-render/latest/pdfium_render/)
- **Latest version**: 0.8.37

## 1. Rendering PDF Pages to JPEG Images

The crate provides a clean, high-level API for rendering PDF pages to images. Here is a complete working example:

```rust
use pdfium_render::prelude::*;
use std::path::Path;

fn export_pdf_to_jpegs(
    path: &impl AsRef<Path>,
    password: Option<&str>,
) -> Result<(), PdfiumError> {
    let pdfium = Pdfium::default();
    let document = pdfium.load_pdf_from_file(path, password)?;

    let render_config = PdfRenderConfig::new()
        .set_target_width(2000)
        .set_maximum_height(2000)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    for (index, page) in document.pages().iter().enumerate() {
        page.render_with_config(&render_config)?
            .as_image()       // Returns image::DynamicImage (RGBA)
            .into_rgb8()      // Strip alpha channel (required for JPEG)
            .save_with_format(
                format!("page-{}.jpg", index),
                image::ImageFormat::Jpeg,
            )
            .map_err(|_| PdfiumError::ImageError)?;
    }

    Ok(())
}
```

Key points:
- `as_image()` returns an `image::DynamicImage` (with alpha channel)
- Must call `.into_rgb8()` before saving as JPEG since JPEG does not support alpha
- Uses the standard `image` crate for saving, so all image formats supported by `image` are available

## 2. Getting the Pdfium Shared Library

pdfium-render does **not** bundle Pdfium. You must supply it separately.

### Option A: Prebuilt Dynamic Library (Recommended for Development)

Download prebuilt binaries from [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries/releases):

```bash
# Linux x64
wget https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-x64.tgz
mkdir pdfium && tar -xzf pdfium-linux-x64.tgz -C pdfium

# macOS arm64
wget https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-mac-arm64.tgz

# Linux x64 (musl/Alpine)
wget https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-linux-musl-x64.tgz
```

Then either:
- Place `libpdfium.so` in the same directory as your executable
- Place it in `/usr/lib/` or `/usr/local/lib/`
- Set `PDFIUM_DYNAMIC_LIB_PATH` environment variable to the containing directory

### Option B: System-Installed Pdfium

If Pdfium is installed system-wide, `Pdfium::bind_to_system_library()` will find it automatically.

### Option C: pdfium-bind Crate (Auto-Download)

The separate [`pdfium-bind`](https://crates.io/crates/pdfium-bind) crate can automatically download the appropriate Pdfium binary for your platform during the build process, eliminating manual setup.

## 3. Key API Reference

### Initializing Pdfium

```rust
use pdfium_render::prelude::*;

// Option 1: Automatic (tries local dir, then system)
let pdfium = Pdfium::default();

// Option 2: Specific library path
let pdfium = Pdfium::new(
    Pdfium::bind_to_library(
        Pdfium::pdfium_platform_library_name_at_path("./lib/")
    ).or_else(|_| Pdfium::bind_to_system_library())
    .unwrap()
);

// Option 3: Static linking (requires `static` feature)
let pdfium = Pdfium::new(
    Pdfium::bind_to_statically_linked_library().unwrap()
);
```

### Opening a PDF Document

```rust
// From file path
let document = pdfium.load_pdf_from_file("input.pdf", None)?;

// From file path with password
let document = pdfium.load_pdf_from_file("encrypted.pdf", Some("password"))?;

// From bytes (useful for data from network/memory)
let bytes: Vec<u8> = std::fs::read("input.pdf")?;
let document = pdfium.load_pdf_from_byte_vec(bytes, None)?;

// From a reader (streams content, good for large files)
let file = std::fs::File::open("large.pdf")?;
let reader = std::io::BufReader::new(file);
let document = pdfium.load_pdf_from_reader(reader, None)?;
```

### Iterating Pages

```rust
// Iterate all pages
for (index, page) in document.pages().iter().enumerate() {
    println!("Page {}: {}x{} points",
        index, page.width().value, page.height().value);
}

// Get page count
let count = document.pages().len();

// Access specific page by index
let page = document.pages().get(0)?;
```

### Configuring Rendering

```rust
let config = PdfRenderConfig::new()
    // Scale to target width, maintaining aspect ratio
    .set_target_width(2000)
    // Constrain maximum height
    .set_maximum_height(3000)
    // Or use fixed dimensions (ignores aspect ratio)
    // .set_fixed_size(1920, 1080)
    // Or scale by factor
    // .scale_page_by_factor(2.0)
    // Auto-rotate landscape pages
    .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true)
    // Generate thumbnails
    // .thumbnail(200)
    ;
```

### Rendering and Saving

```rust
// Render with config
let bitmap = page.render_with_config(&config)?;

// Convert to image::DynamicImage
let image = bitmap.as_image();

// Save as JPEG (must strip alpha first)
image.into_rgb8()
    .save_with_format("output.jpg", image::ImageFormat::Jpeg)
    .map_err(|_| PdfiumError::ImageError)?;

// Save as PNG (alpha supported)
image.save("output.png")?;
```

## 4. Static Linking vs Dynamic Library

### Dynamic Linking (Default)

- Pdfium is loaded at **runtime** via `libloading`
- Library must be present on the target system
- More flexible: can swap library versions without recompiling
- `Pdfium::default()` handles fallback automatically

### Static Linking

Static linking is supported but requires more setup:

**Cargo.toml:**
```toml
[dependencies]
pdfium-render = { version = "0.8", features = ["static"] }
# If your pdfium was built with GNU C++ stdlib:
# pdfium-render = { version = "0.8", features = ["static", "libstdc++"] }
# If built with LLVM C++ stdlib:
# pdfium-render = { version = "0.8", features = ["static", "libc++"] }
```

**Build command:**
```bash
PDFIUM_STATIC_LIB_PATH="/path/to/dir/containing/libpdfium.a" cargo build
```

**Runtime initialization:**
```rust
let pdfium = Pdfium::new(
    Pdfium::bind_to_statically_linked_library().unwrap()
);
```

Requirements:
- The static library must be named `libpdfium.a` (Linux/macOS) or `pdfium.lib` (Windows)
- `PDFIUM_STATIC_LIB_PATH` points to the **directory** containing the library, not the file itself
- You may need `libstdc++` or `libc++` feature depending on how Pdfium was compiled
- On macOS, may also need the `core_graphics` feature

## 5. Cargo.toml Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `pdfium_latest` | Yes | Use latest Pdfium API (currently `pdfium_7543`) |
| `image_latest` | Yes | Use latest `image` crate (currently `image_025`) |
| `thread_safe` | Yes | Mutex-protected Pdfium access for thread safety |
| `static` | No | Enable static linking to Pdfium |
| `libstdc++` | No | Link GNU C++ stdlib (requires `static`) |
| `libc++` | No | Link LLVM C++ stdlib (requires `static`) |
| `core_graphics` | No | Link macOS CoreGraphics (requires `static`) |
| `sync` | No | Add Send/Sync impls (requires `thread_safe`) |
| `image_025` | No | Pin to image crate 0.25.x |
| `image_024` | No | Pin to image crate 0.24.x |
| `image_023` | No | Pin to image crate 0.23.x |
| `pdfium_use_skia` | No | Use Skia rendering backend |
| `pdfium_use_win32` | No | Use Win32 rendering backend |
| `pdfium_enable_xfa` | No | Enable XFA form support |
| `pdfium_enable_v8` | No | Enable V8 JavaScript engine |

## 6. Minimal Cargo.toml

```toml
[dependencies]
pdfium-render = "0.8"
image = "0.25"   # Needed for ImageFormat::Jpeg and save methods
```

The default features (`pdfium_latest`, `image_latest`, `thread_safe`) are sufficient for basic PDF-to-JPEG rendering.

## 7. Additional Notes

- **Minimum Rust version**: 1.80.1 (with image feature), 1.61 (without)
- **Thread safety**: Pdfium itself is not thread-safe. The `thread_safe` feature serializes all calls behind a mutex. For parallel PDF processing, use separate processes rather than threads.
- **WASM**: Supported. File-based loading methods are unavailable; use `load_pdf_from_byte_slice` or `load_pdf_from_byte_vec` instead. Prebuilt WASM builds available from [paulocoutinhox/pdfium-lib](https://github.com/paulocoutinhox/pdfium-lib/releases).
- **Error handling**: All fallible operations return `Result<T, PdfiumError>`.
- **Creating new PDFs**: `pdfium.create_new_pdf()` creates an empty document in memory.
