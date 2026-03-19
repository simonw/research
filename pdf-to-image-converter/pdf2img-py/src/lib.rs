use image::DynamicImage;
use pdfium_render::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::path::PathBuf;

/// Find and bind to the pdfium library.
/// Searches: PDFIUM_LIB_PATH env var, directory of this .so, then system libraries.
fn bind_pdfium() -> Result<Pdfium, PdfiumError> {
    // Try PDFIUM_LIB_PATH env var first
    if let Ok(lib_path) = std::env::var("PDFIUM_LIB_PATH") {
        if let Ok(bindings) = Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(&lib_path),
        ) {
            return Ok(Pdfium::new(bindings));
        }
    }

    // Try the directory where this shared library lives (for bundled wheels)
    let self_dir = find_self_dir();
    if let Some(ref dir) = self_dir {
        if let Ok(bindings) = Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(dir.to_string_lossy().as_ref()),
        ) {
            return Ok(Pdfium::new(bindings));
        }
    }

    // Fall back to system library
    Ok(Pdfium::new(Pdfium::bind_to_system_library()?))
}

/// Find the directory containing this shared library
fn find_self_dir() -> Option<PathBuf> {
    // Use /proc/self/maps to find our .so path on Linux
    if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
        for line in maps.lines() {
            if line.contains("pdf2img_py") {
                if let Some(path_start) = line.find('/') {
                    let path = PathBuf::from(&line[path_start..]);
                    return path.parent().map(|p| p.to_path_buf());
                }
            }
        }
    }
    None
}

/// Convert a PDF file to a directory of JPEG images.
///
/// Args:
///     pdf_path: Path to the input PDF file
///     output_dir: Directory to save the rendered page images
///     dpi: Resolution in dots per inch (default: 150)
///
/// Returns:
///     List of output file paths
#[pyfunction]
#[pyo3(signature = (pdf_path, output_dir, dpi=150))]
fn pdf_to_images(
    pdf_path: &str,
    output_dir: &str,
    dpi: u16,
) -> PyResult<Vec<String>> {
    let output_dir = PathBuf::from(output_dir);
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create output dir: {}", e)))?;

    let pdfium = bind_pdfium()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to bind pdfium: {}", e)))?;

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to load PDF: {}", e)))?;

    let mut output_paths = Vec::new();

    for (i, page) in document.pages().iter().enumerate() {
        let width_points = page.width().value;
        let pixel_width = (width_points * dpi as f32 / 72.0) as i32;

        let config = PdfRenderConfig::new()
            .set_target_width(pixel_width)
            .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

        let bitmap = page
            .render_with_config(&config)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to render page {}: {}", i + 1, e)))?;

        let img = bitmap.as_image();
        let rgb_img = DynamicImage::ImageRgba8(
            img.as_rgba8()
                .ok_or_else(|| PyRuntimeError::new_err("Failed to get RGBA8 image"))?
                .clone(),
        )
        .to_rgb8();

        let output_path = output_dir.join(format!("page_{:03}.jpg", i + 1));
        rgb_img
            .save(&output_path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to save image: {}", e)))?;

        output_paths.push(output_path.to_string_lossy().to_string());
    }

    Ok(output_paths)
}

/// Get the number of pages in a PDF file.
#[pyfunction]
fn page_count(pdf_path: &str) -> PyResult<u16> {
    let pdfium = bind_pdfium()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to bind pdfium: {}", e)))?;

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to load PDF: {}", e)))?;

    Ok(document.pages().len())
}

/// Render a single page of a PDF to JPEG bytes.
///
/// Args:
///     pdf_path: Path to the input PDF file
///     page_number: 0-indexed page number
///     dpi: Resolution in dots per inch (default: 150)
///
/// Returns:
///     JPEG image data as bytes
#[pyfunction]
#[pyo3(signature = (pdf_path, page_number, dpi=150))]
fn render_page(pdf_path: &str, page_number: u16, dpi: u16) -> PyResult<Vec<u8>> {
    let pdfium = bind_pdfium()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to bind pdfium: {}", e)))?;

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to load PDF: {}", e)))?;

    let page = document
        .pages()
        .get(page_number)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to get page {}: {}", page_number, e)))?;

    let width_points = page.width().value;
    let pixel_width = (width_points * dpi as f32 / 72.0) as i32;

    let config = PdfRenderConfig::new()
        .set_target_width(pixel_width)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to render page: {}", e)))?;

    let img = bitmap.as_image();
    let rgb_img = DynamicImage::ImageRgba8(
        img.as_rgba8()
            .ok_or_else(|| PyRuntimeError::new_err("Failed to get RGBA8 image"))?
            .clone(),
    )
    .to_rgb8();

    let mut buf = std::io::Cursor::new(Vec::new());
    rgb_img
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to encode JPEG: {}", e)))?;

    Ok(buf.into_inner())
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(pdf_to_images, m)?)?;
    m.add_function(wrap_pyfunction!(page_count, m)?)?;
    m.add_function(wrap_pyfunction!(render_page, m)?)?;
    Ok(())
}
