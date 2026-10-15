//! Custom painted widgets matching the design mockups.

use egui::{
    Align2, Color32, CursorIcon, FontId, Id, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, pos2, vec2,
};

use crate::theme::{Palette, Weight, alpha, font, interact};

pub fn card(ui: &Ui, rect: Rect, p: &Palette) {
    ui.painter().rect(
        rect,
        16,
        p.card,
        Stroke::new(1.0, p.border),
        StrokeKind::Inside,
    );
}

/// A filled, rounded button. `icon` is drawn before the text.
pub struct Button<'a> {
    pub text: &'a str,
    pub icon: Option<&'a str>,
    pub trailing_icon: bool,
    pub font: FontId,
    pub fill: Color32,
    pub fg: Color32,
    pub radius: u8,
    pub enabled: bool,
}

impl<'a> Button<'a> {
    pub fn new(text: &'a str, fill: Color32, fg: Color32) -> Self {
        Self {
            text,
            icon: None,
            trailing_icon: false,
            font: font(16.0, Weight::Medium),
            fill,
            fg,
            radius: 12,
            enabled: true,
        }
    }
    pub fn primary(text: &'a str, p: &Palette) -> Self {
        Self::new(text, p.primary, p.on_primary)
    }
    pub fn tinted(text: &'a str, p: &Palette) -> Self {
        Self::new(text, p.tint, p.on_tint)
    }
    /// Transparent background, only visible on hover.
    pub fn ghost(text: &'a str, p: &Palette) -> Self {
        Self::new(text, Color32::TRANSPARENT, p.text)
    }
    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn trailing_icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self.trailing_icon = true;
        self
    }
    pub fn font(mut self, font: FontId) -> Self {
        self.font = font;
        self
    }
    pub fn radius(mut self, r: u8) -> Self {
        self.radius = r;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Adds the button in the current layout.
    pub fn show(self, ui: &mut Ui, size: Vec2) -> Response {
        let (rect, resp) = ui.allocate_exact_size(size, self.sense());
        self.paint(ui, rect, resp)
    }

    /// Places the button at an explicit position.
    pub fn show_at(self, ui: &mut Ui, rect: Rect, id: Id) -> Response {
        let resp = ui.interact(rect, id, self.sense());
        self.paint(ui, rect, resp)
    }

    fn sense(&self) -> Sense {
        if self.enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        }
    }

    fn paint(self, ui: &Ui, rect: Rect, resp: Response) -> Response {
        let pressed = resp.is_pointer_button_down_on();
        let hovered = resp.hovered() && self.enabled;
        let mut fill = if self.fill == Color32::TRANSPARENT {
            if hovered {
                alpha(self.fg, 0.07)
            } else {
                Color32::TRANSPARENT
            }
        } else {
            interact(self.fill, hovered, pressed)
        };
        let mut fg = self.fg;
        if !self.enabled {
            fill = alpha(fill, 0.45);
            fg = alpha(fg, 0.45);
        }
        let painter = ui.painter();
        painter.rect_filled(rect, self.radius, fill);

        let job = match self.icon {
            Some(icon) => {
                crate::theme::icon_text(icon, self.text, self.font, fg, self.trailing_icon)
            }
            None => egui::text::LayoutJob::simple_singleline(self.text.to_string(), self.font, fg),
        };
        let galley = painter.layout_job(job);
        painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
        if self.enabled {
            resp.on_hover_cursor(CursorIcon::PointingHand)
        } else {
            resp
        }
    }
}

/// The large press-and-hold Raise / Lower buttons.
#[allow(clippy::too_many_arguments)]
pub fn hold_button(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    icon: &str,
    label: &str,
    fill: Color32,
    fg: Color32,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(rect, id, sense);
    let pressed = enabled && resp.is_pointer_button_down_on();
    let (mut fill, mut fg) = (interact(fill, resp.hovered() && enabled, pressed), fg);
    if !enabled {
        fill = alpha(fill, 0.45);
        fg = alpha(fg, 0.45);
    }
    let painter = ui.painter();
    // A pressed button sinks slightly.
    let r = if pressed { rect.shrink(1.5) } else { rect };
    painter.rect_filled(r, 14, fill);
    let icon_size = (r.height() * 0.36).clamp(24.0, 52.0);
    let label_size = (r.height() * 0.2).clamp(17.0, 28.0);
    let gap = icon_size * 0.2;
    let total = icon_size + gap + label_size;
    let top = r.center().y - total / 2.0;
    painter.text(
        pos2(r.center().x, top + icon_size / 2.0),
        Align2::CENTER_CENTER,
        icon,
        crate::theme::icon_bold(icon_size),
        fg,
    );
    painter.text(
        pos2(r.center().x, top + icon_size + gap + label_size / 2.0),
        Align2::CENTER_CENTER,
        label,
        font(label_size, Weight::SemiBold),
        fg,
    );
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// A round icon-only button (gear, overflow menu).
pub fn icon_button(
    ui: &mut Ui,
    center: Pos2,
    size: f32,
    icon: &str,
    color: Color32,
    id: Id,
) -> Response {
    let rect = Rect::from_center_size(center, Vec2::splat(size + 12.0));
    let resp = ui.interact(rect, id, Sense::click());
    if resp.hovered() {
        ui.painter()
            .circle_filled(center, rect.width() / 2.0, alpha(color, 0.08));
    }
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        icon,
        crate::theme::icon_font(size),
        color,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Text-with-icon link; `anchor` is the right-center point.
pub fn link_button(
    ui: &mut Ui,
    anchor: Pos2,
    icon: &str,
    text: &str,
    size: f32,
    color: Color32,
    id: Id,
) -> Response {
    let galley = ui.painter().layout_job(crate::theme::icon_text(
        icon,
        text,
        font(size, Weight::Medium),
        color,
        false,
    ));
    let rect = Rect::from_min_size(
        pos2(anchor.x - galley.size().x, anchor.y - galley.size().y / 2.0),
        galley.size(),
    )
    .expand2(vec2(8.0, 6.0));
    let resp = ui.interact(rect, id, Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, 8, alpha(color, 0.07));
    }
    ui.painter()
        .galley(rect.shrink2(vec2(8.0, 6.0)).min, galley, color);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// A rotating arc spinner.
pub fn spinner(ui: &Ui, center: Pos2, radius: f32, color: Color32) {
    let t = ui.input(|i| i.time) as f32;
    let start = t * 5.0;
    let points: Vec<Pos2> = (0..=24)
        .map(|i| {
            let a = start + i as f32 / 24.0 * std::f32::consts::PI * 1.4;
            center + radius * vec2(a.cos(), a.sin())
        })
        .collect();
    ui.painter()
        .add(Shape::line(points, Stroke::new(radius * 0.22, color)));
    ui.ctx().request_repaint();
}

/// Four signal-strength bars for an RSSI value.
pub fn signal_bars(ui: &Ui, rect: Rect, rssi: Option<i16>, on: Color32, off: Color32) {
    let level = match rssi {
        Some(r) if r >= -60 => 4,
        Some(r) if r >= -70 => 3,
        Some(r) if r >= -80 => 2,
        Some(_) => 1,
        None => 0,
    };
    let w = rect.width() / 4.0;
    for i in 0..4 {
        let h = rect.height() * (i + 1) as f32 / 4.0;
        let bar = Rect::from_min_max(
            pos2(rect.left() + i as f32 * w + 1.0, rect.bottom() - h),
            pos2(rect.left() + (i + 1) as f32 * w - 1.0, rect.bottom()),
        );
        ui.painter()
            .rect_filled(bar, 1, if i < level { on } else { off });
    }
}

/// A segmented control. Returns the newly selected index if it changed.
pub fn segmented(
    ui: &mut Ui,
    options: &[&str],
    selected: usize,
    width: f32,
    p: &Palette,
) -> Option<usize> {
    let height = 38.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter().rect_filled(rect, 10, p.pill);
    let seg_w = rect.width() / options.len() as f32;
    let mut changed = None;
    for (i, label) in options.iter().enumerate() {
        let seg = Rect::from_min_size(
            pos2(rect.left() + i as f32 * seg_w, rect.top()),
            vec2(seg_w, height),
        )
        .shrink(3.0);
        let resp = ui.interact(seg, ui.id().with(("seg", label)), Sense::click());
        if i == selected {
            ui.painter().rect(
                seg,
                8,
                p.card,
                Stroke::new(1.0, p.border),
                StrokeKind::Inside,
            );
        } else if resp.hovered() {
            ui.painter().rect_filled(seg, 8, alpha(p.text, 0.05));
        }
        let color = if i == selected { p.text } else { p.muted };
        ui.painter().text(
            seg.center(),
            Align2::CENTER_CENTER,
            *label,
            font(14.5, Weight::Medium),
            color,
        );
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() && i != selected {
            changed = Some(i);
        }
    }
    changed
}

/// Row of color swatches plus a custom color picker. Returns true on change.
pub fn color_choice(ui: &mut Ui, color: &mut [u8; 3], presets: &[[u8; 3]], p: &Palette) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        for preset in presets {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
            let c = crate::theme::from_rgb(*preset);
            ui.painter().circle_filled(rect.center(), 13.0, c);
            ui.painter()
                .circle_stroke(rect.center(), 13.0, Stroke::new(1.0, alpha(p.text, 0.15)));
            if *preset == *color {
                ui.painter()
                    .circle_stroke(rect.center(), 16.5, Stroke::new(2.0, p.text));
            } else if resp.hovered() {
                ui.painter().circle_stroke(
                    rect.center(),
                    16.5,
                    Stroke::new(1.5, alpha(p.text, 0.3)),
                );
            }
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                *color = *preset;
                changed = true;
            }
        }
        ui.add_space(4.0);
        let mut c = crate::theme::from_rgb(*color);
        if egui::color_picker::color_edit_button_srgba(
            ui,
            &mut c,
            egui::color_picker::Alpha::Opaque,
        )
        .on_hover_text("Custom color")
        .changed()
        {
            *color = [c.r(), c.g(), c.b()];
            changed = true;
        }
    });
    changed
}

/// Paints a rounded-rect vertical gradient through `stops` (fraction, color),
/// clipped to the rounded corners of `rect` by masking them with `outside`.
pub fn gradient_rect(ui: &Ui, rect: Rect, radius: f32, stops: &[(f32, Color32)], outside: Color32) {
    let mut mesh = egui::Mesh::default();
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        let y0 = rect.top() + rect.height() * t0;
        let y1 = rect.top() + rect.height() * t1;
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(rect.left(), y0), c0);
        mesh.colored_vertex(pos2(rect.right(), y0), c0);
        mesh.colored_vertex(pos2(rect.left(), y1), c1);
        mesh.colored_vertex(pos2(rect.right(), y1), c1);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    // Corner masks: fan from each outer corner along the inner arc.
    let corners = [
        (
            rect.left_top(),
            pos2(rect.left() + radius, rect.top() + radius),
            std::f32::consts::PI,
        ),
        (
            rect.right_top(),
            pos2(rect.right() - radius, rect.top() + radius),
            1.5 * std::f32::consts::PI,
        ),
        (
            rect.right_bottom(),
            pos2(rect.right() - radius, rect.bottom() - radius),
            0.0,
        ),
        (
            rect.left_bottom(),
            pos2(rect.left() + radius, rect.bottom() - radius),
            0.5 * std::f32::consts::PI,
        ),
    ];
    for (corner, center, start) in corners {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(corner, outside);
        let n = 10;
        for i in 0..=n {
            let a = start + i as f32 / n as f32 * std::f32::consts::FRAC_PI_2;
            // Slight overshoot so no seam shows at the edge.
            mesh.colored_vertex(center + (radius + 0.5) * vec2(a.cos(), a.sin()), outside);
        }
        for i in 0..n {
            mesh.add_triangle(base, base + 1 + i, base + 2 + i);
        }
    }
    ui.painter().add(Shape::mesh(mesh));
}

/// The simple table outline from the mockup header.
pub fn table_icon(painter: &egui::Painter, center: Pos2, w: f32, color: Color32) {
    let s = Stroke::new(w * 0.075, color);
    let top = center.y - w * 0.28;
    let (l, r) = (center.x - w * 0.5, center.x + w * 0.5);
    painter.rect_stroke(
        Rect::from_min_max(pos2(l, top), pos2(r, top + w * 0.16)),
        2,
        s,
        StrokeKind::Middle,
    );
    for x in [l + w * 0.12, r - w * 0.12] {
        painter.line_segment([pos2(x, top + w * 0.16), pos2(x, center.y + w * 0.3)], s);
    }
}
