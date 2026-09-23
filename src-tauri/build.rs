use image::{imageops::FilterType, ImageFormat, Rgba, RgbaImage};
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let source_path = manifest_dir.join("../assets/branding/s4quota-mark-small.png");
    let source = image::open(&source_path)
        .unwrap_or_else(|error| panic!("failed to read S4Quota tray mark: {error}"))
        .to_rgba8();

    // The canonical optical mark is near-black with transparency. A restrained
    // neutral tile keeps that exact mark legible on both light and dark taskbars.
    let mut tray = RgbaImage::from_pixel(32, 32, Rgba([244, 244, 240, 255]));
    let mark = image::imageops::resize(&source, 22, 22, FilterType::Lanczos3);
    image::imageops::overlay(&mut tray, &mark, 5, 5);
    tray.save_with_format(output_dir.join("s4quota-tray.png"), ImageFormat::Png)
        .expect("failed to write derived S4Quota tray icon");

    println!("cargo:rerun-if-changed={}", source_path.display());
    tauri_build::build()
}
