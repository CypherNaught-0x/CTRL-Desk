//! System tray icon with a small menu: show/hide, live status, favourites,
//! stop and quit. The glyph follows the taskbar/panel theme (on macOS it is a
//! template image that the system tints).

use std::sync::mpsc;

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::icon::{self, Style};

const TRAY_SIZE: u32 = 64;
const THEME_CHECK_SECS: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayAction {
    ToggleWindow,
    MoveTo(usize),
    Stop,
    Quit,
}

/// What the tray should currently show.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayState {
    pub window_visible: bool,
    /// e.g. "104.5 cm · Connected".
    pub status: String,
    /// Menu labels for the favourites, in order.
    pub favourites: Vec<String>,
    pub can_move: bool,
}

enum Message {
    Icon(TrayIconEvent),
    Menu(MenuEvent),
}

pub struct Tray {
    icon: TrayIcon,
    messages: mpsc::Receiver<Message>,
    actions: Vec<(MenuId, TrayAction)>,
    toggle_item: MenuItem,
    status_item: MenuItem,
    stop_item: MenuItem,
    favourite_items: Vec<MenuItem>,
    shown: Option<TrayState>,
    /// Whether the dark glyph (for light taskbars) is currently shown.
    dark_glyph: Option<bool>,
    last_theme_check: f64,
}

impl Tray {
    /// Creates the tray icon. Returns `None` where no tray is available (for
    /// example a Linux desktop without a StatusNotifier host).
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        let (tx, messages) = mpsc::channel();
        let (tx2, ctx2) = (tx.clone(), ctx.clone());
        let ctx1 = ctx.clone();
        // The handlers run on the platform's event thread; wake egui so the
        // event is handled even while the window is hidden.
        TrayIconEvent::set_event_handler(Some(move |e| {
            let _ = tx.send(Message::Icon(e));
            ctx1.request_repaint();
        }));
        MenuEvent::set_event_handler(Some(move |e| {
            let _ = tx2.send(Message::Menu(e));
            ctx2.request_repaint();
        }));

        let toggle_item = MenuItem::new("Hide CTRL Desk", true, None);
        let status_item = MenuItem::new("Connecting…", false, None);
        let stop_item = MenuItem::new("Stop", false, None);
        let dark = taskbar_is_light(ctx);
        let builder = TrayIconBuilder::new()
            .with_tooltip("CTRL Desk")
            // macOS convention: the menu opens on left click. Elsewhere a left
            // click shows/hides the window and the menu is on right click.
            .with_menu_on_left_click(cfg!(target_os = "macos"));
        // On macOS a template image is tinted by the system to match the menu bar.
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(glyph(dark));
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(glyph(dark));
        let icon = builder
            .build()
            .map_err(|e| eprintln!("tray icon unavailable: {e}"))
            .ok()?;

        let mut tray = Self {
            icon,
            messages,
            actions: Vec::new(),
            toggle_item,
            status_item,
            stop_item,
            favourite_items: Vec::new(),
            shown: None,
            dark_glyph: Some(dark),
            last_theme_check: 0.0,
        };
        tray.rebuild_menu(&[]);
        Some(tray)
    }

    fn rebuild_menu(&mut self, favourites: &[String]) {
        let menu = Menu::new();
        self.favourite_items = favourites
            .iter()
            .map(|f| MenuItem::new(f, false, None))
            .collect();
        self.actions = vec![
            (self.toggle_item.id().clone(), TrayAction::ToggleWindow),
            (self.stop_item.id().clone(), TrayAction::Stop),
        ];
        let quit = MenuItem::new("Quit", true, None);
        self.actions.push((quit.id().clone(), TrayAction::Quit));
        for (i, item) in self.favourite_items.iter().enumerate() {
            self.actions
                .push((item.id().clone(), TrayAction::MoveTo(i)));
        }

        let _ = menu.append(&self.toggle_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&self.status_item);
        if !self.favourite_items.is_empty() {
            let _ = menu.append(&PredefinedMenuItem::separator());
            for item in &self.favourite_items {
                let _ = menu.append(item);
            }
        }
        let _ = menu.append(&self.stop_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit);
        self.icon.set_menu(Some(Box::new(menu)));
    }

    /// Updates labels, enabled states, tooltip and theme. Cheap to call every
    /// frame: only changes are pushed to the platform.
    pub fn sync(&mut self, ctx: &egui::Context, state: TrayState) {
        let now = ctx.input(|i| i.time);
        if now - self.last_theme_check >= THEME_CHECK_SECS {
            self.last_theme_check = now;
            let dark = taskbar_is_light(ctx);
            if self.dark_glyph != Some(dark) {
                let _ = self.icon.set_icon(Some(glyph(dark)));
                self.dark_glyph = Some(dark);
            }
            // Keep polling the theme while the window is hidden.
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(THEME_CHECK_SECS));
        }

        let prev = self.shown.take();
        if prev.as_ref().map(|p| &p.favourites) != Some(&state.favourites) {
            self.rebuild_menu(&state.favourites);
        }
        let fresh =
            prev.is_none() || prev.as_ref().map(|p| &p.favourites) != Some(&state.favourites);
        let changed =
            |f: fn(&TrayState) -> bool| fresh || prev.as_ref().is_none_or(|p| f(p) != f(&state));

        if changed(|s| s.window_visible) {
            self.toggle_item.set_text(if state.window_visible {
                "Hide CTRL Desk"
            } else {
                "Show CTRL Desk"
            });
        }
        if fresh || prev.as_ref().is_none_or(|p| p.status != state.status) {
            self.status_item.set_text(&state.status);
            let _ = self
                .icon
                .set_tooltip(Some(format!("CTRL Desk · {}", state.status)));
        }
        if changed(|s| s.can_move) {
            for item in &self.favourite_items {
                item.set_enabled(state.can_move);
            }
            self.stop_item.set_enabled(state.can_move);
        }
        self.shown = Some(state);
    }

    /// Drains pending clicks and menu selections.
    pub fn poll(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();
        for msg in self.messages.try_iter() {
            match msg {
                Message::Icon(TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }) if !cfg!(target_os = "macos") => out.push(TrayAction::ToggleWindow),
                Message::Menu(e) => {
                    if let Some((_, a)) = self.actions.iter().find(|(id, _)| *id == e.id) {
                        out.push(*a);
                    }
                }
                _ => {}
            }
        }
        out
    }
}

fn glyph(dark: bool) -> tray_icon::Icon {
    let color = if dark {
        icon::GLYPH_DARK
    } else {
        icon::GLYPH_LIGHT
    };
    let rgba = icon::render(TRAY_SIZE, &Style::Glyph(color));
    tray_icon::Icon::from_rgba(rgba, TRAY_SIZE, TRAY_SIZE).expect("valid tray icon")
}

/// Whether the taskbar/panel is light, so the tray needs the dark glyph.
fn taskbar_is_light(ctx: &egui::Context) -> bool {
    #[cfg(windows)]
    if let Some(light) = windows_taskbar_is_light() {
        return light;
    }
    // macOS tints the template image itself. Elsewhere, follow the system
    // theme; most Linux panels match it.
    cfg!(target_os = "macos") || ctx.system_theme() != Some(egui::Theme::Dark)
}

/// The Windows taskbar follows the "system" color mode, which can differ from
/// the app mode, so read it from the registry.
#[cfg(windows)]
fn windows_taskbar_is_light() -> Option<bool> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("SystemUsesLightTheme");
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: valid NUL-terminated strings and a DWORD-sized output buffer.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut data as *mut u32).cast(),
            &mut size,
        )
    };
    (status == 0).then_some(data != 0)
}
