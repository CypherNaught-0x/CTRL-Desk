use egui::text::LayoutJob;
use egui::{
    Align2, Color32, FontId, Id, Key, Popup, PopupCloseBehavior, Rect, Sense, Stroke, StrokeKind,
    TextFormat, Ui, UiBuilder, pos2, vec2,
};
use egui_phosphor::regular as icon;
use linak_desk::{DeskInfo, Height, HeightSpeed};

use crate::config::{self, Config, FavIcon, Favourite, ThemeChoice};
use crate::illustration::{self, DeskScene};
use crate::onboarding::Onboarding;
use crate::theme::{self, Palette, Weight, alpha, font};
use crate::tray::TrayAction;
use crate::widgets::{self, Button, card, hold_button, icon_button, link_button};
use crate::worker::{Command, Direction, Event, FoundDevice, Worker};

pub const LIGHT_PRESETS: [[u8; 3]; 6] = [
    config::DEFAULT_PRIMARY_LIGHT,
    [0x2d, 0x5b, 0x8a],
    [0xa8, 0x52, 0x37],
    [0x6b, 0x4a, 0x7a],
    [0x2b, 0x6a, 0x6a],
    [0x3a, 0x3a, 0x3a],
];
pub const DARK_PRESETS: [[u8; 3]; 6] = [
    config::DEFAULT_PRIMARY_DARK,
    [0xa3, 0xc9, 0xab],
    [0x9e, 0xc2, 0xe6],
    [0xe8, 0xb4, 0x8f],
    [0xc5, 0xae, 0xe0],
    [0xf0, 0xd2, 0x7f],
];

#[derive(Debug, Clone, PartialEq)]
pub enum Conn {
    Idle,
    Scanning,
    Connecting,
    Connected,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Motion {
    Idle,
    MovingTo(String),
    Jogging(Direction),
}

#[derive(PartialEq)]
pub enum Screen {
    Onboarding,
    Main,
}

pub struct DeskApp {
    pub cfg: Config,
    pub worker: Worker,
    pub conn: Conn,
    pub info: Option<DeskInfo>,
    pub sample: Option<HeightSpeed>,
    pub devices: Vec<FoundDevice>,
    pub motion: Motion,
    pub screen: Screen,
    pub onboarding: Onboarding,
    pub adapter_error: Option<String>,
    settings_open: bool,
    toast: Option<(String, f64)>,
    applied_palette: Option<Palette>,
    pub dev: crate::devtools::DevTools,
    tray: Option<crate::tray::Tray>,
    window_visible: bool,
    /// Set by "Quit" so closing isn't redirected to the tray.
    quitting: bool,
}

impl DeskApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let cfg = Config::load();
        let worker = Worker::spawn(cc.egui_ctx.clone(), crate::config::is_demo());
        let screen = match &cfg.desk {
            Some(desk) => {
                worker.send(Command::Connect {
                    id: desk.id.clone(),
                });
                Screen::Main
            }
            None => Screen::Onboarding,
        };
        Self {
            cfg,
            worker,
            conn: Conn::Idle,
            info: None,
            sample: None,
            devices: Vec::new(),
            motion: Motion::Idle,
            screen,
            onboarding: Onboarding::default(),
            adapter_error: None,
            settings_open: false,
            toast: None,
            applied_palette: None,
            dev: crate::devtools::DevTools::from_env(),
            tray: None,
            window_visible: true,
            quitting: false,
        }
        .with_dev_settings()
        .with_tray(&cc.egui_ctx)
    }

    fn with_dev_settings(mut self) -> Self {
        self.settings_open = self.dev.open_settings;
        self
    }

    fn with_tray(mut self, ctx: &egui::Context) -> Self {
        // Automated screenshot runs don't need (or want) a tray icon.
        if !self.dev.capturing() {
            self.tray = crate::tray::Tray::new(ctx);
        }
        self
    }

    fn show_window(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.window_visible = true;
    }

    fn hide_window(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        self.window_visible = false;
    }

    /// Short status for the tray, e.g. "104.5 cm · Moving".
    fn tray_status(&self) -> String {
        match (&self.conn, self.height_mm()) {
            (Conn::Connected, Some(h)) => {
                let moving =
                    self.motion != Motion::Idle || self.sample.is_some_and(|s| s.speed.0 != 0);
                format!(
                    "{:.1} cm{}",
                    h / 10.0,
                    if moving { " · Moving" } else { "" }
                )
            }
            (Conn::Connecting, _) => "Connecting…".into(),
            _ if self.cfg.desk.is_none() => "Not set up yet".into(),
            _ => "Desk offline".into(),
        }
    }

    /// Tray, minimize-to-tray and close-to-tray handling. Runs even while the
    /// window is hidden.
    fn window_and_tray(&mut self, ctx: &egui::Context) {
        if self.tray.is_none() {
            return;
        }
        let state = crate::tray::TrayState {
            window_visible: self.window_visible,
            status: self.tray_status(),
            favourites: self
                .cfg
                .favourites
                .iter()
                .map(|f| format!("{}  ·  {:.1} cm", f.name, f.height_mm / 10.0))
                .collect(),
            can_move: self.conn == Conn::Connected,
        };
        let Some(tray) = &mut self.tray else { return };
        tray.sync(ctx, state);
        for action in tray.poll() {
            match action {
                TrayAction::ToggleWindow if self.window_visible => self.hide_window(ctx),
                TrayAction::ToggleWindow => self.show_window(ctx),
                TrayAction::MoveTo(i) => {
                    if let Some(f) = self.cfg.favourites.get(i).cloned() {
                        self.move_to_mm(f.height_mm, f.name);
                    }
                }
                TrayAction::Stop => {
                    self.worker.send(Command::Stop);
                    self.motion = Motion::Idle;
                }
                TrayAction::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        let (minimized, close_requested) = ctx.input(|i| {
            (
                i.viewport().minimized == Some(true),
                i.viewport().close_requested(),
            )
        });
        if self.window_visible && minimized && self.cfg.minimize_to_tray {
            self.hide_window(ctx);
        }
        if close_requested && !self.quitting && self.cfg.close_to_tray {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_window(ctx);
        }
    }

    // ---- Height helpers -------------------------------------------------

    /// Tabletop height at the lowest position.
    pub fn base_mm(&self) -> f32 {
        self.cfg
            .base_height_mm
            .or(self
                .info
                .as_ref()
                .and_then(|i| i.base_height_mm)
                .map(f32::from))
            .unwrap_or(620.0)
    }

    pub fn max_mm(&self) -> f32 {
        self.cfg
            .max_height_mm
            .unwrap_or(self.base_mm() + config::DEFAULT_TRAVEL_MM)
    }

    pub fn height_mm(&self) -> Option<f32> {
        self.sample
            .map(|s| s.height.0 as f32 / 10.0 + self.base_mm())
    }

    pub fn connected(&self) -> bool {
        self.conn == Conn::Connected
    }

    pub fn move_to_mm(&mut self, mm: f32, label: String) {
        let raw = ((mm - self.base_mm()) * 10.0)
            .round()
            .clamp(0.0, u16::MAX as f32) as u16;
        self.worker.send(Command::MoveTo(Height(raw)));
        self.motion = Motion::MovingTo(label);
    }

    /// Starts or stops jogging when the requested direction changes.
    pub fn set_jog(&mut self, want: Option<Direction>) {
        let current = match self.motion {
            Motion::Jogging(d) => Some(d),
            _ => None,
        };
        if want == current {
            return;
        }
        match want {
            Some(d) => {
                self.worker.send(Command::Jog(d));
                self.motion = Motion::Jogging(d);
            }
            None => {
                self.worker.send(Command::Stop);
                self.motion = Motion::Idle;
            }
        }
    }

    pub fn start_scan(&mut self) {
        self.devices.clear();
        self.worker.send(Command::StartScan);
        self.conn = Conn::Scanning;
        self.info = None;
        self.sample = None;
    }

    pub fn toast(&mut self, msg: impl Into<String>, now: f64) {
        self.toast = Some((msg.into(), now));
    }

    pub fn palette(&self, ctx: &egui::Context) -> Palette {
        let dark = match self.cfg.theme {
            ThemeChoice::Light => false,
            ThemeChoice::Dark => true,
            ThemeChoice::System => ctx.system_theme() == Some(egui::Theme::Dark),
        };
        let primary = if dark {
            self.cfg.primary_dark
        } else {
            self.cfg.primary_light
        };
        Palette::new(dark, theme::from_rgb(primary))
    }

    fn process_events(&mut self, now: f64) {
        let events: Vec<Event> = self.worker.poll().collect();
        for ev in events {
            match ev {
                Event::AdapterError(e) => self.adapter_error = Some(e),
                Event::ScanResults(d) => self.devices = d,
                Event::Connecting => self.conn = Conn::Connecting,
                Event::Connected { info } => {
                    self.conn = Conn::Connected;
                    self.info = Some(info);
                }
                Event::ConnectFailed(e) => {
                    self.conn = Conn::Failed(e);
                    // During setup the user decides whether to retry.
                    if self.screen == Screen::Onboarding {
                        self.worker.send(Command::Disconnect);
                    }
                }
                Event::Disconnected => {
                    if self.conn == Conn::Connected {
                        self.conn = Conn::Idle;
                    }
                    self.motion = Motion::Idle;
                }
                Event::Sample(hs) => self.sample = Some(hs),
                Event::MotionDone => {
                    if matches!(self.motion, Motion::MovingTo(_)) {
                        self.motion = Motion::Idle;
                    }
                }
                Event::Error(e) => self.toast(e, now),
            }
        }
    }

    // ---- Main view ------------------------------------------------------

    fn main_view(&mut self, ui: &mut Ui, p: &Palette) {
        let outer = ui.max_rect().shrink(14.0);
        ui.painter().rect(
            outer,
            22,
            p.frame,
            Stroke::new(1.0, p.border),
            StrokeKind::Inside,
        );
        let inner = outer.shrink(14.0);

        let header = Rect::from_min_size(inner.min, vec2(inner.width(), 58.0));
        self.header(ui, header, p);

        let body = Rect::from_min_max(pos2(inner.left(), header.bottom() + 6.0), inner.max);
        let n = self.cfg.favourites.len();
        let rows = n.div_ceil(3).max(1) as f32;
        let item_h = 158.0;
        let fav_h = 74.0 + rows * item_h + (rows - 1.0) * 14.0 + 22.0;
        let main_h = (body.height() - fav_h - 14.0).max(400.0);
        let total_h = main_h + 14.0 + fav_h;

        ui.scope_builder(UiBuilder::new().max_rect(body), |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    let (r, _) =
                        ui.allocate_exact_size(vec2(ui.available_width(), total_h), Sense::hover());
                    let hero_w = ((r.width() - 14.0) * 0.67).round();
                    let hero = Rect::from_min_size(r.min, vec2(hero_w, main_h));
                    let adjust = Rect::from_min_max(
                        pos2(hero.right() + 14.0, r.top()),
                        pos2(r.right(), r.top() + main_h),
                    );
                    let favs = Rect::from_min_size(
                        pos2(r.left(), hero.bottom() + 14.0),
                        vec2(r.width(), fav_h),
                    );
                    self.hero(ui, hero, p);
                    self.adjust_card(ui, adjust, p);
                    self.favourites(ui, favs, item_h, p);
                });
        });
    }

    fn header(&mut self, ui: &mut Ui, rect: Rect, p: &Palette) {
        let cy = rect.center().y;
        let painter = ui.painter();
        widgets::table_icon(painter, pos2(rect.left() + 38.0, cy), 40.0, p.text);
        painter.text(
            pos2(rect.left() + 72.0, cy),
            Align2::LEFT_CENTER,
            "My desk",
            font(26.0, Weight::SemiBold),
            p.text,
        );

        let gear = icon_button(
            ui,
            pos2(rect.right() - 24.0, cy),
            26.0,
            icon::GEAR_SIX,
            p.text,
            Id::new("gear"),
        );
        if gear.on_hover_text("Settings").clicked() {
            self.settings_open = true;
        }
        let painter = ui.painter();
        let div_x = rect.right() - 56.0;
        painter.line_segment(
            [pos2(div_x, cy - 14.0), pos2(div_x, cy + 14.0)],
            Stroke::new(1.0, p.divider),
        );

        let (label, dot) = match (&self.adapter_error, &self.conn) {
            (Some(_), _) => ("Bluetooth unavailable", p.danger),
            (_, Conn::Connected) => ("Connected", p.dot),
            (_, Conn::Connecting) => ("Connecting…", alpha(p.muted, 0.6)),
            (_, Conn::Scanning) => ("Scanning…", alpha(p.muted, 0.6)),
            _ => ("Offline", p.danger),
        };
        let galley = painter.layout_no_wrap(label.into(), font(17.0, Weight::Regular), p.text);
        let text_x = div_x - 22.0 - galley.size().x;
        let text_rect =
            Rect::from_min_size(pos2(text_x, cy - galley.size().y / 2.0), galley.size());
        painter.galley(text_rect.min, galley, p.text);
        let dot_c = pos2(text_x - 16.0, cy);
        if self.conn == Conn::Connecting {
            widgets::spinner(ui, dot_c, 6.0, p.muted);
        } else {
            ui.painter().circle_filled(dot_c, 6.0, dot);
        }
        let status = ui.interact(
            text_rect
                .expand(8.0)
                .union(Rect::from_center_size(dot_c, vec2(14.0, 14.0))),
            Id::new("status"),
            Sense::hover(),
        );
        let tip = match (&self.adapter_error, &self.conn, &self.info, &self.cfg.desk) {
            (Some(e), ..) => e.clone(),
            (_, Conn::Failed(e), ..) => format!("Couldn't connect: {e}\nRetrying automatically."),
            (_, _, Some(info), Some(desk)) => format!(
                "{} · {}\n{} {}",
                info.name, desk.id, info.manufacturer, info.model
            ),
            (_, _, _, Some(desk)) => format!("{} · {}", desk.name, desk.id),
            _ => String::new(),
        };
        if !tip.is_empty() {
            status.on_hover_text(tip);
        }
    }

    fn hero(&mut self, ui: &mut Ui, rect: Rect, p: &Palette) {
        illustration::paint_background(ui, rect, p, p.frame);
        let target = self.height_mm().unwrap_or(self.base_mm() + 250.0);
        let shown = ui
            .ctx()
            .animate_value_with_time(Id::new("hero_height"), target, 0.3);
        let left = rect.left() + 36.0;
        ui.painter().galley(
            pos2(left, rect.top() + 34.0),
            spaced(
                ui,
                "CURRENT HEIGHT",
                font(14.5, Weight::Medium),
                p.muted,
                2.2,
            ),
            p.muted,
        );

        let number = self
            .height_mm()
            .map(|h| format!("{:.1}", h / 10.0))
            .unwrap_or_else(|| "—".into());
        let big = (rect.height() * 0.17).clamp(56.0, 88.0);
        let num_fmt = TextFormat {
            font_id: font(big, Weight::Bold),
            color: p.text,
            extra_letter_spacing: -big * 0.03,
            ..Default::default()
        };
        let (job, trailing) = widgets::tabular_job(ui.painter(), &number, &num_fmt);
        let num = ui.painter().layout_job(job);
        let unit =
            ui.painter()
                .layout_no_wrap("cm".into(), font(big * 0.4, Weight::Medium), p.text);
        // Align the unit on the number's baseline.
        let num_pos = pos2(left - 4.0, rect.top() + 56.0);
        let unit_pos = pos2(
            num_pos.x + num.size().x + trailing + big * 0.08,
            num_pos.y + baseline(&num) - baseline(&unit),
        );
        // Keep the desk clear of the widest reading this desk can show, so the
        // illustration doesn't shift as the digits change.
        let widest = format!("{:.1}", self.max_mm().max(1000.0) / 10.0)
            .replace(|c: char| c.is_ascii_digit(), "0");
        let reserved = widgets::tabular_width(ui.painter(), &widest, &num_fmt);
        let text_block = Rect::from_min_max(
            pos2(left, rect.top() + 34.0),
            pos2(
                num_pos.x + reserved + big * 0.08 + unit.size().x,
                num_pos.y + num.size().y,
            ),
        );
        let label = self.height_mm().map(|h| format!("{:.1} cm", h / 10.0));
        illustration::paint(
            ui,
            rect,
            &DeskScene {
                height_mm: shown,
                max_mm: self.max_mm(),
                label,
                center_x: 0.6,
                avoid: Some(text_block),
            },
            p,
        );
        ui.painter()
            .rect_stroke(rect, 16, Stroke::new(1.0, p.border), StrokeKind::Inside);

        ui.painter().galley(num_pos, num, p.text);
        ui.painter().galley(unit_pos, unit, p.text);

        // Status pill.
        let speed = self.sample.map(|s| s.speed.mm_per_s()).unwrap_or(0.0);
        let status = if !self.connected() {
            "Desk is offline".to_string()
        } else if speed.abs() > 0.1 || self.motion != Motion::Idle {
            let dir = if speed > 0.0 {
                "up"
            } else if speed < 0.0 {
                "down"
            } else {
                ""
            };
            match &self.motion {
                Motion::MovingTo(name) => format!("Moving to {name}"),
                _ => format!("Moving {dir}  ·  {:.1} cm/s", speed.abs() / 10.0)
                    .replace("  ·  0.0 cm/s", ""),
            }
        } else {
            "Desk is stationary".to_string()
        };
        let moving = self.connected() && (speed.abs() > 0.1 || self.motion != Motion::Idle);
        let galley = ui
            .painter()
            .layout_no_wrap(status, font(16.5, Weight::Regular), p.text);
        let pill = Rect::from_min_size(
            pos2(left - 6.0, rect.bottom() - 34.0 - 44.0),
            vec2(galley.size().x + 66.0, 44.0),
        );
        ui.painter().rect_filled(pill, 22, alpha(p.pill, 0.92));
        let dot = if moving {
            p.primary
        } else {
            alpha(p.muted, 0.7)
        };
        ui.painter()
            .circle_filled(pos2(pill.left() + 24.0, pill.center().y), 6.5, dot);
        ui.painter().galley(
            pos2(pill.left() + 44.0, pill.center().y - galley.size().y / 2.0),
            galley,
            p.text,
        );

        if moving && matches!(self.motion, Motion::MovingTo(_)) {
            let stop = Rect::from_min_size(pos2(pill.right() + 10.0, pill.top()), vec2(96.0, 44.0));
            let resp = Button::primary("Stop", p)
                .icon(icon::STOP)
                .radius(22)
                .show_at(ui, stop, Id::new("stop"));
            if resp.clicked() {
                self.worker.send(Command::Stop);
                self.motion = Motion::Idle;
            }
        }
    }

    fn adjust_card(&mut self, ui: &mut Ui, rect: Rect, p: &Palette) {
        card(ui, rect, p);
        let pad = 32.0;
        let painter = ui.painter();
        painter.text(
            pos2(rect.left() + pad, rect.top() + 34.0),
            Align2::LEFT_TOP,
            "Adjust height",
            font(30.0, Weight::SemiBold),
            p.text,
        );
        painter.text(
            pos2(rect.left() + pad, rect.top() + 78.0),
            Align2::LEFT_TOP,
            "Press and hold to move",
            font(17.5, Weight::Regular),
            p.muted,
        );

        let enabled = self.connected();
        let area = Rect::from_min_max(
            pos2(rect.left() + pad, rect.top() + 122.0),
            pos2(rect.right() - pad, rect.bottom() - 90.0),
        );
        let bh = (area.height() - 16.0) / 2.0;
        let raise = Rect::from_min_size(area.min, vec2(area.width(), bh));
        let lower = Rect::from_min_size(
            pos2(area.left(), raise.bottom() + 16.0),
            vec2(area.width(), bh),
        );
        let up = hold_button(
            ui,
            raise,
            Id::new("raise"),
            egui_phosphor::bold::CARET_UP,
            "Raise",
            p.primary,
            p.on_primary,
            enabled,
        );
        let down = hold_button(
            ui,
            lower,
            Id::new("lower"),
            egui_phosphor::bold::CARET_DOWN,
            "Lower",
            p.tint,
            p.on_tint,
            enabled,
        );

        let keys = ui.ctx().egui_wants_keyboard_input() || self.settings_open;
        let key_up = !keys && ui.input(|i| i.key_down(Key::ArrowUp));
        let key_down = !keys && ui.input(|i| i.key_down(Key::ArrowDown));
        if enabled {
            let want = if up.is_pointer_button_down_on() || key_up {
                Some(Direction::Up)
            } else if down.is_pointer_button_down_on() || key_down {
                Some(Direction::Down)
            } else {
                None
            };
            // Releasing a hold stops; a running "move to" is left alone.
            if want.is_some() || matches!(self.motion, Motion::Jogging(_)) {
                self.set_jog(want);
            }
        }
        up.on_hover_text("Hold, or hold ↑");
        down.on_hover_text("Hold, or hold ↓");

        let painter = ui.painter();
        let line_y = rect.bottom() - 70.0;
        painter.line_segment(
            [
                pos2(rect.left() + pad, line_y),
                pos2(rect.right() - pad, line_y),
            ],
            Stroke::new(1.0, p.divider),
        );
        let ty = rect.bottom() - 38.0;
        painter.text(
            pos2(rect.left() + pad, ty),
            Align2::LEFT_CENTER,
            "Range",
            font(17.5, Weight::Regular),
            p.muted,
        );
        let range = format!(
            "{:.0}–{:.0} cm",
            self.base_mm() / 10.0,
            self.max_mm() / 10.0
        );
        painter.text(
            pos2(rect.right() - pad, ty),
            Align2::RIGHT_CENTER,
            range,
            font(17.5, Weight::Regular),
            p.muted,
        );
    }

    fn favourites(&mut self, ui: &mut Ui, rect: Rect, item_h: f32, p: &Palette) {
        card(ui, rect, p);
        let pad = 24.0;
        let title_y = rect.top() + 40.0;
        ui.painter().text(
            pos2(rect.left() + 28.0, title_y),
            Align2::LEFT_CENTER,
            "Favourites",
            font(26.0, Weight::SemiBold),
            p.text,
        );
        let now = ui.input(|i| i.time);

        let current = self.height_mm();
        let save = link_button(
            ui,
            pos2(rect.right() - 28.0, title_y),
            icon::PLUS,
            "Save current height",
            17.0,
            p.text,
            Id::new("save_fav"),
        );
        if save.clicked() {
            match current {
                Some(h) => {
                    let name = format!("Favourite {}", self.cfg.favourites.len() + 1);
                    self.cfg.favourites.push(Favourite {
                        name: name.clone(),
                        height_mm: h.round(),
                        icon: FavIcon::Star,
                    });
                    self.cfg.save();
                    self.toast(format!("Saved {:.1} cm as “{name}”", h / 10.0), now);
                }
                None => self.toast("Connect to the desk first", now),
            }
        }

        let grid = Rect::from_min_max(
            pos2(rect.left() + pad, rect.top() + 74.0),
            pos2(rect.right() - pad, rect.bottom() - 22.0),
        );
        if self.cfg.favourites.is_empty() {
            ui.painter().text(
                grid.center(),
                Align2::CENTER_CENTER,
                "Save a height to add it here",
                font(16.0, Weight::Regular),
                p.muted,
            );
            return;
        }
        let gap = 16.0;
        let w = (grid.width() - 2.0 * gap) / 3.0;
        let mut action: Option<(usize, FavAction)> = None;

        for (i, fav) in self.cfg.favourites.iter_mut().enumerate() {
            let col = (i % 3) as f32;
            let row = (i / 3) as f32;
            let r = Rect::from_min_size(
                pos2(
                    grid.left() + col * (w + gap),
                    grid.top() + row * (item_h + 14.0),
                ),
                vec2(w, item_h),
            );
            ui.painter().rect(
                r,
                14,
                p.card,
                Stroke::new(1.0, p.border),
                StrokeKind::Inside,
            );
            let painter = ui.painter();
            painter.text(
                pos2(r.left() + 42.0, r.top() + 50.0),
                Align2::CENTER_CENTER,
                fav.icon.glyph(),
                theme::icon_font(40.0),
                p.text,
            );
            painter.text(
                pos2(r.left() + 84.0, r.top() + 22.0),
                Align2::LEFT_TOP,
                &fav.name,
                font(17.5, Weight::Medium),
                p.text,
            );
            painter.text(
                pos2(r.left() + 84.0, r.top() + 50.0),
                Align2::LEFT_TOP,
                format!("{:.1} cm", fav.height_mm / 10.0),
                font(20.0, Weight::Regular),
                p.text,
            );

            let dots = icon_button(
                ui,
                pos2(r.right() - 28.0, r.top() + 34.0),
                22.0,
                icon::DOTS_THREE_VERTICAL,
                p.text,
                Id::new(("fav_menu", i)),
            );
            Popup::menu(&dots)
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                .width(240.0)
                .show(|ui| {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Name").color(p.muted).size(13.0));
                    if ui.text_edit_singleline(&mut fav.name).changed() {
                        action = Some((i, FavAction::Save));
                    }
                    ui.label(egui::RichText::new("Icon").color(p.muted).size(13.0));
                    ui.horizontal(|ui| {
                        for ic in FavIcon::ALL {
                            let selected = fav.icon == ic;
                            let resp = ui.add(egui::Button::selectable(
                                selected,
                                egui::RichText::new(ic.glyph()).font(theme::icon_font(20.0)),
                            ));
                            if resp.clicked() {
                                fav.icon = ic;
                                action = Some((i, FavAction::Save));
                            }
                        }
                    });
                    ui.separator();
                    if ui
                        .add_enabled(
                            current.is_some(),
                            egui::Button::new(theme::icon_text(
                                icon::ARROWS_CLOCKWISE,
                                "Set to current height",
                                font(15.0, Weight::Regular),
                                p.text,
                                false,
                            )),
                        )
                        .clicked()
                    {
                        action = Some((i, FavAction::SetCurrent));
                        ui.close();
                    }
                    if ui
                        .button(theme::icon_text(
                            icon::TRASH,
                            "Delete",
                            font(15.0, Weight::Regular),
                            p.danger,
                            false,
                        ))
                        .clicked()
                    {
                        action = Some((i, FavAction::Delete));
                        ui.close();
                    }
                });

            let here = current.is_some_and(|h| (h - fav.height_mm).abs() < 5.0)
                && self.motion == Motion::Idle;
            let moving_here = self.motion == Motion::MovingTo(fav.name.clone());
            let label = if moving_here {
                "Moving…"
            } else if here {
                "You're here"
            } else {
                "Move to"
            };
            let btn = Rect::from_min_max(
                pos2(r.left() + 20.0, r.bottom() - 20.0 - 48.0),
                pos2(r.right() - 20.0, r.bottom() - 20.0),
            );
            let resp = Button::tinted(label, p)
                .font(font(17.0, Weight::Medium))
                .enabled(self.conn == Conn::Connected && !here && !moving_here)
                .show_at(ui, btn, Id::new(("fav_move", i)));
            if resp.clicked() {
                action = Some((i, FavAction::Move));
            }
        }

        if let Some((i, a)) = action {
            match a {
                FavAction::Move => {
                    let fav = self.cfg.favourites[i].clone();
                    self.move_to_mm(fav.height_mm, fav.name);
                }
                FavAction::Save => self.cfg.save(),
                FavAction::SetCurrent => {
                    if let Some(h) = current {
                        self.cfg.favourites[i].height_mm = h.round();
                        self.cfg.save();
                    }
                }
                FavAction::Delete => {
                    let removed = self.cfg.favourites.remove(i);
                    self.cfg.save();
                    self.toast(format!("Deleted “{}”", removed.name), now);
                }
            }
        }
    }

    // ---- Settings -------------------------------------------------------

    fn settings(&mut self, ctx: &egui::Context, p: &Palette) {
        if !self.settings_open {
            return;
        }
        let frame = egui::Frame::new()
            .fill(p.card)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(18)
            .inner_margin(28);
        let modal = egui::Modal::new(Id::new("settings")).frame(frame).show(ctx, |ui| {
            ui.set_width(480.0);
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Settings").font(font(26.0, Weight::SemiBold)));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(icon::X).font(theme::icon_font(20.0))).frame(false)).clicked() {
                        self.settings_open = false;
                    }
                });
            });
            ui.add_space(14.0);

            section(ui, "Appearance", p);
            ui.label("Theme");
            let idx = self.cfg.theme as usize;
            if let Some(i) = widgets::segmented(ui, &["System", "Light", "Dark"], idx, 300.0, p) {
                self.cfg.theme = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark][i];
                changed = true;
            }
            ui.add_space(8.0);
            ui.label("Primary color · light theme");
            changed |= widgets::color_choice(ui, &mut self.cfg.primary_light, &LIGHT_PRESETS, p);
            ui.add_space(6.0);
            ui.label("Primary color · dark theme");
            changed |= widgets::color_choice(ui, &mut self.cfg.primary_dark, &DARK_PRESETS, p);

            ui.add_space(16.0);
            section(ui, "Tray", p);
            // Screenshot runs skip the tray but should still show the options.
            if self.tray.is_some() || self.dev.capturing() {
                changed |= widgets::toggle(ui, &mut self.cfg.minimize_to_tray, "Minimize to the tray", p);
                changed |= widgets::toggle(
                    ui,
                    &mut self.cfg.close_to_tray,
                    "Keep running in the tray when the window is closed",
                    p,
                );
            } else {
                ui.label(egui::RichText::new("No system tray is available on this desktop.").color(p.muted));
            }
            ui.add_space(16.0);
            section(ui, "Desk", p);
            if let Some(desk) = &self.cfg.desk {
                let model = self.info.as_ref().map(|i| format!("  ·  {} {}", i.manufacturer, i.model)).unwrap_or_default();
                ui.label(egui::RichText::new(format!("{}  ·  {}{model}", desk.name, desk.id)).color(p.muted));
            }
            let mut base_cm = self.base_mm() / 10.0;
            let mut max_cm = self.max_mm() / 10.0;
            egui::Grid::new("calib").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
                ui.label("Lowest position");
                if ui.add(egui::DragValue::new(&mut base_cm).range(30.0..=150.0).speed(0.1).suffix(" cm").fixed_decimals(1)).changed() {
                    self.cfg.base_height_mm = Some(base_cm * 10.0);
                    changed = true;
                }
                ui.end_row();
                ui.label("Highest position");
                if ui.add(egui::DragValue::new(&mut max_cm).range(base_cm..=200.0).speed(0.1).suffix(" cm").fixed_decimals(1)).changed() {
                    self.cfg.max_height_mm = Some(max_cm * 10.0);
                    changed = true;
                }
                ui.end_row();
            });
            ui.label(egui::RichText::new("Lowest position is the floor-to-tabletop height when fully lowered. All heights are measured from it.").color(p.muted).size(13.0));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                if Button::tinted("Run setup again", p).font(font(15.0, Weight::Medium)).show(ui, vec2(170.0, 38.0)).clicked() {
                    self.settings_open = false;
                    self.onboarding = Onboarding::default();
                    self.screen = Screen::Onboarding;
                }
                let forget = Button::new("Forget desk", Color32::TRANSPARENT, p.danger).font(font(15.0, Weight::Medium)).show(ui, vec2(130.0, 38.0));
                if forget.clicked() {
                    self.cfg.desk = None;
                    self.cfg.base_height_mm = None;
                    self.cfg.max_height_mm = None;
                    self.worker.send(Command::Disconnect);
                    self.conn = Conn::Idle;
                    self.settings_open = false;
                    self.onboarding = Onboarding::default();
                    self.screen = Screen::Onboarding;
                    changed = true;
                }
            });
            if changed {
                self.cfg.save();
            }
        });
        if modal.should_close() {
            self.settings_open = false;
        }
    }

    fn paint_toast(&mut self, ui: &mut Ui, p: &Palette) {
        let Some((msg, at)) = &self.toast else { return };
        let now = ui.input(|i| i.time);
        let age = (now - at) as f32;
        if age > 4.0 {
            self.toast = None;
            return;
        }
        let a = (4.0 - age).min(0.3) / 0.3;
        let screen = ui.max_rect();
        let galley =
            ui.painter()
                .layout_no_wrap(msg.clone(), font(15.0, Weight::Medium), p.on_primary);
        let rect = Rect::from_center_size(
            pos2(screen.center().x, screen.bottom() - 48.0),
            galley.size() + vec2(36.0, 22.0),
        );
        let painter = ui
            .ctx()
            .layer_painter(egui::LayerId::new(egui::Order::Tooltip, Id::new("toast")));
        painter.rect_filled(
            rect.translate(vec2(0.0, 3.0)),
            14,
            alpha(Color32::BLACK, 0.12 * a),
        );
        painter.rect_filled(rect, 14, alpha(p.primary, a));
        painter.galley(rect.min + vec2(18.0, 11.0), galley, alpha(p.on_primary, a));
        ui.ctx().request_repaint();
    }
}

enum FavAction {
    Move,
    Save,
    SetCurrent,
    Delete,
}

/// Y offset of the first line's baseline within a galley.
fn baseline(g: &egui::Galley) -> f32 {
    g.rows
        .first()
        .and_then(|r| r.glyphs.first().map(|gl| r.pos.y + gl.pos.y))
        .unwrap_or(g.size().y)
}

/// Letter-spaced single line of text.
pub fn spaced(
    ui: &Ui,
    text: &str,
    font_id: FontId,
    color: Color32,
    spacing: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::default();
    job.append(
        text,
        0.0,
        TextFormat {
            font_id,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    ui.painter().layout_job(job)
}

pub fn section(ui: &mut Ui, title: &str, p: &Palette) {
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .font(font(12.5, Weight::SemiBold))
            .color(p.muted),
    );
    ui.add_space(2.0);
}

impl eframe::App for DeskApp {
    /// Also runs while the window is hidden in the tray.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.process_events(now);
        self.window_and_tray(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        let p = self.palette(&ctx);
        if self.applied_palette != Some(p) {
            theme::apply_style(&ctx, &p);
            self.applied_palette = Some(p);
        }
        ui.painter().rect_filled(ui.max_rect(), 0, p.page);

        match self.screen {
            Screen::Main => self.main_view(ui, &p),
            Screen::Onboarding => self.onboarding_view(ui, &p),
        }
        self.settings(&ctx, &p);
        self.paint_toast(ui, &p);
        if self.screen == Screen::Onboarding {
            self.autopilot();
        }
        if self.sample.is_some()
            && self.connected()
            && let Some(mm) = self.dev.move_to.take()
        {
            self.move_to_mm(mm, format!("{:.1} cm", mm / 10.0));
        }
        self.dev.tick(&ctx);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }
}
