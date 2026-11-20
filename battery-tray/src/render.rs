//! Pure icon renderer. The shape is designed on a 16x16 pixel grid (the tray cell at 100% scaling)
//! and scaled by `size / 16` for other sizes. Every outline edge is snapped to a whole device pixel
//! so the icon is crisp; only the rounded corners and the fill-level edge are anti-aliased
//! (4x4 supersampling).

const GRID: f32 = 16.0;
const SS: u32 = 4;

/// Body size in grid pixels (nub: 2 wide, 1 gap).
const BODY_W: f32 = 13.0;
const PLAIN_H: f32 = 10.0;
const PERCENT_H: f32 = 11.0;
const BODY_R: f32 = 3.0;
const NUB_X: f32 = 14.0;
const NUB_W: f32 = 2.0;

const TRACK_ALPHA: f32 = 0.5;
const NUB_ALPHA: f32 = 0.75;

/// Bolt polygon in body-local grid pixels (body 13x10, about 6 wide and 8 tall, centered).
const BOLT: [(f32, f32); 6] = [
    (7.55, 1.2),
    (3.6, 6.0),
    (6.2, 6.0),
    (5.45, 8.8),
    (9.4, 4.0),
    (6.8, 4.0),
];

/// 3x5 digit glyphs, one row per entry, bit 2 = left column.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];
/// Small bolt shown before the digits while charging (below 100%).
const BOLT_GLYPH: [u8; 5] = [0b001, 0b010, 0b111, 0b010, 0b100];

fn in_round_rect(px: f32, py: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    let (w, h) = (x1 - x0, y1 - y0);
    let dx = ((px - (x0 + w / 2.0)).abs() - (w / 2.0 - r)).max(0.0);
    let dy = ((py - (y0 + h / 2.0)).abs() - (h / 2.0 - r)).max(0.0);
    dx * dx + dy * dy <= r * r
}

fn in_polygon(px: f32, py: f32, pts: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Where everything sits, in device pixels for one icon size.
struct Layout {
    /// Body rectangle (x0, y0, x1, y1), edges snapped to whole pixels.
    body: (f32, f32, f32, f32),
    body_r: f32,
    nub: (f32, f32, f32, f32),
    nub_r: f32,
    bolt: Vec<(f32, f32)>,
    glyphs: Vec<[u8; 5]>,
    cell: f32,
    digits_x: f32,
    digits_y: f32,
}

impl Layout {
    fn new(level: u8, charging: bool, show_percent: bool, size: u32) -> Self {
        let k = size as f32 / GRID;
        let (body_h, nub_h) = if show_percent { (PERCENT_H, 5.0) } else { (PLAIN_H, 4.0) };
        let oy = ((GRID - body_h) / 2.0).floor();
        let snap = |v: f32| (v * k).round();
        let body = (0.0, snap(oy), snap(BODY_W), snap(oy + body_h));
        let nub_y = oy + (body_h - nub_h) / 2.0;
        let nub = (snap(NUB_X), snap(nub_y), snap(NUB_X + NUB_W), snap(nub_y + nub_h));

        let mut glyphs = Vec::new();
        if show_percent {
            if charging && level < 100 {
                glyphs.push(BOLT_GLYPH);
            }
            if level >= 100 {
                glyphs.extend([DIGITS[1], DIGITS[0], DIGITS[0]]);
            } else {
                if level >= 10 {
                    glyphs.push(DIGITS[usize::from(level / 10)]);
                }
                glyphs.push(DIGITS[usize::from(level % 10)]);
            }
        }
        let cells = glyphs.len() as f32 * 4.0 - 1.0;
        let (body_w_px, body_h_px) = (body.2 - body.0, body.3 - body.1);
        Self {
            body,
            body_r: BODY_R * k,
            nub,
            nub_r: k,
            bolt: BOLT.iter().map(|&(x, y)| (body.0 + x * k, body.1 + y * k)).collect(),
            digits_x: (body.0 + (body_w_px - cells * k) / 2.0).round(),
            digits_y: (body.1 + (body_h_px - 5.0 * k) / 2.0).round(),
            cell: k,
            glyphs,
        }
    }

    fn in_digits(&self, px: f32, py: f32) -> bool {
        let row = ((py - self.digits_y) / self.cell).floor();
        let col = ((px - self.digits_x) / self.cell).floor();
        if !(0.0..5.0).contains(&row) || col < 0.0 {
            return false;
        }
        let (row, col) = (row as usize, col as usize);
        let (idx, within) = (col / 4, col % 4);
        within < 3
            && self
                .glyphs
                .get(idx)
                .is_some_and(|g| g[row] >> (2 - within) & 1 == 1)
    }
}

/// Alpha (0..=1) of one sample point in device pixels.
fn sample(px: f32, py: f32, fill_x: f32, charging: bool, l: &Layout) -> f32 {
    let (bx0, by0, bx1, by1) = l.body;
    if in_round_rect(px, py, bx0, by0, bx1, by1, l.body_r) {
        let cut = if l.glyphs.is_empty() {
            charging && in_polygon(px, py, &l.bolt)
        } else {
            l.in_digits(px, py)
        };
        if cut {
            0.0
        } else if px < fill_x {
            1.0
        } else {
            TRACK_ALPHA
        }
    } else if in_round_rect(px, py, l.nub.0, l.nub.1, l.nub.2, l.nub.3, l.nub_r) {
        NUB_ALPHA
    } else {
        0.0
    }
}

/// Icon color as (b, g, r).
fn color(level: u8, charging: bool, light_taskbar: bool) -> (u8, u8, u8) {
    if level <= 20 && !charging {
        (0x3a, 0x45, 0xff) // #ff453a
    } else if light_taskbar {
        (0x1a, 0x1a, 0x1a)
    } else {
        (0xff, 0xff, 0xff)
    }
}

/// Renders a `size` x `size` icon as straight-alpha BGRA (row-major, top-down).
pub fn render(level: u8, charging: bool, light_taskbar: bool, show_percent: bool, size: u32) -> Vec<u8> {
    let level = level.min(100);
    let layout = Layout::new(level, charging, show_percent, size);
    let fill_x = layout.body.0 + (layout.body.2 - layout.body.0) * f32::from(level) / 100.0;
    let (b, g, r) = color(level, charging, light_taskbar);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut acc = 0.0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f32 + (sx as f32 + 0.5) / SS as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SS as f32;
                    acc += sample(x, y, fill_x, charging, &layout);
                }
            }
            let a = (acc / (SS * SS) as f32 * 255.0).round() as u8;
            out.extend_from_slice(&[b, g, r, a]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha_at(buf: &[u8], size: u32, x: u32, y: u32) -> u8 {
        buf[((y * size + x) * 4 + 3) as usize]
    }

    fn opaque_count(buf: &[u8]) -> usize {
        buf.chunks(4).filter(|p| p[3] > 200).count()
    }

    #[test]
    fn edges_are_pixel_aligned_at_16px() {
        let b = render(100, false, false, false, 16);
        assert_eq!(alpha_at(&b, 16, 6, 2), 0);
        assert_eq!(alpha_at(&b, 16, 6, 3), 255);
        assert_eq!(alpha_at(&b, 16, 6, 12), 255);
        assert_eq!(alpha_at(&b, 16, 6, 13), 0);
        assert_eq!(alpha_at(&b, 16, 0, 8), 255);
        assert_eq!(alpha_at(&b, 16, 12, 8), 255);
        assert_eq!(alpha_at(&b, 16, 13, 8), 0);
    }

    #[test]
    fn percent_digits_are_pixel_aligned_at_16px() {
        // "100" at full level: every pixel in the body rows is either body (255) or digit cut-out (0).
        let b = render(100, false, false, true, 16);
        for y in 5..10 {
            for x in 1..12 {
                let a = alpha_at(&b, 16, x, y);
                assert!(a == 0 || a == 255, "pixel ({x},{y}) alpha {a}");
            }
        }
    }
    #[test]
    fn full_battery_body_is_opaque() {
        let buf = render(100, false, false, false, 32);
        assert_eq!(alpha_at(&buf, 32, 6, 16), 255);
    }

    #[test]
    fn empty_battery_shows_only_the_track() {
        let buf = render(0, true, false, false, 32);
        let a = alpha_at(&buf, 32, 6, 16);
        assert!((124..=132).contains(&a), "track alpha was {a}");
    }

    #[test]
    fn corners_are_transparent() {
        let buf = render(100, false, false, false, 32);
        assert_eq!(alpha_at(&buf, 32, 0, 0), 0);
        assert_eq!(alpha_at(&buf, 32, 31, 31), 0);
    }

    #[test]
    fn bolt_cuts_pixels_out_when_charging() {
        let plain = opaque_count(&render(70, false, false, false, 32));
        let charging = opaque_count(&render(70, true, false, false, 32));
        assert!(charging < plain, "{charging} vs {plain}");
    }

    #[test]
    fn red_at_20_or_lower_unless_charging() {
        let low = render(15, false, false, false, 32);
        let px = &low[((16 * 32 + 1) * 4) as usize..][..4];
        assert_eq!((px[2], px[1], px[0]), (0xff, 0x45, 0x3a));
        let chg = render(15, true, false, false, 32);
        let px = &chg[((16 * 32 + 1) * 4) as usize..][..4];
        assert_eq!((px[2], px[1], px[0]), (0xff, 0xff, 0xff));
    }

    #[test]
    fn light_taskbar_uses_dark_icon() {
        let buf = render(100, false, true, false, 32);
        let px = &buf[((16 * 32 + 6) * 4) as usize..][..4];
        assert_eq!((px[2], px[1], px[0], px[3]), (0x1a, 0x1a, 0x1a, 255));
    }

    #[test]
    fn output_size_matches() {
        for s in [16, 20, 24, 32] {
            assert_eq!(render(50, false, false, false, s).len(), (s * s * 4) as usize);
        }
    }

    // ---- visual check: `cargo test dump_contact_sheet -- --ignored` writes a PNG ----

    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    }

    fn png_rgb(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
        let mut raw = Vec::new();
        for row in rgb.chunks((w * 3) as usize) {
            raw.push(0);
            raw.extend_from_slice(row);
        }
        let mut z = vec![0x78, 0x01];
        let mut blocks = raw.chunks(65535).peekable();
        while let Some(block) = blocks.next() {
            z.push(u8::from(blocks.peek().is_none()));
            z.extend_from_slice(&(block.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
            z.extend_from_slice(block);
        }
        let (mut s1, mut s2) = (1u32, 0u32);
        for &b in &raw {
            s1 = (s1 + u32::from(b)) % 65521;
            s2 = (s2 + s1) % 65521;
        }
        z.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    #[ignore]
    fn dump_contact_sheet() {
        const CELL: u32 = 128;
        const UP: u32 = 4;
        let states = [(100u8, false), (60, false), (15, false), (70, true), (5, false), (100, true)];
        let sizes = [16u32, 20, 24, 32];
        for percent in [false, true] {
        let (w, h) = (CELL * states.len() as u32, CELL * (sizes.len() as u32 * 2));
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        for (theme, light) in [false, true].into_iter().enumerate() {
            let bg: u8 = if light { 0xec } else { 0x1f };
            for (row, &size) in sizes.iter().enumerate() {
                for (col, &(level, charging)) in states.iter().enumerate() {
                    let icon = render(level, charging, light, percent, size);
                    let cx = col as u32 * CELL;
                    let cy = (theme * sizes.len() + row) as u32 * CELL;
                    for y in 0..CELL {
                        for x in 0..CELL {
                            let o = (((cy + y) * w + cx + x) * 3) as usize;
                            let (ix, iy) = ((x / UP).wrapping_sub((CELL / UP - size) / 2), (y / UP).wrapping_sub((CELL / UP - size) / 2));
                            let px = if ix < size && iy < size {
                                let p = &icon[((iy * size + ix) * 4) as usize..][..4];
                                let a = f32::from(p[3]) / 255.0;
                                [p[2], p[1], p[0]].map(|c| (f32::from(c) * a + f32::from(bg) * (1.0 - a)).round() as u8)
                            } else {
                                [bg; 3]
                            };
                            rgb[o..o + 3].copy_from_slice(&px);
                        }
                    }
                }
            }
        }
        let name = if percent { "battery-tray-contact-sheet-percent.png" } else { "battery-tray-contact-sheet.png" };
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, png_rgb(w, h, &rgb)).unwrap();
        println!("wrote {}", path.display());
        }
    }

    #[test]
    fn percent_mode_cuts_digits_out_of_the_body() {
        let plain = opaque_count(&render(60, false, false, false, 32));
        let with = opaque_count(&render(60, false, false, true, 32));
        assert_ne!(plain, with);
    }

    #[test]
    fn different_numbers_render_differently() {
        let a = render(60, false, false, true, 32);
        let b = render(61, false, false, true, 32);
        let c = render(100, false, false, true, 32);
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn percent_mode_keeps_size_and_red() {
        assert_eq!(render(5, false, false, true, 24).len(), 24 * 24 * 4);
        let low = render(15, false, false, true, 32);
        // The left edge of the body is filled at 15% and must be red.
        let px = low.chunks(4).find(|p| p[3] == 255).unwrap();
        assert_eq!((px[2], px[1], px[0]), (0xff, 0x45, 0x3a));
    }

    #[test]
    fn layout_glyphs_match_the_number() {
        assert_eq!(Layout::new(7, false, true, 16).glyphs, vec![DIGITS[7]]);
        assert_eq!(Layout::new(60, false, true, 16).glyphs, vec![DIGITS[6], DIGITS[0]]);
        assert_eq!(Layout::new(100, true, true, 16).glyphs, vec![DIGITS[1], DIGITS[0], DIGITS[0]]);
        assert_eq!(Layout::new(60, true, true, 16).glyphs.len(), 3); // bolt + two digits
        assert!(Layout::new(60, false, false, 16).glyphs.is_empty());
    }
}
