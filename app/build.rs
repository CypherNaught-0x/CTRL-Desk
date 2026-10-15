//! Embeds the app icon and version info into the Windows executable.

use std::path::PathBuf;

#[allow(dead_code)]
#[path = "src/icon.rs"]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let ico_path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ctrl-desk.ico");
    let style = icon::app_style();
    let frames: Vec<_> = [16, 20, 24, 32, 40, 48, 64, 128, 256]
        .into_iter()
        .map(|size| {
            image::codecs::ico::IcoFrame::as_png(
                &icon::render(size, &style),
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )
            .expect("encode icon frame")
        })
        .collect();
    let file = std::fs::File::create(&ico_path).expect("create icon file");
    image::codecs::ico::IcoEncoder::new(file)
        .encode_images(&frames)
        .expect("write icon");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().unwrap());
    res.set("ProductName", "CTRL Desk");
    res.set("FileDescription", "CTRL Desk");
    res.compile().expect("embed Windows resources");
}
