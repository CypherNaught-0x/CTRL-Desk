//! Colors, fonts and egui style. Every accent color is derived from a single
//! user-configurable primary color.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub page: Color32,
    pub frame: Color32,
    pub card: Color32,
    pub border: Color32,
    pub divider: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// Main accent: Raise button, selected states.
    pub primary: Color32,
    pub on_primary: Color32,
    /// Primary washed into the card color: Lower / Move to buttons.
    pub tint: Color32,
    pub on_tint: Color32,
    /// Status indicator dot.
    pub dot: Color32,
    pub pill: Color32,
    pub danger: Color32,
    /// Hero background: wall top/bottom, floor far/near.
    pub wall: [Color32; 2],
    pub floor: [Color32; 2],
}

impl Palette {
    pub fn new(dark: bool, primary: Color32) -> Self {
        if dark {
            let card = hex(0x1c1c1c);
            Self {
                dark,
                page: hex(0x0f0f0f),
                frame: hex(0x161616),
                card,
                border: hex(0x2b2b2b),
                divider: hex(0x323232),
                text: hex(0xf3f3f3),
                muted: hex(0xa3a3a3),
                primary,
                on_primary: on_color(primary),
                tint: mix(card, primary, 0.13),
                on_tint: hex(0xf3f3f3),
                dot: mix(primary, hex(0xffffff), 0.3),
                pill: hex(0x262626),
                danger: hex(0xe5735c),
                wall: [hex(0x242424), hex(0x1d1d1d)],
                floor: [hex(0x141414), hex(0x1b1b1b)],
            }
        } else {
            let card = hex(0xf8f6f2);
            let text = hex(0x1a1d1b);
            Self {
                dark,
                page: hex(0xeeebe4),
                frame: hex(0xf6f4ef),
                card,
                border: hex(0xe3dfd6),
                divider: hex(0xe3dfd6),
                text,
                muted: hex(0x5f6460),
                primary,
                on_primary: on_color(primary),
                tint: mix(card, primary, 0.15),
                on_tint: mix(text, primary, 0.3),
                dot: mix(primary, hex(0xffffff), 0.25),
                pill: hex(0xe9e6e0),
                danger: hex(0xb4442e),
                wall: [hex(0xf3f1ec), hex(0xeceae4)],
                floor: [hex(0xe6e3dc), hex(0xf0eee9)],
            }
        }
    }

    pub fn shadow(&self) -> Color32 {
        if self.dark {
            Color32::from_black_alpha(90)
        } else {
            Color32::from_black_alpha(28)
        }
    }
}

pub const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub fn from_rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Linear mix in sRGB space: `t = 0` gives `a`, `t = 1` gives `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

pub fn luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// Readable text color on top of `bg`.
pub fn on_color(bg: Color32) -> Color32 {
    if luminance(bg) > 0.4 {
        hex(0x262626)
    } else {
        hex(0xffffff)
    }
}

/// Slightly lighter/darker version for hover and press feedback.
pub fn interact(c: Color32, hovered: bool, pressed: bool) -> Color32 {
    let toward = if luminance(c) > 0.4 {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    let t = if pressed {
        0.14
    } else if hovered {
        0.07
    } else {
        0.0
    };
    mix(c, toward, t)
}

pub fn alpha(c: Color32, a: f32) -> Color32 {
    c.gamma_multiply(a)
}

#[derive(Clone, Copy)]
pub enum Weight {
    Regular,
    Medium,
    SemiBold,
    Bold,
}

impl Weight {
    fn family_name(self) -> &'static str {
        match self {
            Weight::Regular => "regular",
            Weight::Medium => "medium",
            Weight::SemiBold => "semibold",
            Weight::Bold => "bold",
        }
    }
}

pub fn font(size: f32, w: Weight) -> FontId {
    FontId::new(size, FontFamily::Name(w.family_name().into()))
}

pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

/// Bold Phosphor icons (chevrons on the big buttons).
pub fn icon_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons-bold".into()))
}

/// An icon followed (or, with `trailing`, preceded) by text, vertically centered.
pub fn icon_text(
    icon: &str,
    text: &str,
    text_font: FontId,
    color: Color32,
    trailing: bool,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let icon_fmt = egui::TextFormat {
        font_id: icon_font(text_font.size * 1.15),
        color,
        valign: egui::Align::Center,
        ..Default::default()
    };
    let text_fmt = egui::TextFormat {
        font_id: text_font,
        color,
        valign: egui::Align::Center,
        ..Default::default()
    };
    if trailing {
        job.append(text, 0.0, text_fmt.clone());
        job.append(icon, 8.0, icon_fmt);
    } else {
        job.append(icon, 0.0, icon_fmt);
        if !text.is_empty() {
            job.append(text, 8.0, text_fmt);
        }
    }
    job
}

const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

/// Installs the bundled Inter fonts (SIL OFL 1.1, see `assets/fonts/OFL.txt`)
/// and Phosphor icons, so the app looks the same on every platform.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let defaults = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    // Inter Bold is not bundled; SemiBold is heavy enough at display sizes.
    let weights = [
        (Weight::Regular, INTER_REGULAR),
        (Weight::Medium, INTER_MEDIUM),
        (Weight::SemiBold, INTER_SEMIBOLD),
        (Weight::Bold, INTER_SEMIBOLD),
    ];
    for (weight, bytes) in weights {
        let name = weight.family_name();
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
        let mut family = defaults.clone();
        family.insert(0, name.into());
        fonts.families.insert(FontFamily::Name(name.into()), family);
    }
    if let Some(regular) = fonts
        .families
        .get(&FontFamily::Name("regular".into()))
        .cloned()
    {
        fonts.families.insert(FontFamily::Proportional, regular);
    }

    // Icons get their own families: text fonts like Inter define glyphs in the
    // private-use area Phosphor uses, so the two must not share a family.
    for (name, variant) in [
        ("icons", egui_phosphor::Variant::Regular),
        ("icons-bold", egui_phosphor::Variant::Bold),
    ] {
        fonts.font_data.insert(
            name.into(),
            egui::FontData::from_static(variant.font_bytes()).into(),
        );
        fonts
            .families
            .insert(FontFamily::Name(name.into()), vec![name.into()]);
    }
    ctx.set_fonts(fonts);
}

/// Styles stock egui widgets (text fields, drag values, popups, color picker)
/// to match the palette.
pub fn apply_style(ctx: &egui::Context, p: &Palette) {
    let theme = if p.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(theme);
    let mut style = (*ctx.style_of(theme)).clone();
    let v = &mut style.visuals;
    *v = if p.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.page;
    v.window_fill = p.card;
    v.extreme_bg_color = if p.dark { hex(0x141414) } else { hex(0xffffff) };
    v.faint_bg_color = p.frame;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(12);
    v.override_text_color = Some(p.text);
    v.selection.bg_fill = alpha(p.primary, 0.35);
    v.selection.stroke = Stroke::new(1.0, p.primary);
    v.hyperlink_color = p.primary;
    for (w, fill) in [
        (&mut v.widgets.noninteractive, p.card),
        (&mut v.widgets.inactive, p.tint),
        (&mut v.widgets.hovered, interact(p.tint, true, false)),
        (&mut v.widgets.active, interact(p.tint, true, true)),
        (&mut v.widgets.open, p.tint),
    ] {
        w.bg_fill = fill;
        w.weak_bg_fill = fill;
        w.corner_radius = CornerRadius::same(8);
        w.fg_stroke.color = p.text;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, alpha(p.primary, 0.5));
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.primary);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.interact_size.y = 30.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, font(15.0, Weight::Regular));
    style
        .text_styles
        .insert(egui::TextStyle::Button, font(15.0, Weight::Medium));
    style
        .text_styles
        .insert(egui::TextStyle::Small, font(12.0, Weight::Regular));
    style
        .text_styles
        .insert(egui::TextStyle::Heading, font(22.0, Weight::SemiBold));
    ctx.set_style_of(theme, style);
}
