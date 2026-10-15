//! First-run setup: find the desk, verify it, calibrate, personalise.

use egui::{Align2, Id, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, pos2, vec2};
use egui_phosphor::regular as icon;

use crate::app::{Conn, DARK_PRESETS, DeskApp, LIGHT_PRESETS, Motion, Screen};
use crate::config::{FavIcon, Favourite, SavedDesk, ThemeChoice};
use crate::illustration::{self, DeskScene};
use crate::theme::{Palette, Weight, alpha, font};
use crate::widgets::{self, Button, hold_button};
use crate::worker::{Command, Direction};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Welcome,
    Find,
    Connect,
    Calibrate,
    Personalise,
    Done,
}

impl Step {
    /// Steps shown in the progress indicator (Welcome and Done are not counted).
    const COUNTED: [Step; 4] = [
        Step::Find,
        Step::Connect,
        Step::Calibrate,
        Step::Personalise,
    ];
}

pub struct Onboarding {
    pub step: Step,
    selected: Option<(String, String)>,
    show_all: bool,
    measured_cm: f32,
    max_cm: f32,
    sitting_cm: f32,
    standing_cm: f32,
}

impl Default for Onboarding {
    fn default() -> Self {
        Self {
            step: Step::Welcome,
            selected: None,
            show_all: false,
            measured_cm: 0.0,
            max_cm: 0.0,
            sitting_cm: 72.0,
            standing_cm: 110.0,
        }
    }
}

enum Nav {
    Back,
    Next,
}

impl DeskApp {
    pub fn onboarding_view(&mut self, ui: &mut Ui, p: &Palette) {
        let screen = ui.max_rect();
        let w = (screen.width() - 48.0).min(640.0);
        let h = (screen.height() - 48.0).min(680.0);
        let rect = Rect::from_center_size(screen.center(), vec2(w, h));
        ui.painter().rect(
            rect.translate(vec2(0.0, 6.0)),
            24,
            alpha(p.shadow(), 0.25),
            Stroke::NONE,
            StrokeKind::Inside,
        );
        ui.painter().rect(
            rect,
            24,
            p.frame,
            Stroke::new(1.0, p.border),
            StrokeKind::Inside,
        );

        let inner = rect.shrink2(vec2(40.0, 34.0));
        let step = self.onboarding.step;

        // Progress dots.
        if let Some(idx) = Step::COUNTED.iter().position(|s| *s == step) {
            for (i, _) in Step::COUNTED.iter().enumerate() {
                let x = inner.left() + i as f32 * 30.0;
                let r = Rect::from_min_size(
                    pos2(x, inner.top() + 4.0),
                    vec2(if i == idx { 22.0 } else { 8.0 }, 8.0),
                );
                let r = if i > idx {
                    r.translate(vec2(14.0, 0.0))
                } else {
                    r
                };
                let c = if i <= idx {
                    p.primary
                } else {
                    alpha(p.muted, 0.3)
                };
                ui.painter().rect_filled(r, 4, c);
            }
            ui.painter().text(
                pos2(inner.right(), inner.top() + 8.0),
                Align2::RIGHT_CENTER,
                format!("Step {} of {}", idx + 1, Step::COUNTED.len()),
                font(14.0, Weight::Medium),
                p.muted,
            );
        }

        let footer = Rect::from_min_max(pos2(inner.left(), inner.bottom() - 46.0), inner.max);
        let content_top = if step == Step::Welcome || step == Step::Done {
            inner.top()
        } else {
            inner.top() + 36.0
        };
        let content = Rect::from_min_max(
            pos2(inner.left(), content_top),
            pos2(inner.right(), footer.top() - 16.0),
        );

        let nav = ui
            .scope_builder(UiBuilder::new().max_rect(content), |ui| match step {
                Step::Welcome => self.step_welcome(ui, p),
                Step::Find => self.step_find(ui, p),
                Step::Connect => self.step_connect(ui, p),
                Step::Calibrate => self.step_calibrate(ui, p),
                Step::Personalise => self.step_personalise(ui, p),
                Step::Done => self.step_done(ui, p),
            })
            .inner;
        let nav = self.footer(ui, footer, p).or(nav);

        match nav {
            Some(Nav::Next) => self.next_step(),
            Some(Nav::Back) => self.prev_step(),
            None => {}
        }
    }

    fn footer(&mut self, ui: &mut Ui, rect: Rect, p: &Palette) -> Option<Nav> {
        let step = self.onboarding.step;
        let (label, enabled) = self.next_state();
        let next_w = 180.0;
        let next = Rect::from_min_size(
            pos2(rect.right() - next_w, rect.top()),
            vec2(next_w, rect.height()),
        );
        let mut nav = None;
        let resp = Button::primary(label, p)
            .trailing_icon(icon::ARROW_RIGHT)
            .font(font(16.0, Weight::SemiBold))
            .enabled(enabled)
            .show_at(ui, next, Id::new("ob_next"));
        if resp.clicked() || (enabled && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
            nav = Some(Nav::Next);
        }
        if !matches!(step, Step::Welcome | Step::Done) {
            let back = Rect::from_min_size(rect.min, vec2(110.0, rect.height()));
            if Button::ghost("Back", p)
                .icon(icon::ARROW_LEFT)
                .show_at(ui, back, Id::new("ob_back"))
                .clicked()
            {
                nav = Some(Nav::Back);
            }
        }
        if step == Step::Welcome && self.cfg.desk.is_some() {
            let cancel = Rect::from_min_size(rect.min, vec2(110.0, rect.height()));
            if Button::ghost("Cancel", p)
                .show_at(ui, cancel, Id::new("ob_cancel"))
                .clicked()
            {
                self.screen = Screen::Main;
            }
        }
        nav
    }

    fn next_state(&self) -> (&'static str, bool) {
        match self.onboarding.step {
            Step::Welcome => ("Get started", true),
            Step::Find => ("Connect", self.onboarding.selected.is_some()),
            Step::Connect => ("Continue", self.connected() && self.sample.is_some()),
            Step::Calibrate => ("Looks right", self.sample.is_some()),
            Step::Personalise => ("Continue", true),
            Step::Done => ("Open dashboard", true),
        }
    }

    /// Dev helper: walks the flow to `dev.target_step` (see `devtools`).
    pub fn autopilot(&mut self) {
        let Some(target) = self.dev.target_step else {
            return;
        };
        if self.onboarding.step == Step::Find && self.onboarding.selected.is_none() {
            let want = self.dev.select.clone();
            if let Some(d) = self.devices.iter().find(|d| {
                want.as_deref().is_some_and(|w| {
                    w.eq_ignore_ascii_case(&d.id)
                        || d.name.as_deref().is_some_and(|n| w.eq_ignore_ascii_case(n))
                })
            }) {
                self.onboarding.selected = Some((d.id.clone(), d.name.clone().unwrap_or_default()));
            }
        }
        if self.onboarding.step < target && self.next_state().1 {
            self.next_step();
        }
    }

    fn next_step(&mut self) {
        let ob = &mut self.onboarding;
        match ob.step {
            Step::Welcome => {
                ob.step = Step::Find;
                self.start_scan();
            }
            Step::Find => {
                let Some((id, _)) = ob.selected.clone() else {
                    return;
                };
                ob.step = Step::Connect;
                self.worker.send(Command::Connect { id });
                self.conn = Conn::Connecting;
            }
            Step::Connect => {
                // Start from what the controller reports.
                let reported = self
                    .info
                    .as_ref()
                    .and_then(|i| i.base_height_mm)
                    .map(f32::from)
                    .unwrap_or(620.0);
                let raw_mm = self.sample.map(|s| s.height.0 as f32 / 10.0).unwrap_or(0.0);
                let base = self.cfg.base_height_mm.unwrap_or(reported);
                let ob = &mut self.onboarding;
                ob.measured_cm = ((base + raw_mm) / 10.0 * 10.0).round() / 10.0;
                ob.max_cm = self
                    .cfg
                    .max_height_mm
                    .unwrap_or(base + crate::config::DEFAULT_TRAVEL_MM)
                    / 10.0;
                ob.step = Step::Calibrate;
            }
            Step::Calibrate => {
                let raw_mm = self.sample.map(|s| s.height.0 as f32 / 10.0).unwrap_or(0.0);
                let base = ob.measured_cm * 10.0 - raw_mm;
                self.cfg.base_height_mm = Some(base);
                self.cfg.max_height_mm = Some((ob.max_cm * 10.0).max(base));
                if let Some(f) = self
                    .cfg
                    .favourites
                    .iter()
                    .find(|f| f.icon == FavIcon::Sitting)
                {
                    self.onboarding.sitting_cm = f.height_mm / 10.0;
                }
                if let Some(f) = self
                    .cfg
                    .favourites
                    .iter()
                    .find(|f| f.icon == FavIcon::Standing)
                {
                    self.onboarding.standing_cm = f.height_mm / 10.0;
                }
                self.onboarding.step = Step::Personalise;
            }
            Step::Personalise => {
                let favs = &mut self.cfg.favourites;
                for (ic, name, cm) in [
                    (FavIcon::Sitting, "Sitting", ob.sitting_cm),
                    (FavIcon::Standing, "Standing", ob.standing_cm),
                ] {
                    match favs.iter_mut().find(|f| f.icon == ic) {
                        Some(f) => f.height_mm = cm * 10.0,
                        None => favs.push(Favourite {
                            name: name.into(),
                            height_mm: cm * 10.0,
                            icon: ic,
                        }),
                    }
                }
                ob.step = Step::Done;
            }
            Step::Done => {
                if let Some((id, name)) = ob.selected.clone() {
                    self.cfg.desk = Some(SavedDesk { name, id });
                }
                self.cfg.save();
                self.screen = Screen::Main;
            }
        }
    }

    fn prev_step(&mut self) {
        self.onboarding.step = match self.onboarding.step {
            Step::Welcome | Step::Find => {
                self.worker.send(Command::StopScan);
                Step::Welcome
            }
            Step::Connect => {
                self.start_scan();
                Step::Find
            }
            Step::Calibrate => Step::Connect,
            Step::Personalise => Step::Calibrate,
            Step::Done => Step::Personalise,
        };
    }

    // ---- Steps ----------------------------------------------------------

    fn step_welcome(&mut self, ui: &mut Ui, p: &Palette) -> Option<Nav> {
        let r = ui.max_rect();
        let art = Rect::from_min_size(r.min, vec2(r.width(), (r.height() * 0.58).min(320.0)));
        illustration::paint_background(ui, art, p, p.frame);
        let t = ui.input(|i| i.time) as f32;
        let h = 935.0 + 215.0 * (t * 0.7).sin();
        illustration::paint(
            ui,
            art,
            &DeskScene {
                height_mm: h,
                max_mm: 1250.0,
                label: None,
                center_x: 0.5,
                avoid: None,
            },
            p,
        );
        ui.painter()
            .rect_stroke(art, 16, Stroke::new(1.0, p.border), StrokeKind::Inside);
        ui.ctx().request_repaint();

        ui.allocate_space(vec2(r.width(), art.height() + 26.0));
        heading(ui, "Welcome to Desk Control", p);
        body(
            ui,
            "Let's connect to your standing desk. You'll find it over Bluetooth, check its height, and pick your favourite positions. It takes about a minute.",
            p,
        );
        if let Some(e) = &self.adapter_error {
            ui.add_space(6.0);
            warning(
                ui,
                &format!("Bluetooth isn't available: {e}. Turn Bluetooth on and restart the app."),
                p,
            );
        }
        None
    }

    fn step_find(&mut self, ui: &mut Ui, p: &Palette) -> Option<Nav> {
        heading(ui, "Find your desk", p);
        body(
            ui,
            "Make sure the desk is plugged in and nearby. If it's connected to the phone app, close the app first so the desk can be found.",
            p,
        );
        ui.add_space(4.0);

        let mut devices: Vec<_> = self
            .devices
            .iter()
            .filter(|d| self.onboarding.show_all || d.name.is_some())
            .cloned()
            .collect();
        devices.sort_by_key(|d| {
            (
                !d.likely_desk,
                d.name.is_none(),
                -(d.rssi.unwrap_or(-127) as i32),
            )
        });
        let hidden = self.devices.len() - devices.len();

        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
            widgets::spinner(ui, r.center(), 7.0, p.muted);
            ui.label(
                egui::RichText::new(format!("Scanning…  {} found", devices.len())).color(p.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.onboarding.show_all {
                    "Hide unnamed devices".to_string()
                } else {
                    format!("Show all devices ({hidden} unnamed)")
                };
                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(label).color(p.muted).size(13.5))
                            .frame(false),
                    )
                    .clicked()
                {
                    self.onboarding.show_all = !self.onboarding.show_all;
                }
            });
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if devices.is_empty() {
                    ui.add_space(30.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("Looking for nearby devices…").color(p.muted));
                    });
                }
                for d in &devices {
                    let selected = self
                        .onboarding
                        .selected
                        .as_ref()
                        .is_some_and(|(a, _)| *a == d.id);
                    let (rect, resp) = ui.allocate_exact_size(
                        vec2(ui.available_width() - 6.0, 64.0),
                        Sense::click(),
                    );
                    let fill = if selected {
                        p.tint
                    } else if resp.hovered() {
                        alpha(p.text, 0.04)
                    } else {
                        p.card
                    };
                    let stroke = if selected {
                        Stroke::new(2.0, p.primary)
                    } else {
                        Stroke::new(1.0, p.border)
                    };
                    ui.painter()
                        .rect(rect, 12, fill, stroke, StrokeKind::Inside);
                    let icon_pos = pos2(rect.left() + 32.0, rect.center().y);
                    if d.likely_desk {
                        widgets::table_icon(ui.painter(), icon_pos, 28.0, p.text);
                    } else {
                        ui.painter().text(
                            icon_pos,
                            Align2::CENTER_CENTER,
                            icon::BLUETOOTH,
                            crate::theme::icon_font(26.0),
                            p.text,
                        );
                    }
                    let name = d.name.clone().unwrap_or_else(|| "Unnamed device".into());
                    ui.painter().text(
                        pos2(rect.left() + 62.0, rect.top() + 14.0),
                        Align2::LEFT_TOP,
                        &name,
                        font(16.0, Weight::SemiBold),
                        p.text,
                    );
                    ui.painter().text(
                        pos2(rect.left() + 62.0, rect.top() + 36.0),
                        Align2::LEFT_TOP,
                        &d.id,
                        font(13.0, Weight::Regular),
                        p.muted,
                    );
                    if d.likely_desk {
                        let g = ui.painter().layout_no_wrap(
                            "Desk".into(),
                            font(12.5, Weight::SemiBold),
                            p.on_primary,
                        );
                        let badge = Rect::from_min_size(
                            pos2(rect.right() - 120.0, rect.center().y - 12.0),
                            vec2(g.size().x + 18.0, 24.0),
                        );
                        ui.painter().rect_filled(badge, 12, p.primary);
                        ui.painter().galley(
                            badge.min + vec2(9.0, (24.0 - g.size().y) / 2.0),
                            g,
                            p.on_primary,
                        );
                    }
                    let bars = Rect::from_min_size(
                        pos2(rect.right() - 44.0, rect.center().y - 9.0),
                        vec2(22.0, 18.0),
                    );
                    widgets::signal_bars(ui, bars, d.rssi, p.text, alpha(p.muted, 0.25));
                    if resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.onboarding.selected = Some((d.id.clone(), name));
                    }
                    ui.add_space(4.0);
                }
            });
        None
    }

    fn step_connect(&mut self, ui: &mut Ui, p: &Palette) -> Option<Nav> {
        let name = self
            .onboarding
            .selected
            .as_ref()
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        match self.conn.clone() {
            Conn::Connected => {
                ui.label(crate::theme::icon_text(
                    icon::CHECK_CIRCLE,
                    "Connected",
                    font(28.0, Weight::SemiBold),
                    p.text,
                    false,
                ));
                ui.add_space(2.0);
                body(
                    ui,
                    "Press and hold the buttons below. If the desk moves, you've got the right one.",
                    p,
                );
                ui.add_space(8.0);
                let info = self.info.clone();
                egui::Grid::new("ob_info")
                    .num_columns(2)
                    .spacing([28.0, 8.0])
                    .show(ui, |ui| {
                        let rows = [
                            (
                                "Name",
                                info.as_ref().map(|i| i.name.clone()).unwrap_or(name),
                            ),
                            (
                                "Manufacturer",
                                info.as_ref()
                                    .map(|i| i.manufacturer.clone())
                                    .unwrap_or_default(),
                            ),
                            (
                                "Model",
                                info.as_ref().map(|i| i.model.clone()).unwrap_or_default(),
                            ),
                            (
                                "Device ID",
                                self.onboarding
                                    .selected
                                    .as_ref()
                                    .map(|(a, _)| a.clone())
                                    .unwrap_or_default(),
                            ),
                        ];
                        for (k, v) in rows {
                            ui.label(egui::RichText::new(k).color(p.muted));
                            ui.label(v);
                            ui.end_row();
                        }
                    });
                ui.add_space(14.0);
                let r = ui.available_rect_before_wrap();
                let row = Rect::from_min_size(
                    r.min,
                    vec2(r.width(), (r.height() - 4.0).clamp(70.0, 120.0)),
                );
                let readout = Rect::from_min_size(row.min, vec2(row.width() * 0.4, row.height()));
                ui.painter().rect(
                    readout,
                    14,
                    p.card,
                    Stroke::new(1.0, p.border),
                    StrokeKind::Inside,
                );
                ui.painter().text(
                    readout.left_top() + vec2(18.0, 16.0),
                    Align2::LEFT_TOP,
                    "Raw height",
                    font(13.0, Weight::Medium),
                    p.muted,
                );
                let raw = self
                    .sample
                    .map(|s| format!("{:.1} cm", s.height.0 as f32 / 100.0))
                    .unwrap_or_else(|| "—".into());
                ui.painter().text(
                    readout.left_bottom() + vec2(18.0, -16.0),
                    Align2::LEFT_BOTTOM,
                    raw,
                    font(30.0, Weight::Bold),
                    p.text,
                );
                ui.painter().text(
                    readout.right_top() + vec2(-14.0, 16.0),
                    Align2::RIGHT_TOP,
                    "above lowest",
                    font(12.0, Weight::Regular),
                    p.muted,
                );

                let bw = (row.width() * 0.6 - 24.0) / 2.0;
                let up_r = Rect::from_min_size(
                    pos2(readout.right() + 12.0, row.top()),
                    vec2(bw, row.height()),
                );
                let down_r = Rect::from_min_size(
                    pos2(up_r.right() + 12.0, row.top()),
                    vec2(bw, row.height()),
                );
                let up = hold_button(
                    ui,
                    up_r,
                    Id::new("ob_up"),
                    egui_phosphor::bold::CARET_UP,
                    "Raise",
                    p.primary,
                    p.on_primary,
                    true,
                );
                let down = hold_button(
                    ui,
                    down_r,
                    Id::new("ob_down"),
                    egui_phosphor::bold::CARET_DOWN,
                    "Lower",
                    p.tint,
                    p.on_tint,
                    true,
                );
                let want = if up.is_pointer_button_down_on() {
                    Some(Direction::Up)
                } else if down.is_pointer_button_down_on() {
                    Some(Direction::Down)
                } else {
                    None
                };
                if want.is_some() || matches!(self.motion, Motion::Jogging(_)) {
                    self.set_jog(want);
                }
            }
            Conn::Failed(e) => {
                heading(ui, "Couldn't connect", p);
                warning(ui, &e, p);
                ui.add_space(6.0);
                body(
                    ui,
                    "• Make sure no other app or phone is connected to the desk.\n• Move closer, or press a button on the desk to wake it.\n• If the desk was paired in Windows Bluetooth settings, try removing it there.",
                    p,
                );
                ui.add_space(10.0);
                if Button::tinted("Try again", p)
                    .icon(icon::ARROWS_CLOCKWISE)
                    .show(ui, vec2(150.0, 40.0))
                    .clicked()
                    && let Some((id, _)) = self.onboarding.selected.clone()
                {
                    self.worker.send(Command::Connect { id });
                    self.conn = Conn::Connecting;
                }
            }
            _ => {
                heading(ui, &format!("Connecting to {name}…"), p);
                body(
                    ui,
                    "Reading the controller's details. This can take up to 20 seconds.",
                    p,
                );
                let r = ui.available_rect_before_wrap();
                widgets::spinner(
                    ui,
                    pos2(r.center().x, r.top() + r.height() * 0.4),
                    22.0,
                    p.primary,
                );
            }
        }
        None
    }

    fn step_calibrate(&mut self, ui: &mut Ui, p: &Palette) -> Option<Nav> {
        heading(ui, "Check the height", p);
        body(
            ui,
            "Measure from the floor to the top of the desk surface and correct the value below if needed. All heights in the app are based on it.",
            p,
        );
        ui.add_space(10.0);

        let reported = self.height_mm_with_reported_base();
        let ob = &mut self.onboarding;
        egui::Grid::new("ob_cal")
            .num_columns(2)
            .spacing([24.0, 14.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Current height").font(font(16.0, Weight::Medium)));
                ui.add(
                    egui::DragValue::new(&mut ob.measured_cm)
                        .range(40.0..=200.0)
                        .speed(0.1)
                        .fixed_decimals(1)
                        .suffix(" cm")
                        .custom_formatter(|v, _| format!("{v:.1}")),
                );
                ui.end_row();
                ui.label(egui::RichText::new("Highest position").font(font(16.0, Weight::Medium)));
                ui.add(
                    egui::DragValue::new(&mut ob.max_cm)
                        .range(60.0..=200.0)
                        .speed(0.1)
                        .fixed_decimals(1)
                        .suffix(" cm"),
                );
                ui.end_row();
            });
        ui.add_space(8.0);
        if let Some(h) = reported {
            body(
                ui,
                &format!(
                    "The controller reports {:.1} cm. The highest position is only used for the range shown in the app.",
                    h / 10.0
                ),
                p,
            );
        }

        // Preview of the result.
        let r = ui.available_rect_before_wrap();
        if r.height() > 120.0 {
            let art =
                Rect::from_min_size(r.min + vec2(0.0, 8.0), vec2(r.width(), r.height() - 8.0));
            illustration::paint_background(ui, art, p, p.frame);
            let h = self.onboarding.measured_cm * 10.0;
            illustration::paint(
                ui,
                art,
                &DeskScene {
                    height_mm: h,
                    max_mm: self.onboarding.max_cm * 10.0,
                    label: Some(format!("{:.1} cm", h / 10.0)),
                    center_x: 0.48,
                    avoid: None,
                },
                p,
            );
            ui.painter()
                .rect_stroke(art, 16, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        None
    }

    fn height_mm_with_reported_base(&self) -> Option<f32> {
        let base = self.info.as_ref()?.base_height_mm? as f32;
        Some(self.sample?.height.0 as f32 / 10.0 + base)
    }

    fn step_personalise(&mut self, ui: &mut Ui, p: &Palette) -> Option<Nav> {
        heading(ui, "Make it yours", p);
        body(
            ui,
            "Pick a look and your two main positions. You can change all of this later in Settings.",
            p,
        );
        ui.add_space(8.0);

        let mut changed = false;
        ui.label(egui::RichText::new("Theme").font(font(15.0, Weight::Medium)));
        if let Some(i) = widgets::segmented(
            ui,
            &["System", "Light", "Dark"],
            self.cfg.theme as usize,
            300.0,
            p,
        ) {
            self.cfg.theme = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark][i];
            changed = true;
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Primary color").font(font(15.0, Weight::Medium)));
        ui.horizontal(|ui| {
            if p.dark {
                changed |= widgets::color_choice(ui, &mut self.cfg.primary_dark, &DARK_PRESETS, p);
            } else {
                changed |=
                    widgets::color_choice(ui, &mut self.cfg.primary_light, &LIGHT_PRESETS, p);
            }
            ui.add_space(12.0);
            // Live preview.
            let (r, _) = ui.allocate_exact_size(vec2(150.0, 34.0), Sense::hover());
            let half = Rect::from_min_size(r.min, vec2(r.width() / 2.0 - 4.0, r.height()));
            ui.painter().rect_filled(half, 8, p.primary);
            ui.painter().text(
                half.center(),
                Align2::CENTER_CENTER,
                "Raise",
                font(13.0, Weight::SemiBold),
                p.on_primary,
            );
            let half2 = half.translate(vec2(half.width() + 8.0, 0.0));
            ui.painter().rect_filled(half2, 8, p.tint);
            ui.painter().text(
                half2.center(),
                Align2::CENTER_CENTER,
                "Lower",
                font(13.0, Weight::SemiBold),
                p.on_tint,
            );
        });
        if changed {
            self.cfg.save();
        }

        ui.add_space(12.0);
        let current = self
            .sample
            .map(|s| s.height.0 as f32 / 10.0 + self.base_mm());
        let ob = &mut self.onboarding;
        egui::Grid::new("ob_favs")
            .num_columns(3)
            .spacing([18.0, 12.0])
            .show(ui, |ui| {
                for (glyph, label, value) in [
                    (FavIcon::Sitting.glyph(), "Sitting", &mut ob.sitting_cm),
                    (FavIcon::Standing.glyph(), "Standing", &mut ob.standing_cm),
                ] {
                    ui.label(crate::theme::icon_text(
                        glyph,
                        label,
                        font(16.0, Weight::Medium),
                        p.text,
                        false,
                    ));
                    ui.add(
                        egui::DragValue::new(value)
                            .range(40.0..=200.0)
                            .speed(0.1)
                            .fixed_decimals(1)
                            .suffix(" cm"),
                    );
                    let use_current =
                        ui.add_enabled(current.is_some(), egui::Button::new("Use current height"));
                    if use_current.clicked() {
                        *value = (current.unwrap_or_default() / 10.0 * 10.0).round() / 10.0;
                    }
                    ui.end_row();
                }
            });
        None
    }

    fn step_done(&mut self, ui: &mut Ui, p: &Palette) -> Option<Nav> {
        let r = ui.max_rect();
        let c = pos2(r.center().x, r.top() + r.height() * 0.3);
        ui.painter().circle_filled(c, 54.0, p.tint);
        ui.painter().circle_filled(c, 40.0, p.primary);
        ui.painter().text(
            c,
            Align2::CENTER_CENTER,
            egui_phosphor::bold::CHECK,
            crate::theme::icon_bold(40.0),
            p.on_primary,
        );
        ui.allocate_space(vec2(r.width(), r.height() * 0.3 + 80.0));
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("You're all set").font(font(30.0, Weight::SemiBold)));
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Press and hold Raise or Lower (or the ↑ / ↓ keys) to move the desk, or jump to a favourite with one click.")
                    .font(font(16.0, Weight::Regular))
                    .color(p.muted),
            );
        });
        None
    }
}

fn heading(ui: &mut Ui, text: &str, p: &Palette) {
    ui.label(
        egui::RichText::new(text)
            .font(font(28.0, Weight::SemiBold))
            .color(p.text),
    );
    ui.add_space(2.0);
}

fn body(ui: &mut Ui, text: &str, p: &Palette) {
    ui.label(
        egui::RichText::new(text)
            .font(font(16.0, Weight::Regular))
            .color(p.muted),
    );
}

fn warning(ui: &mut Ui, text: &str, p: &Palette) {
    egui::Frame::new()
        .fill(alpha(p.danger, 0.1))
        .corner_radius(10)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(crate::theme::icon_text(
                icon::WARNING,
                text,
                font(15.0, Weight::Regular),
                p.danger,
                false,
            ));
        });
}
