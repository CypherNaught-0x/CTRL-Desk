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

    let label_format = egui::TextFormat {
        font_id: font(14.0, Weight::Regular),
        color: p.text,
        ..Default::default()
    };
    // Width right of the dimension line taken by its label, plus a margin.
    let label_w = match scene.label {
        Some(_) => 18.0 + crate::widgets::tabular_width(&painter, "000.0 cm", &label_format),
        None => 0.0,
    };
    // Design units right of the tabletop's front edge: its depth, then the gap
    // before the dimension line.
    const RIGHT_UNITS: f32 = 34.0 + 18.0;
    let units_per_mm = 540.0 / 2000.0;

    // Heights are to scale; the desk is drawn wider than life (2 m) to match
    // the mockup's proportions. The monitor reaches ~60 cm above the front edge.
    let mut px_per_mm =
        (rect.width() * 0.56 / 2000.0).min((floor_y - rect.top() - 16.0) / (scene.max_mm + 600.0));
    if let Some(avoid) = scene.avoid {
        // Decide from the highest position, not the current one, so the desk
        // never jumps sideways while the height animates. The plant reaches
        // ~100 units above the front edge at the left end.
        let gap = 24.0;
        let u = px_per_mm / units_per_mm;
        let highest_top = floor_y - scene.max_mm * px_per_mm;
        let clear_x = avoid.right() + gap;
        if highest_top - 100.0 * u < avoid.bottom() && cx - 1000.0 * px_per_mm < clear_x {
            // Fit the desk and its dimension label between the text and the
            // right edge, drawing it smaller if needed.
            let space = rect.right() - clear_x - label_w;
            px_per_mm = px_per_mm.min(space / (2000.0 + RIGHT_UNITS / units_per_mm));
            cx = cx.max(clear_x + 1000.0 * px_per_mm);
        }
    }
    let dw = 2000.0 * px_per_mm;
    let u = dw / 540.0; // design unit; the desk is 540 units wide
    // Keep the dimension label inside the scene.
    cx = cx.min(rect.right() - label_w - RIGHT_UNITS * u - dw / 2.0);
    let top_y = floor_y - scene.height_mm * px_per_mm;
    // Oblique projection: the back of the desk is up and to the right. Every
    // front-to-back line (tabletop, feet, things on the desk) follows `depth`.
    let depth = vec2(34.0 * u, -40.0 * u);

    let wood_top = hex(0xe0c29b);
    let wood_edge = hex(0xc9a374);
    let wood_side = hex(0xb48d5c);
    let (metal, metal_hi) = if p.dark {
        (hex(0x0c0c0c), hex(0x363636))
    } else {
        (hex(0x2a2a2a), hex(0x3d3d3d))
    };

    let thick = 9.0 * u;
    let left = cx - dw / 2.0;
    let right = cx + dw / 2.0;
    // A point on the tabletop: `x` along the front edge, `t` from the front (0)
    // to the back (1).
    let on_top = |x: f32, t: f32| pos2(x, top_y) + depth * t;
    // A flat quad on the tabletop.
    let plate = |x0: f32, x1: f32, t0: f32, t1: f32| {
        vec![
            on_top(x0, t0),
            on_top(x1, t0),
            on_top(x1, t1),
            on_top(x0, t1),
        ]
    };

    // Floor shadow, centered under the middle of the desk's depth.
    let shadow_c = pos2(cx, floor_y) + depth * 0.5;
    for i in 0..6 {
        let k = 1.0 - i as f32 * 0.14;
        painter.add(Shape::ellipse_filled(
            shadow_c,
            vec2(dw * 0.56 * k + 18.0 * u, -depth.y * 0.8 * k),
            alpha(p.shadow(), 0.22),
        ));
    }

    // Legs and feet. The columns stand at mid-depth; the feet run front to
    // back along `depth`, a little longer than the top is deep.
    let leg_xs = [left + dw * 0.15, right - dw * 0.15];
    let lower_len = 290.0 * px_per_mm;
    for &x in &leg_xs {
        let foot_w = 9.0 * u;
        let a = pos2(x, floor_y) + depth * -0.1;
        let b = pos2(x, floor_y) + depth * 1.1;
        painter.line_segment([a, b], Stroke::new(foot_w, metal));
        painter.circle_filled(a, foot_w / 2.0, metal);
        painter.circle_filled(b, foot_w / 2.0, metal);

        // Telescoping column: wide lower stage, narrower upper stage.
        let base = pos2(x, floor_y) + depth * 0.5;
        let leg_base = base.y - 2.0 * u;
        let lower = Rect::from_min_max(
            pos2(base.x - 12.0 * u, leg_base - lower_len),
            pos2(base.x + 12.0 * u, leg_base),
        );
        let upper = Rect::from_min_max(
            pos2(base.x - 9.5 * u, top_y + thick + depth.y * 0.5),
            pos2(base.x + 9.5 * u, lower.top() + 2.0),
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

    // Things on the desk, back to front. First the monitor near the back, on
    // a flat base plate.
    let mon_x = cx + 30.0 * u;
    painter.add(Shape::convex_polygon(
        plate(mon_x - 26.0 * u, mon_x + 26.0 * u, 0.62, 0.86),
        hex(0x1b1b1b),
        Stroke::NONE,
    ));
    let neck_foot = on_top(mon_x, 0.76);
    let neck = Rect::from_min_max(
        pos2(neck_foot.x - 3.5 * u, neck_foot.y - 38.0 * u),
        pos2(neck_foot.x + 3.5 * u, neck_foot.y),
    );
    painter.rect_filled(neck, 1, hex(0x202020));
    let screen = Rect::from_center_size(
        pos2(neck.center().x, neck.top() - 44.0 * u),
        vec2(190.0 * u, 108.0 * u),
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

    paint_plant(&painter, on_top(left + dw * 0.17, 0.5), u * 1.35);

    // Keyboard (roughly 44 by 14 cm) near the front edge, with a mouse beside it.
    let kb_x0 = cx - 72.0 * u;
    let kb_x1 = kb_x0 + 120.0 * u;
    let (kb_t0, kb_t1) = (0.1, 0.32);
    let lift = vec2(0.0, -2.5 * u);
    painter.add(Shape::convex_polygon(
        vec![
            on_top(kb_x0, kb_t0),
            on_top(kb_x1, kb_t0),
            on_top(kb_x1, kb_t0) + lift,
            on_top(kb_x0, kb_t0) + lift,
        ],
        hex(0x1c1c1c),
        Stroke::NONE,
    ));
    let kb_top = plate(kb_x0, kb_x1, kb_t0, kb_t1)
        .into_iter()
        .map(|q| q + lift)
        .collect();
    painter.add(Shape::convex_polygon(kb_top, hex(0x2e2e2e), Stroke::NONE));
    // Rows of keys.
    let key_stroke = Stroke::new((0.9 * u).max(1.0), hex(0x3d3d3d));
    for row in 1..5 {
        let t = kb_t0 + (kb_t1 - kb_t0) * row as f32 / 5.0;
        let inset = 5.0 * u;
        painter.line_segment(
            [
                on_top(kb_x0 + inset, t) + lift,
                on_top(kb_x1 - inset, t) + lift,
            ],
            key_stroke,
        );
    }
    let mouse = on_top(kb_x1 + 26.0 * u, 0.22);
    painter.add(Shape::ellipse_filled(
        mouse + vec2(1.0 * u, 1.0 * u),
        vec2(8.5 * u, 4.5 * u),
        Color32::from_black_alpha(30),
    ));
    painter.add(Shape::ellipse_filled(
        mouse + lift * 0.6,
        vec2(8.0 * u, 4.5 * u),
        hex(0x2b2b2b),
    ));

    // Dimension line on the right.
    if let Some(label) = &scene.label {
        let x = right + depth.x + 18.0 * u;
        let c = alpha(p.text, 0.55);
        let s = Stroke::new(1.0, c);
        painter.line_segment([pos2(x, top_y), pos2(x, floor_y)], s);
        for y in [top_y, floor_y] {
            painter.line_segment([pos2(x - 6.0, y), pos2(x + 6.0, y)], s);
        }
        // The desk was placed so the label fits to the right of the line.
        let (job, _) = crate::widgets::tabular_job(&painter, label, &label_format);
        let galley = painter.layout_job(job);
        let pos = pos2(x + 10.0, (top_y + floor_y - galley.size().y) / 2.0);
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
