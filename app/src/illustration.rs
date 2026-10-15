//! Vector illustration of a standing desk whose tabletop follows the real height.

use egui::{Color32, Pos2, Rect, Shape, Stroke, Ui, pos2, vec2};

use crate::theme::{Palette, Weight, alpha, font, hex, mix};

pub struct DeskScene {
    /// Current tabletop height above the floor.
    pub height_mm: f32,
    /// Highest possible position, used to fit the drawing into `rect`.
    pub max_mm: f32,
    /// Label for the dimension line, e.g. "104.5 cm".
    pub label: Option<String>,
    /// Horizontal position of the desk center as a fraction of the width.
    pub center_x: f32,
    /// Area (e.g. a text block) the desk should not overlap; the desk is
    /// shifted right to clear it when the tabletop is high enough to collide.
    pub avoid: Option<Rect>,
}

/// Paints the scene (without background) into `rect`.
pub fn paint(ui: &Ui, rect: Rect, scene: &DeskScene, p: &Palette) {
    let painter = ui.painter_at(rect);
    let floor_y = rect.bottom() - rect.height() * 0.14;
    let mut cx = rect.left() + rect.width() * scene.center_x;

    // Heights are to scale; the desk is drawn wider than life (2 m) to match
    // the mockup's proportions. The monitor reaches ~52 cm above the top.
    let px_per_mm =
        (rect.width() * 0.56 / 2000.0).min((floor_y - rect.top() - 16.0) / (scene.max_mm + 520.0));
    let dw = 2000.0 * px_per_mm;
    let u = dw / 540.0; // design unit; the desk is 540 units wide
    let top_y = floor_y - scene.height_mm * px_per_mm;
    if let Some(avoid) = scene.avoid {
        // Decide from the highest position, not the current one, so the desk
        // never jumps sideways while the height animates. The plant reaches
        // ~75 units above the tabletop at the left end.
        let gap = 24.0;
        let left_edge = cx - dw / 2.0;
        let highest_top = floor_y - scene.max_mm * px_per_mm;
        if highest_top - 75.0 * u < avoid.bottom() && left_edge < avoid.right() + gap {
            // Leave room for the dimension label on the right.
            let max_cx = rect.right() - dw / 2.0 - 110.0 * u;
            cx = (cx + avoid.right() + gap - left_edge).min(max_cx.max(cx));
        }
    }

    let wood_top = hex(0xe0c29b);
    let wood_edge = hex(0xc9a374);
    let wood_side = hex(0xb48d5c);
    let (metal, metal_hi) = if p.dark {
        (hex(0x0c0c0c), hex(0x363636))
    } else {
        (hex(0x2a2a2a), hex(0x3d3d3d))
    };

    // Oblique projection: the back of the desk is up and to the right.
    let depth = vec2(9.0 * u, -16.0 * u);
    let thick = 9.0 * u;
    let left = cx - dw / 2.0;
    let right = cx + dw / 2.0;

    // Floor shadow.
    for i in 0..6 {
        let k = 1.0 - i as f32 * 0.14;
        painter.add(Shape::ellipse_filled(
            pos2(cx + 10.0 * u, floor_y + 2.0 * u),
            vec2(dw * 0.56 * k + 18.0 * u, 20.0 * u * k),
            alpha(p.shadow(), 0.22),
        ));
    }

    // Legs and feet.
    let leg_xs = [left + dw * 0.15, right - dw * 0.15];
    let lower_len = 290.0 * px_per_mm;
    for &x in &leg_xs {
        // Foot: a bar running front-to-back.
        let a = pos2(x - 22.0 * u, floor_y + 8.0 * u);
        let b = pos2(x + 30.0 * u, floor_y - 18.0 * u);
        let w = 9.0 * u;
        painter.line_segment([a, b], Stroke::new(w, metal));
        painter.circle_filled(a, w / 2.0, metal);
        painter.circle_filled(b, w / 2.0, metal);
        painter.rect_filled(
            Rect::from_center_size(a + vec2(0.0, w * 0.6), vec2(w * 1.4, 2.0 * u)),
            1,
            metal,
        );

        // Telescoping column: wide lower stage, narrower upper stage.
        let leg_base = floor_y - 4.0 * u;
        let lower = Rect::from_min_max(
            pos2(x - 12.0 * u, leg_base - lower_len),
            pos2(x + 12.0 * u, leg_base),
        );
        let upper = Rect::from_min_max(
            pos2(x - 9.5 * u, top_y + thick),
            pos2(x + 9.5 * u, lower.top() + 2.0),
        );
        if upper.height() > 0.0 {
            painter.rect_filled(upper, 1, metal);
            painter.rect_filled(
                Rect::from_min_size(upper.min, vec2(2.2 * u, upper.height())),
                0,
                metal_hi,
            );
        }
        painter.rect_filled(lower, 1.5, metal);
        painter.rect_filled(
            Rect::from_min_size(lower.min, vec2(2.6 * u, lower.height())),
            0,
            metal_hi,
        );
        // Collar where the stages meet.
        painter.rect_filled(
            Rect::from_min_size(
                lower.min - vec2(0.8 * u, 0.0),
                vec2(lower.width() + 1.6 * u, 3.0 * u),
            ),
            1,
            metal_hi,
        );
    }

    // Frame under the top.
    let beam = Rect::from_min_max(
        pos2(leg_xs[0] - 4.0 * u, top_y + thick),
        pos2(leg_xs[1] + 4.0 * u, top_y + thick + 5.0 * u),
    );
    painter.rect_filled(beam, 1, metal);
    // Control panel under the right front edge.
    let panel = Rect::from_min_size(
        pos2(right - dw * 0.27, top_y + thick),
        vec2(16.0 * u, 5.0 * u),
    );
    painter.rect_filled(panel, 1.5, hex(0x222222));

    // Tabletop: top face, right side, front edge.
    let fl = pos2(left, top_y);
    let fr = pos2(right, top_y);
    painter.add(Shape::convex_polygon(
        vec![fl, fr, fr + depth, fl + depth],
        wood_top,
        Stroke::NONE,
    ));
    painter.add(Shape::convex_polygon(
        vec![
            fr,
            fr + depth,
            fr + depth + vec2(0.0, thick),
            fr + vec2(0.0, thick),
        ],
        wood_side,
        Stroke::NONE,
    ));
    painter.rect_filled(Rect::from_min_max(fl, fr + vec2(0.0, thick)), 2, wood_edge);
    painter.line_segment(
        [fl + vec2(1.0, 0.5), fr + vec2(-1.0, 0.5)],
        Stroke::new(1.0, mix(wood_top, Color32::WHITE, 0.35)),
    );

    // Monitor.
    let mon_c = pos2(cx + 40.0 * u, top_y + depth.y * 0.55);
    let stand_base =
        Rect::from_center_size(mon_c + vec2(depth.x * 0.55, 0.0), vec2(46.0 * u, 3.5 * u));
    painter.rect_filled(stand_base, 2, hex(0x1b1b1b));
    let neck = Rect::from_min_max(
        pos2(stand_base.center().x - 3.0 * u, mon_c.y - 40.0 * u),
        pos2(stand_base.center().x + 3.0 * u, mon_c.y),
    );
    painter.rect_filled(neck, 1, hex(0x202020));
    let screen = Rect::from_center_size(
        pos2(neck.center().x, mon_c.y - 40.0 * u - 52.0 * u),
        vec2(205.0 * u, 120.0 * u),
    );
    painter.rect_filled(
        screen.translate(vec2(2.0 * u, 3.0 * u)),
        4,
        alpha(Color32::BLACK, 0.15),
    );
    painter.rect_filled(screen, 4, hex(0x131313));
    let glass = screen.shrink(3.0 * u);
    painter.rect_filled(glass, 2, hex(0x1f2022));
    painter.add(Shape::convex_polygon(
        vec![
            glass.left_top(),
            pos2(glass.left() + glass.width() * 0.55, glass.top()),
            pos2(glass.left() + glass.width() * 0.25, glass.bottom()),
            glass.left_bottom(),
        ],
        Color32::from_white_alpha(6),
        Stroke::NONE,
    ));

    // Keyboard and mouse.
    let kb_y = top_y + depth.y * 0.32;
    let kb_x = cx - 15.0 * u;
    let kb = [
        pos2(kb_x - 52.0 * u, kb_y + 2.5 * u),
        pos2(kb_x + 52.0 * u, kb_y + 2.5 * u),
        pos2(kb_x + 55.0 * u, kb_y - 2.5 * u),
        pos2(kb_x - 49.0 * u, kb_y - 2.5 * u),
    ];
    painter.add(Shape::convex_polygon(
        kb.to_vec(),
        hex(0x2b2b2b),
        Stroke::NONE,
    ));
    painter.add(Shape::ellipse_filled(
        pos2(cx + 72.0 * u, kb_y),
        vec2(6.5 * u, 3.2 * u),
        hex(0x2b2b2b),
    ));

    paint_plant(
        &painter,
        pos2(left + dw * 0.2, top_y + depth.y * 0.5),
        u * 1.35,
    );

    // Dimension line on the right.
    if let Some(label) = &scene.label {
        let x = right + depth.x + 18.0 * u;
        let c = alpha(p.text, 0.55);
        let s = Stroke::new(1.0, c);
        painter.line_segment([pos2(x, top_y), pos2(x, floor_y)], s);
        for y in [top_y, floor_y] {
            painter.line_segment([pos2(x - 6.0, y), pos2(x + 6.0, y)], s);
        }
        let text_pos = pos2(x + 10.0, (top_y + floor_y) / 2.0);
        let format = egui::TextFormat {
            font_id: font(14.0, Weight::Regular),
            color: p.text,
            ..Default::default()
        };
        let (job, trailing) = crate::widgets::tabular_job(&painter, label, &format);
        let galley = painter.layout_job(job);
        // Put the label inside if a label for the highest position would be
        // clipped on the right (a fixed decision, so it never flips sides).
        let widest = crate::widgets::tabular_width(&painter, "000.0 cm", &format);
        let pos = if text_pos.x + widest > rect.right() - 8.0 {
            pos2(
                x - 10.0 - galley.size().x - trailing,
                text_pos.y - galley.size().y / 2.0,
            )
        } else {
            pos2(text_pos.x, text_pos.y - galley.size().y / 2.0)
        };
        painter.galley(pos, galley, p.text);
    }
}

fn paint_plant(painter: &egui::Painter, base: Pos2, u: f32) {
    let pot_w = 28.0 * u;
    let pot_h = 26.0 * u;
    let pot = Rect::from_min_max(
        pos2(base.x - pot_w / 2.0, base.y - pot_h),
        pos2(base.x + pot_w / 2.0, base.y),
    );
    painter.add(Shape::ellipse_filled(
        pos2(base.x + 3.0 * u, base.y),
        vec2(pot_w * 0.6, 3.0 * u),
        Color32::from_black_alpha(30),
    ));
    painter.add(Shape::convex_polygon(
        vec![
            pot.left_top(),
            pot.right_top(),
            pos2(pot.right() - 3.0 * u, pot.bottom()),
            pos2(pot.left() + 3.0 * u, pot.bottom()),
        ],
        hex(0xe9e5dd),
        Stroke::NONE,
    ));
    painter.rect_filled(
        Rect::from_min_size(
            pos2(pot.center().x + 2.0 * u, pot.top()),
            vec2(pot_w * 0.35, pot_h),
        ),
        0,
        Color32::from_black_alpha(14),
    );
    painter.rect_filled(
        Rect::from_min_size(
            pot.left_top() - vec2(1.0 * u, 1.5 * u),
            vec2(pot_w + 2.0 * u, 4.0 * u),
        ),
        1,
        hex(0xdcd7cd),
    );

    let stem = pos2(base.x, pot.top());
    let leaves: [(f32, f32, u32); 9] = [
        (-150.0, 44.0, 0x3f6b3a),
        (-30.0, 44.0, 0x3f6b3a),
        (-125.0, 50.0, 0x4b7d43),
        (-55.0, 50.0, 0x4b7d43),
        (-105.0, 54.0, 0x5a8f4f),
        (-75.0, 56.0, 0x5a8f4f),
        (-165.0, 36.0, 0x355e31),
        (-15.0, 36.0, 0x355e31),
        (-90.0, 46.0, 0x66a05a),
    ];
    for (deg, len, color) in leaves {
        let dir = vec2(deg.to_radians().cos(), deg.to_radians().sin());
        let normal = vec2(-dir.y, dir.x);
        let l = len * u;
        let w = l * 0.24;
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let t = i as f32 / 16.0;
                let along = if i <= 8 { t * 2.0 } else { (1.0 - t) * 2.0 };
                let side = if i <= 8 { 1.0 } else { -1.0 };
                let width = (along * std::f32::consts::PI).sin() * w * side;
                stem + dir * (l * 0.15 + along * l * 0.85) + normal * width
            })
            .collect();
        let c = hex(color);
        painter.add(Shape::convex_polygon(pts, c, Stroke::NONE));
        painter.line_segment(
            [stem + dir * l * 0.15, stem + dir * l * 0.95],
            Stroke::new(0.8, mix(c, Color32::WHITE, 0.25)),
        );
    }
}

/// The hero background: wall and floor gradients clipped to a rounded rect.
pub fn paint_background(ui: &Ui, rect: Rect, p: &Palette, outside: Color32) {
    let horizon = 0.6;
    crate::widgets::gradient_rect(
        ui,
        rect,
        16.0,
        &[
            (0.0, p.wall[0]),
            (horizon, p.wall[1]),
            (horizon + 0.06, p.floor[0]),
            (1.0, p.floor[1]),
        ],
        outside,
    );
}
