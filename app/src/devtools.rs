//! Development helpers, driven by environment variables:
//!
//! - `DESK_CTRL_SCREENSHOT=out.png` saves a screenshot after
//!   `DESK_CTRL_SCREENSHOT_DELAY` seconds (default 3) and exits.
//! - `DESK_CTRL_STEP=find|connect|calibrate|personalise|done` advances the
//!   onboarding flow to that step as soon as each step allows it.
//! - `DESK_CTRL_SELECT=<device id or name>` picks that device on the Find step.
//! - `DESK_CTRL_SETTINGS=1` opens the settings dialog on start.

use std::path::PathBuf;

use crate::onboarding::Step;

#[derive(Default)]
pub struct DevTools {
    screenshot: Option<(PathBuf, f64)>,
    requested: bool,
    pub target_step: Option<Step>,
    pub select: Option<String>,
    pub open_settings: bool,
}

impl DevTools {
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let delay = var("DESK_CTRL_SCREENSHOT_DELAY")
            .and_then(|v| v.parse().ok())
            .unwrap_or(3.0);
        Self {
            screenshot: var("DESK_CTRL_SCREENSHOT").map(|p| (PathBuf::from(p), delay)),
            requested: false,
            target_step: var("DESK_CTRL_STEP").and_then(|s| match s.as_str() {
                "find" => Some(Step::Find),
                "connect" => Some(Step::Connect),
                "calibrate" => Some(Step::Calibrate),
                "personalise" => Some(Step::Personalise),
                "done" => Some(Step::Done),
                _ => None,
            }),
            select: var("DESK_CTRL_SELECT"),
            open_settings: var("DESK_CTRL_SETTINGS").is_some(),
        }
    }

    pub fn tick(&mut self, ctx: &egui::Context) {
        let Some((path, delay)) = &self.screenshot else {
            return;
        };
        ctx.request_repaint();
        if !self.requested && ctx.input(|i| i.time) >= *delay {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.requested = true;
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let [w, h] = img.size;
            if let Err(e) =
                image::save_buffer(path, &bytes, w as u32, h as u32, image::ColorType::Rgba8)
            {
                eprintln!("screenshot failed: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
