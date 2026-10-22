//! Development helpers, driven by environment variables:
//!
//! - `DESK_CTRL_SCREENSHOT=out.png` saves a screenshot after
//!   `DESK_CTRL_SCREENSHOT_DELAY` seconds (default 3) and exits.
//! - `DESK_CTRL_STEP=find|connect|calibrate|personalise|done` advances the
//!   onboarding flow to that step as soon as each step allows it.
//! - `DESK_CTRL_SELECT=<device id or name>` picks that device on the Find step.
//! - `DESK_CTRL_SETTINGS=1` opens the settings dialog on start.
//! - `DESK_CTRL_MOVE_TO=<mm>` moves the desk there once connected.
//! - `DESK_CTRL_REMINDER=stand|sit` shows the reminder to stand up or sit down.
//! - `DESK_CTRL_EXPORT_ICONS=<dir>` writes the icons as PNGs (and an ICO) and exits.

use std::path::PathBuf;

use crate::activity::Posture;
use crate::onboarding::Step;

#[derive(Default)]
pub struct DevTools {
    screenshot: Option<(PathBuf, f64)>,
    requested: bool,
    pub target_step: Option<Step>,
    pub select: Option<String>,
    pub open_settings: bool,
    pub move_to: Option<f32>,
    /// The posture held too long, for a reminder shown on start.
    pub reminder: Option<Posture>,
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
            move_to: var("DESK_CTRL_MOVE_TO").and_then(|v| v.parse().ok()),
            reminder: var("DESK_CTRL_REMINDER").and_then(|s| match s.as_str() {
                "stand" => Some(Posture::Sitting),
                "sit" => Some(Posture::Standing),
                _ => None,
            }),
        }
    }

    /// Whether this is an automated screenshot run.
    pub fn capturing(&self) -> bool {
        self.screenshot.is_some()
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

/// Writes the app icon at several sizes plus both tray glyph variants as PNGs,
/// and a multi-size `icon.ico`, for packaging (`DESK_CTRL_EXPORT_ICONS=<dir>`).
pub fn export_icons(dir: &std::path::Path) -> std::io::Result<()> {
    use crate::icon::{GLYPH_DARK, GLYPH_LIGHT, Style, app_style, render};
    std::fs::create_dir_all(dir)?;
    let save = |name: String, size: u32, style: &Style| {
        image::save_buffer(
            dir.join(name),
            &render(size, style),
            size,
            size,
            image::ColorType::Rgba8,
        )
        .map_err(std::io::Error::other)
    };
    for size in [16, 32, 64, 128, 256, 512, 1024] {
        save(format!("icon-{size}.png"), size, &app_style())?;
    }
    save("tray-light.png".into(), 64, &Style::Glyph(GLYPH_LIGHT))?;
    save("tray-dark.png".into(), 64, &Style::Glyph(GLYPH_DARK))?;
    let frames = [16, 24, 32, 48, 64, 128, 256]
        .into_iter()
        .map(|size| {
            image::codecs::ico::IcoFrame::as_png(
                &render(size, &app_style()),
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(std::io::Error::other)?;
    image::codecs::ico::IcoEncoder::new(std::fs::File::create(dir.join("icon.ico"))?)
        .encode_images(&frames)
        .map_err(std::io::Error::other)
}
