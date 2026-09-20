//! Shared grid-overlay rasterizer.
//!
//! Turns an `OverlayFrame` into premultiplied BGRA pixels, top-down, which is the
//! layout both a Win32 DIB section and an X11/XQuartz ZPixmap expect on
//! little-endian machines. Pure Rust on purpose: every platform renderer tests
//! against the same pixel output.

use clickless_core::grid::{OverlayFrame, Rect};

pub const PANEL_RGB: (u8, u8, u8) = (24, 28, 38);
pub const PANEL_ALPHA: u8 = 70;
pub const BORDER_RGB: (u8, u8, u8) = (100, 122, 150);
pub const HIGHLIGHT_RGB: (u8, u8, u8) = (255, 196, 64);
pub const HIGHLIGHT_ALPHA: u8 = 110;
pub const LABEL_RGB: (u8, u8, u8) = (240, 246, 255);
pub const POINTER_RGB: (u8, u8, u8) = (255, 96, 96);

pub const BORDER_PX: i64 = 1;
pub const HIGHLIGHT_BORDER_PX: i64 = 4;
pub const POINTER_ARM: i64 = 8;
pub const GLYPH_W: i64 = 5;
pub const GLYPH_H: i64 = 7;
pub const GLYPH_SCALE: i64 = 3;

/// A rendered overlay image plus where it belongs on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTarget {
    pub origin: (i64, i64),
    pub width: u32,
    pub height: u32,
    /// Premultiplied BGRA, top-down, `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

impl RenderTarget {
    /// Pixel at screen coordinates, if the point is inside the image.
    pub fn pixel(&self, x: i64, y: i64) -> Option<[u8; 4]> {
        let px = x - self.origin.0;
        let py = y - self.origin.1;
        if px < 0 || py < 0 || px >= self.width as i64 || py >= self.height as i64 {
            return None;
        }
        let offset = ((py * self.width as i64 + px) * 4) as usize;
        self.pixels
            .get(offset..offset + 4)
            .map(|p| [p[0], p[1], p[2], p[3]])
    }
}

/// Smallest rect covering every cell in the frame.
pub fn frame_bounds(frame: &OverlayFrame) -> Option<Rect> {
    let first = frame.cells.first()?.rect;
    let mut bounds = first;
    for cell in &frame.cells {
        let right = bounds.x + bounds.width;
        let bottom = bounds.y + bounds.height;
        let cell_right = cell.rect.x + cell.rect.width;
        let cell_bottom = cell.rect.y + cell.rect.height;
        bounds.x = bounds.x.min(cell.rect.x);
        bounds.y = bounds.y.min(cell.rect.y);
        bounds.width = right.max(cell_right) - bounds.x;
        bounds.height = bottom.max(cell_bottom) - bounds.y;
    }
    Some(bounds)
}

/// Where a label is centred inside a cell.
pub fn label_anchor(rect: Rect) -> (i64, i64) {
    rect.center()
}

pub fn text_size(text: &str) -> (i64, i64) {
    let chars = text.chars().count() as i64;
    if chars == 0 {
        return (0, GLYPH_H * GLYPH_SCALE);
    }
    (
        (chars * (GLYPH_W + 1) - 1) * GLYPH_SCALE,
        GLYPH_H * GLYPH_SCALE,
    )
}

/// 5x7 glyph rows, most significant bit is the leftmost column.
pub fn glyph_rows(ch: char) -> Option<[u8; 7]> {
    let rows = match ch.to_ascii_lowercase() {
        'b' => [16, 16, 30, 17, 17, 30, 0],
        'g' => [0, 15, 17, 15, 1, 17, 14],
        'n' => [0, 0, 30, 17, 17, 17, 0],
        'q' => [0, 14, 17, 17, 15, 1, 1],
        'r' => [0, 0, 22, 25, 16, 16, 0],
        't' => [8, 8, 30, 8, 8, 6, 0],
        'v' => [0, 0, 17, 17, 17, 10, 4],
        'x' => [0, 0, 17, 10, 4, 10, 17],
        'y' => [0, 17, 17, 15, 1, 17, 14],
        'z' => [0, 0, 31, 2, 4, 8, 31],
        ';' => [0, 6, 6, 0, 6, 4, 8],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        'a' => [
            0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111, 0b00000,
        ],
        'c' => [
            0b00000, 0b01110, 0b10001, 0b10000, 0b10001, 0b01110, 0b00000,
        ],
        'd' => [
            0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b01111, 0b00000,
        ],
        'e' => [
            0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01111, 0b00000,
        ],
        'f' => [
            0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000,
        ],
        'h' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b00000,
        ],
        'i' => [
            0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b01110, 0b00000,
        ],
        'j' => [
            0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'k' => [
            0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010,
        ],
        'l' => [
            0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110, 0b00000,
        ],
        'm' => [
            0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10101, 0b00000,
        ],
        'o' => [
            0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110, 0b00000,
        ],
        'p' => [
            0b00000, 0b01110, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        's' => [
            0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110, 0b00000,
        ],
        'u' => [
            0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101, 0b00000,
        ],
        'w' => [
            0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010, 0b00000,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00110,
        ],
        ' ' => [0; 7],
        _ => return None,
    };
    Some(rows)
}

fn premultiply(rgb: (u8, u8, u8), alpha: u8) -> [u8; 4] {
    let scale = |c: u8| ((c as u16 * alpha as u16) / 255) as u8;
    [scale(rgb.2), scale(rgb.1), scale(rgb.0), alpha]
}

fn put_pixel(pixels: &mut [u8], width: i64, height: i64, x: i64, y: i64, value: [u8; 4]) {
    if x < 0 || y < 0 || x >= width || y >= height {
        return;
    }
    let offset = ((y * width + x) * 4) as usize;
    if let Some(px) = pixels.get_mut(offset..offset + 4) {
        px.copy_from_slice(&value);
    }
}

fn fill_rect(pixels: &mut [u8], width: i64, height: i64, rect: Rect, value: [u8; 4]) {
    let left = rect.x.max(0);
    let right = rect.x.saturating_add(rect.width).min(width);
    let top = rect.y.max(0);
    let bottom = rect.y.saturating_add(rect.height).min(height);
    if left >= right || top >= bottom {
        return;
    }
    let start = ((top * width + left) * 4) as usize;
    let row_len = ((right - left) * 4) as usize;
    for pixel in pixels[start..start + row_len].as_chunks_mut::<4>().0 {
        *pixel = value;
    }
    for y in top + 1..bottom {
        let destination = ((y * width + left) * 4) as usize;
        pixels.copy_within(start..start + row_len, destination);
    }
}

fn stroke_rect(
    pixels: &mut [u8],
    width: i64,
    height: i64,
    rect: Rect,
    thickness: i64,
    value: [u8; 4],
) {
    if rect.width <= 0 || rect.height <= 0 || thickness <= 0 {
        return;
    }
    let thickness = thickness
        .min((rect.width / 2).max(1))
        .min((rect.height / 2).max(1));
    let horizontal = Rect::new(rect.x, rect.y, rect.width, thickness);
    fill_rect(pixels, width, height, horizontal, value);
    fill_rect(
        pixels,
        width,
        height,
        Rect::new(
            rect.x,
            rect.y + rect.height - thickness,
            rect.width,
            thickness,
        ),
        value,
    );
    let vertical = Rect::new(rect.x, rect.y, thickness, rect.height);
    fill_rect(pixels, width, height, vertical, value);
    fill_rect(
        pixels,
        width,
        height,
        Rect::new(
            rect.x + rect.width - thickness,
            rect.y,
            thickness,
            rect.height,
        ),
        value,
    );
}

fn draw_text(
    pixels: &mut [u8],
    width: i64,
    height: i64,
    text: &str,
    anchor: (i64, i64),
    value: [u8; 4],
    scale: i64,
) {
    let text_width = ((text.chars().count() as i64 * (GLYPH_W + 1)) - 1).max(0) * scale;
    let text_height = GLYPH_H * scale;
    let mut pen_x = anchor.0 - text_width / 2;
    let pen_y = anchor.1 - text_height / 2;
    for ch in text.chars() {
        if let Some(rows) = glyph_rows(ch) {
            for (row_index, row) in rows.iter().enumerate() {
                for col in 0..GLYPH_W {
                    if row & (1 << (GLYPH_W - 1 - col)) == 0 {
                        continue;
                    }
                    let x = pen_x + col * scale;
                    let y = pen_y + row_index as i64 * scale;
                    fill_rect(pixels, width, height, Rect::new(x, y, scale, scale), value);
                }
            }
        }
        pen_x += (GLYPH_W + 1) * scale;
    }
}

fn draw_pointer_with(pixels: &mut [u8], width: i64, height: i64, x: i64, y: i64, value: [u8; 4]) {
    for step in 1..=POINTER_ARM {
        put_pixel(pixels, width, height, x + step, y, value);
        put_pixel(pixels, width, height, x - step, y, value);
        put_pixel(pixels, width, height, x, y + step, value);
        put_pixel(pixels, width, height, x, y - step, value);
    }
    fill_rect(pixels, width, height, Rect::new(x - 2, y - 2, 5, 5), value);
}

/// User-tunable overlay colours and sizes. Defaults reproduce the original
/// hard-coded constants, so existing pixel tests keep passing unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayTheme {
    pub panel_rgb: (u8, u8, u8),
    pub panel_alpha: u8,
    pub border_rgb: (u8, u8, u8),
    pub border_px: i64,
    pub highlight_rgb: (u8, u8, u8),
    pub highlight_alpha: u8,
    pub label_rgb: (u8, u8, u8),
    pub pointer_rgb: (u8, u8, u8),
    /// Label glyph scale; 1 is smallest, larger means bigger text.
    pub glyph_scale: i64,
}

impl Default for OverlayTheme {
    fn default() -> Self {
        Self {
            panel_rgb: PANEL_RGB,
            panel_alpha: PANEL_ALPHA,
            border_rgb: BORDER_RGB,
            border_px: BORDER_PX,
            highlight_rgb: HIGHLIGHT_RGB,
            highlight_alpha: HIGHLIGHT_ALPHA,
            label_rgb: LABEL_RGB,
            pointer_rgb: POINTER_RGB,
            glyph_scale: GLYPH_SCALE,
        }
    }
}

/// Renders the frame with the built-in default theme.
pub fn render_frame(frame: &OverlayFrame) -> Option<RenderTarget> {
    render_frame_with_theme(frame, &OverlayTheme::default())
}

/// Renders the frame with a caller-supplied theme, or none when the frame has
/// no cells.
pub fn render_frame_with_theme(frame: &OverlayFrame, theme: &OverlayTheme) -> Option<RenderTarget> {
    let bounds = frame_bounds(frame)?;
    let width = bounds.width.max(1);
    let height = bounds.height.max(1);
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let origin = (bounds.x, bounds.y);

    let panel = premultiply(theme.panel_rgb, theme.panel_alpha);
    let border = premultiply(theme.border_rgb, 255);
    let highlight_panel = premultiply(theme.highlight_rgb, theme.highlight_alpha);
    let highlight_border = premultiply(theme.highlight_rgb, 255);
    let label = premultiply(theme.label_rgb, 255);

    for cell in &frame.cells {
        let local = Rect::new(
            cell.rect.x - origin.0,
            cell.rect.y - origin.1,
            cell.rect.width,
            cell.rect.height,
        );
        let active = frame.highlight == Some(cell.rect);
        let dense_label = cell.label.chars().count() == 2;
        let panel = if dense_label {
            let blend = ((local.x * 255) / width.max(1)) as u8;
            premultiply((160 - blend / 3, 140 + blend / 5, 80 + blend / 3), 85)
        } else {
            panel
        };
        fill_rect(
            &mut pixels,
            width,
            height,
            local,
            if active { highlight_panel } else { panel },
        );
        stroke_rect(
            &mut pixels,
            width,
            height,
            local,
            if active {
                HIGHLIGHT_BORDER_PX
            } else {
                theme.border_px
            },
            if active { highlight_border } else { border },
        );
        let (mut cx, mut cy) = label_anchor(local);
        // Opaque key badges keep labels readable over bright or busy applications.
        let (tw, th) = text_size(&cell.label);
        let badge_width = (tw + 20).min(local.width.saturating_sub(4)).max(0);
        let badge_height = (th + 14).min(local.height.saturating_sub(4)).max(0);
        if active && local.width > badge_width + 20 && local.height > badge_height + 20 {
            cx = local.x + badge_width / 2 + 8;
            cy = local.y + badge_height / 2 + 8;
        }
        let badge = Rect::new(
            cx - badge_width / 2,
            cy - badge_height / 2,
            badge_width,
            badge_height,
        );
        if !dense_label {
            // One-char nested labels must stay readable over bright or busy
            // applications at every DPI. The badge clamps to the cell, so it
            // backs the glyph even in 19x12 subcells where the old height
            // threshold never fired. Fixed dark colour on purpose: contrast
            // against the fixed light label colour.
            fill_rect(
                &mut pixels,
                width,
                height,
                badge,
                premultiply(PANEL_RGB, 255),
            );
            stroke_rect(
                &mut pixels,
                width,
                height,
                badge,
                1,
                if active { highlight_border } else { border },
            );
        }
        let scale = theme
            .glyph_scale
            .min((local.height - 4).max(0) / GLYPH_H)
            .min(
                (local.width - 4).max(0)
                    / (cell.label.chars().count() as i64 * (GLYPH_W + 1)).max(1),
            );
        if scale > 0 {
            if dense_label || local.height < 35 {
                draw_text(
                    &mut pixels,
                    width,
                    height,
                    &cell.label,
                    (cx + 1, cy + 1),
                    premultiply(theme.panel_rgb, 255),
                    scale,
                );
            }
            draw_text(
                &mut pixels,
                width,
                height,
                &cell.label,
                (cx, cy),
                label,
                scale,
            );
        }
    }

    if let Some((x, y)) = frame.pointer {
        draw_pointer_with(
            &mut pixels,
            width,
            height,
            x - origin.0,
            y - origin.1,
            premultiply(theme.pointer_rgb, 255),
        );
    }

    Some(RenderTarget {
        origin,
        width: width as u32,
        height: height as u32,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_core::grid::OverlayCell;

    fn frame(
        cells: Vec<(Rect, &str)>,
        highlight: Option<usize>,
        pointer: Option<(i64, i64)>,
    ) -> OverlayFrame {
        let cells: Vec<OverlayCell> = cells
            .into_iter()
            .map(|(rect, label)| OverlayCell {
                rect,
                label: label.to_string(),
            })
            .collect();
        let highlight = highlight.map(|index| cells[index].rect);
        OverlayFrame {
            level: 1,
            cells,
            highlight,
            pointer,
        }
    }

    fn count_pixels(target: &RenderTarget, rgb: (u8, u8, u8)) -> usize {
        let mut count = 0;
        let mut offset = 0;
        while offset + 4 <= target.pixels.len() {
            if target.pixels[offset] == rgb.2
                && target.pixels[offset + 1] == rgb.1
                && target.pixels[offset + 2] == rgb.0
            {
                count += 1;
            }
            offset += 4;
        }
        count
    }

    #[test]
    fn t01_frame_bounds_covers_every_cell() {
        let f = frame(
            vec![
                (Rect::new(10, 20, 100, 50), "u"),
                (Rect::new(400, 300, 200, 100), "i"),
            ],
            None,
            None,
        );
        assert_eq!(frame_bounds(&f), Some(Rect::new(10, 20, 590, 380)));
    }

    #[test]
    fn t02_empty_frame_has_no_bounds_and_no_image() {
        let f = frame(Vec::new(), None, None);
        assert_eq!(frame_bounds(&f), None);
        assert!(render_frame(&f).is_none());
    }

    #[test]
    fn t03_render_target_matches_bounds_and_encodes_four_bytes_per_pixel() {
        let f = frame(
            vec![
                (Rect::new(100, 200, 60, 40), "u"),
                (Rect::new(160, 200, 60, 40), "i"),
            ],
            None,
            None,
        );
        let target = render_frame(&f).unwrap();
        assert_eq!(target.origin, (100, 200));
        assert_eq!(target.width, 120);
        assert_eq!(target.height, 40);
        assert_eq!(target.pixels.len(), 120 * 40 * 4);
    }

    #[test]
    fn t04_cell_interiors_hold_translucent_premultiplied_panel_colour() {
        let f = frame(vec![(Rect::new(0, 0, 40, 20), "u")], None, None);
        let target = render_frame(&f).unwrap();
        let body = target.pixel(1, 1).unwrap();
        assert_eq!(body, premultiply(PANEL_RGB, PANEL_ALPHA));
        assert_eq!(body[3], PANEL_ALPHA);
    }

    #[test]
    fn t05_cell_edges_are_opaque_border_colour() {
        let f = frame(vec![(Rect::new(0, 0, 40, 20), "u")], None, None);
        let target = render_frame(&f).unwrap();
        assert_eq!(target.pixel(0, 0).unwrap(), premultiply(BORDER_RGB, 255));
        assert_eq!(target.pixel(39, 19).unwrap(), premultiply(BORDER_RGB, 255));
    }

    #[test]
    fn t06_labels_are_drawn_inside_their_cell() {
        let f = frame(vec![(Rect::new(0, 0, 60, 40), "u")], None, None);
        let target = render_frame(&f).unwrap();
        assert!(count_pixels(&target, LABEL_RGB) > 0);
        // The label sits around the cell centre, not against the border.
        let centre = target.pixel(30, 20).unwrap();
        assert_ne!(centre, premultiply(LABEL_RGB, 255));
    }

    #[test]
    fn t07_unknown_glyphs_draw_nothing_and_known_ones_draw() {
        assert!(glyph_rows('?').is_none());
        for label in ["u", "i", "o", "j", "k", "l", "m", ",", "."] {
            let f = frame(vec![(Rect::new(0, 0, 60, 40), label)], None, None);
            let target = render_frame(&f).unwrap();
            assert!(
                count_pixels(&target, LABEL_RGB) > 0,
                "label {label} drew nothing"
            );
        }
    }

    #[test]
    fn t08_highlight_needs_a_thicker_border_in_the_highlight_colour() {
        let plain = frame(vec![(Rect::new(0, 0, 60, 40), "u")], None, None);
        let active = frame(vec![(Rect::new(0, 0, 60, 40), "u")], Some(0), None);
        let plain_target = render_frame(&plain).unwrap();
        let active_target = render_frame(&active).unwrap();

        assert_eq!(count_pixels(&plain_target, HIGHLIGHT_RGB), 0);
        assert!(
            count_pixels(&active_target, HIGHLIGHT_RGB) > count_pixels(&active_target, BORDER_RGB)
        );
        assert_eq!(
            active_target.pixel(10, 10).unwrap(),
            premultiply(HIGHLIGHT_RGB, HIGHLIGHT_ALPHA)
        );
    }

    #[test]
    fn t09_pointer_draws_a_marker_and_reads_back_by_screen_coordinate() {
        let f = frame(vec![(Rect::new(0, 0, 60, 40), "u")], None, Some((30, 20)));
        let target = render_frame(&f).unwrap();
        assert!(count_pixels(&target, POINTER_RGB) > 0);
        assert_eq!(target.pixel(30, 20).unwrap(), premultiply(POINTER_RGB, 255));
        assert_eq!(target.pixel(29, 20).unwrap(), premultiply(POINTER_RGB, 255));
        assert_eq!(target.pixel(30, 12).unwrap(), premultiply(POINTER_RGB, 255));
        assert_eq!(target.pixel(-1, 20), None);
        assert_eq!(target.pixel(60, 20), None);
    }

    #[test]
    fn t10_text_size_grows_with_the_label() {
        assert_eq!(text_size(""), (0, GLYPH_H * GLYPH_SCALE));
        assert_eq!(
            text_size("u"),
            (GLYPH_W * GLYPH_SCALE, GLYPH_H * GLYPH_SCALE)
        );
        assert!(text_size("uu").0 > text_size("u").0);
    }

    // theme plumbing

    #[test]
    fn t11_theme_changes_panel_and_label_colour() {
        let f = frame(vec![(Rect::new(0, 0, 60, 40), "u")], None, None);
        let theme = OverlayTheme {
            panel_rgb: (255, 0, 255),
            label_rgb: (0, 255, 0),
            ..OverlayTheme::default()
        };
        let target = render_frame_with_theme(&f, &theme).unwrap();
        // Panel is stored premultiplied with its alpha; compare in that space.
        assert!(
            count_pixels(&target, (255, 0, 255)) > 0 || target.pixel(1, 1).unwrap()[0] != 0,
            "panel missing"
        );
        assert!(count_pixels(&target, (0, 255, 0)) > 0, "label missing");
        // Default-theme output must not contain the custom panel colour.
        let default_target = render_frame(&f).unwrap();
        assert_eq!(count_pixels(&default_target, (255, 0, 255)), 0);
    }

    #[test]
    fn t12_theme_alpha_controls_panel_opacity() {
        let f = frame(vec![(Rect::new(0, 0, 40, 20), "u")], None, None);
        let theme = OverlayTheme {
            panel_alpha: 200,
            ..OverlayTheme::default()
        };
        let target = render_frame_with_theme(&f, &theme).unwrap();
        assert_eq!(target.pixel(1, 1).unwrap()[3], 200);
    }

    #[test]
    fn t13_theme_pointer_colour_replaces_the_marker() {
        let f = frame(vec![(Rect::new(0, 0, 60, 40), "u")], None, Some((30, 20)));
        let theme = OverlayTheme {
            pointer_rgb: (1, 2, 3),
            ..OverlayTheme::default()
        };
        let target = render_frame_with_theme(&f, &theme).unwrap();
        assert!(count_pixels(&target, (1, 2, 3)) > 0);
        assert_eq!(count_pixels(&target, POINTER_RGB), 0);
    }

    #[test]
    fn t14_theme_border_width_widens_the_frame() {
        let f = frame(vec![(Rect::new(0, 0, 60, 40), "u")], None, None);
        let theme = OverlayTheme {
            border_px: 6,
            ..OverlayTheme::default()
        };
        let target = render_frame_with_theme(&f, &theme).unwrap();
        assert_eq!(target.pixel(4, 4).unwrap(), premultiply(BORDER_RGB, 255));
    }

    // nested-label legibility (Prompt 2)

    /// Real dense level-2 frame: 299 two-char context cells plus 30 nested
    /// one-char cells.
    fn dense_level2_frame(width: i64, height: i64) -> OverlayFrame {
        use clickless_core::LogicalKey;
        use clickless_core::grid::{GridConfig, GridNavigator};
        let mut nav = GridNavigator::new(width, height, GridConfig::dense());
        nav.activate();
        nav.on_key_press(LogicalKey::K); // column prefix
        nav.on_key_release(LogicalKey::K); // release before typing again
        nav.on_key_press(LogicalKey::K); // row: parent selected, nested shown
        nav.overlay_frame().unwrap()
    }

    fn rect_contains(target: &RenderTarget, rect: Rect, value: [u8; 4]) -> bool {
        (rect.y..rect.y + rect.height)
            .any(|y| (rect.x..rect.x + rect.width).any(|x| target.pixel(x, y) == Some(value)))
    }

    #[test]
    fn t15_nested_one_char_cells_carry_an_opaque_badge() {
        for (width, height) in [(1920i64, 1080i64), (2880, 1620), (3840, 2160)] {
            let frame = dense_level2_frame(width, height);
            let target = render_frame(&frame).unwrap();
            let nested: Vec<_> = frame
                .cells
                .iter()
                .filter(|c| c.label.chars().count() == 1)
                .collect();
            assert_eq!(nested.len(), 30);
            // The pointer marker intentionally overwrites whatever it sits
            // on; skip cells its arms and box can reach.
            let marker = frame.pointer.map(|(px, py)| {
                Rect::new(
                    px - POINTER_ARM - 2,
                    py - POINTER_ARM - 2,
                    2 * POINTER_ARM + 5,
                    2 * POINTER_ARM + 5,
                )
            });
            for cell in &nested {
                if let Some(m) = marker {
                    let overlaps = cell.rect.x < m.x + m.width
                        && m.x < cell.rect.x + cell.rect.width
                        && cell.rect.y < m.y + m.height
                        && m.y < cell.rect.y + cell.rect.height;
                    if overlaps {
                        continue;
                    }
                }
                // Replicate the badge geometry: clamped to the cell minus
                // 2px padding, centred on the label anchor. The probe sits
                // just inside the badge's left edge, left of the glyph and
                // its shadow: it must be opaque badge fill, never the
                // 27%-opaque panel or a neighbouring border.
                let (tw, _) = text_size(&cell.label);
                let badge_width = ((tw + 20).min(cell.rect.width - 4)).max(0);
                let probe = (
                    cell.rect.x + cell.rect.width / 2 - badge_width / 2 + 1,
                    cell.rect.y + cell.rect.height / 2,
                );
                assert_eq!(
                    target.pixel(probe.0, probe.1),
                    Some(premultiply(PANEL_RGB, 255)),
                    "no opaque badge behind nested label at {width}x{height}"
                );
                assert!(
                    rect_contains(&target, cell.rect, premultiply(LABEL_RGB, 255)),
                    "nested glyph missing at {width}x{height}"
                );
            }
        }
    }
}
