//! The CTRL Desk icon: a standing desk with an up arrow, drawn from signed
//! distance fields so it is crisp at every size. Pure Rust (no egui) so the
//! build script can reuse it for the Windows executable icon.

/// Design space: the glyph's bounding box in 1250×1250 design units.
const CONTENT_MIN: (f32, f32) = (80.0, 262.0);
const CONTENT_MAX: (f32, f32) = (1172.0, 1018.0);

pub type Rgba = [u8; 4];

/// Light glyph, for dark taskbars and panels.
pub const GLYPH_LIGHT: Rgba = [0xf0, 0xf0, 0xf0, 0xff];
/// Dark glyph, for light taskbars and panels.
pub const GLYPH_DARK: Rgba = [0x1f, 0x1f, 0x1f, 0xff];

pub enum Style {
    /// Only the glyph on a transparent background (tray icons).
    Glyph(Rgba),
    /// The glyph on a rounded tile with a vertical gradient (app icon).
    Tile { top: Rgba, bottom: Rgba, fg: Rgba },
}

/// The application icon: a light glyph on the default green primary color.
pub fn app_style() -> Style {
    Style::Tile {
        top: [0x3a, 0x6c, 0x4f, 0xff],
        bottom: [0x26, 0x4b, 0x35, 0xff],
        fg: [0xf6, 0xf4, 0xee, 0xff],
    }
}

/// Renders a `size`×`size` RGBA image (straight alpha).
pub fn render(size: u32, style: &Style) -> Vec<u8> {
    let n = size as f32;
    // Small sizes get a slightly larger, bolder glyph so it stays legible.
    let small = size <= 32;
    let (glyph_extent, fg) = match style {
        Style::Glyph(fg) => (0.94, *fg),
        Style::Tile { fg, .. } => (if small { 0.74 } else { 0.64 }, *fg),
    };
    let bolden = if small { 0.35 } else { 0.0 };
    // Fit the glyph's bounding box into `glyph_extent` of the image, centered.
    let content_w = CONTENT_MAX.0 - CONTENT_MIN.0;
    let content_h = CONTENT_MAX.1 - CONTENT_MIN.1;
    let scale = n * glyph_extent / content_w.max(content_h);
    let cx = (CONTENT_MIN.0 + CONTENT_MAX.0) / 2.0;
    let cy = (CONTENT_MIN.1 + CONTENT_MAX.1) / 2.0;

    let mut out = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut color = [0.0f32; 4];

            if let Style::Tile { top, bottom, .. } = style {
                // Rounded square with ~22% corner radius, inset by half a pixel.
                let half = n / 2.0 - 0.5;
                let d = sd_round_box((px - n / 2.0, py - n / 2.0), (half, half), n * 0.22);
                let a = coverage(d);
                let t = py / n;
                for (c, (a0, b0)) in color.iter_mut().zip(top.iter().zip(bottom)) {
                    *c = *a0 as f32 / 255.0 * (1.0 - t) + *b0 as f32 / 255.0 * t;
                }
                color[3] *= a;
            }

            // Pixel center in design units; one pixel = 1/scale design units.
            let p = ((px - n / 2.0) / scale + cx, (py - n / 2.0) / scale + cy);
            let a = coverage(glyph_sdf(p) * scale - bolden) * fg[3] as f32 / 255.0;
            let src = [
                fg[0] as f32 / 255.0,
                fg[1] as f32 / 255.0,
                fg[2] as f32 / 255.0,
            ];
            // Straight-alpha "over" compositing.
            let out_a = a + color[3] * (1.0 - a);
            if out_a > 0.0 {
                for i in 0..3 {
                    color[i] = (src[i] * a + color[i] * color[3] * (1.0 - a)) / out_a;
                }
            }
            color[3] = out_a;

            let i = ((y * size + x) * 4) as usize;
            for c in 0..4 {
                out[i + c] = (color[c].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    out
}

/// Anti-aliased coverage from a signed distance in pixels.
fn coverage(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

/// Signed distance (design units) to the desk-with-arrow glyph.
fn glyph_sdf(p: (f32, f32)) -> f32 {
    let at = |c: (f32, f32)| (p.0 - c.0, p.1 - c.1);
    let parts = [
        // Tabletop.
        sd_round_box(at((626.0, 329.5)), (546.0, 67.5), 67.5),
        // Legs, overlapping the top and feet so the joins are seamless.
        sd_round_box(at((270.5, 664.0)), (47.5, 290.0), 0.0),
        sd_round_box(at((982.5, 664.0)), (47.5, 290.0), 0.0),
        // Feet.
        sd_round_box(at((263.5, 975.0)), (158.5, 43.0), 43.0),
        sd_round_box(at((990.0, 975.0)), (158.0, 43.0), 43.0),
        // Arrow shaft and rounded head.
        sd_round_box(at((626.5, 760.0)), (52.5, 133.0), 36.0),
        sd_polygon(p, &[(627.0, 540.0), (742.0, 655.0), (512.0, 655.0)]) - 24.0,
    ];
    parts.into_iter().fold(f32::MAX, f32::min)
}

/// Box centered at the origin with half extents `b` and corner radius `r`.
fn sd_round_box(p: (f32, f32), b: (f32, f32), r: f32) -> f32 {
    let qx = p.0.abs() - b.0 + r;
    let qy = p.1.abs() - b.1 + r;
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

/// Exact signed distance to a closed polygon (negative inside).
fn sd_polygon(p: (f32, f32), v: &[(f32, f32)]) -> f32 {
    let dot = |a: (f32, f32), b: (f32, f32)| a.0 * b.0 + a.1 * b.1;
    let sub = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0, a.1 - b.1);
    let mut d = dot(sub(p, v[0]), sub(p, v[0]));
    let mut s = 1.0;
    let mut j = v.len() - 1;
    for i in 0..v.len() {
        let e = sub(v[j], v[i]);
        let w = sub(p, v[i]);
        let t = (dot(w, e) / dot(e, e)).clamp(0.0, 1.0);
        let b = (w.0 - e.0 * t, w.1 - e.1 * t);
        d = d.min(dot(b, b));
        let c = [p.1 >= v[i].1, p.1 < v[j].1, e.0 * w.1 > e.1 * w.0];
        if c.iter().all(|&x| x) || c.iter().all(|&x| !x) {
            s = -s;
        }
        j = i;
    }
    s * d.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_covers_expected_regions() {
        let img = render(100, &Style::Glyph(GLYPH_LIGHT));
        let alpha = |x: u32, y: u32| img[((y * 100 + x) * 4 + 3) as usize];
        // Middle of the tabletop and of the arrow are solid, the gap beside
        // the arrow and the corners are empty.
        assert_eq!(alpha(50, 23), 255);
        assert_eq!(alpha(50, 60), 255);
        assert_eq!(alpha(35, 60), 0);
        assert_eq!(alpha(1, 1), 0);
    }

    #[test]
    fn tile_is_opaque_inside_and_transparent_at_corners() {
        let img = render(64, &app_style());
        assert_eq!(img[3], 0);
        assert_eq!(img[((32 * 64 + 4) * 4 + 3) as usize], 255);
    }
}
