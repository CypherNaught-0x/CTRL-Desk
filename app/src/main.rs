#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod demo;
mod devtools;
mod illustration;
mod onboarding;
mod theme;
mod widgets;
mod worker;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(if config::is_demo() {
                "Desk Control (demo)"
            } else {
                "Desk Control"
            })
            .with_inner_size([1180.0, 800.0])
            .with_min_inner_size([900.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Desk Control",
        options,
        Box::new(|cc| Ok(Box::new(app::DeskApp::new(cc)))),
    )
}
