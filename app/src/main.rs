#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod demo;
mod devtools;
mod icon;
mod illustration;
mod onboarding;
mod theme;
mod tray;
mod widgets;
mod worker;

fn main() -> eframe::Result {
    if let Some(dir) = std::env::var_os("DESK_CTRL_EXPORT_ICONS") {
        if let Err(e) = devtools::export_icons(std::path::Path::new(&dir)) {
            eprintln!("icon export failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(if config::is_demo() {
                "CTRL Desk (demo)"
            } else {
                "CTRL Desk"
            })
            .with_icon(std::sync::Arc::new(egui::IconData {
                rgba: icon::render(256, &icon::app_style()),
                width: 256,
                height: 256,
            }))
            .with_inner_size([1180.0, 800.0])
            .with_min_inner_size([900.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CTRL Desk",
        options,
        Box::new(|cc| Ok(Box::new(app::DeskApp::new(cc)))),
    )
}
