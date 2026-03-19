use image::DynamicImage;
use pdfium_render::prelude::*;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: pdf2img <input.pdf> <output_dir> [--dpi <dpi>]");
        std::process::exit(1);
    }

    let pdf_path = &args[1];
    let output_dir = PathBuf::from(&args[2]);
    let dpi = if args.len() >= 5 && args[3] == "--dpi" {
        args[4].parse::<u16>().unwrap_or(150)
    } else {
        150
    };

    std::fs::create_dir_all(&output_dir)?;

    // Try to find pdfium library
    let lib_path = std::env::var("PDFIUM_LIB_PATH").unwrap_or_else(|_| "./".to_string());
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(&lib_path),
        )
        .or_else(|_| Pdfium::bind_to_system_library())?,
    );

    let document = pdfium.load_pdf_from_file(pdf_path, None)?;
    let page_count = document.pages().len();
    println!("PDF has {} pages, rendering at {} DPI...", page_count, dpi);

    for (i, page) in document.pages().iter().enumerate() {
        let width_points = page.width().value;
        let pixel_width = (width_points * dpi as f32 / 72.0) as i32;

        let config = PdfRenderConfig::new()
            .set_target_width(pixel_width)
            .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

        let bitmap = page.render_with_config(&config)?;
        let img = bitmap.as_image();
        // Convert RGBA to RGB for JPEG compatibility
        let rgb_img = DynamicImage::ImageRgba8(
            img.as_rgba8().ok_or("Failed to get RGBA8 image")?.clone(),
        )
        .to_rgb8();
        let output_path = output_dir.join(format!("page_{:03}.jpg", i + 1));
        rgb_img.save(&output_path)?;
        println!("  Saved {}", output_path.display());
    }

    println!("Done! {} pages rendered.", page_count);
    Ok(())
}
