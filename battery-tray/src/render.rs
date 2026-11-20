//! Pure icon renderer. Geometry matches `battery-icon.svg` / `preview.html`
//! (30x14 viewBox) and is drawn with 4x4 supersampling for smooth edges.

const VB_W: f32 = 30.0;
const BODY_W: f32 = 26.0;
/// Plain icon: 14 tall. With the percentage on, the body is taller so digits fit.
const PLAIN_H: f32 = 14.0;
const PLAIN_R: f32 = 4.5;
const PERCENT_H: f32 = 20.0;
const PERCENT_R: f32 = 5.0;
/// Size of one digit "pixel" in viewBox units: about one device pixel at 16 px (100% scaling).
const CELL: f32 = 1.875;
const TRACK_ALPHA: f32 = 0.32;
const NUB_ALPHA: f32 = 0.6;
const SS: u32 = 4;

const BOLT: [(f32, f32); 6] = [
    (14.8, 2.2),
    (8.2, 8.2),
    (12.6, 8.2),
    (11.4, 11.8),
    (18.0, 5.8),
    (13.6, 5.8),
];

fn in_round_rect(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> bool {
    let dx = ((px - (x + w / 2.0)).abs() - (w / 2.0 - r)).max(0.0);
    let dy = ((py - (y + h / 2.0)).abs() - (h / 2.0 - r)).max(0.0);
    dx * dx + dy * dy <= r * r
}

fn in_bolt(px: f32, py: f32) -> bool {
    let mut inside = false;
    let mut j = BOLT.len() - 1;
    for i in 0..BOLT.len() {
        let (xi, yi) = BOLT[i];
        let (xj, yj) = BOLT[j];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

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

/// Where the body and the optional digits sit, in viewBox units.
struct Layout {
    body_h: f32,
    body_r: f32,
    glyphs: Vec<[u8; 5]>,
    digits_x: f32,
    digits_y: f32,
}

impl Layout {
    fn new(level: u8, charging: bool, show_percent: bool) -> Self {
        if !show_percent {
            return Self { body_h: PLAIN_H, body_r: PLAIN_R, glyphs: Vec::new(), digits_x: 0.0, digits_y: 0.0 };
        }
        let mut glyphs = Vec::new();
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
        let cells = glyphs.len() as f32 * 4.0 - 1.0;
        Self {
            body_h: PERCENT_H,
            body_r: PERCENT_R,
            digits_x: (BODY_W - cells * CELL) / 2.0,
            digits_y: (PERCENT_H - 5.0 * CELL) / 2.0,
            glyphs,
        }
    }

    fn in_digits(&self, vx: f32, vy: f32) -> bool {
        let row = ((vy - self.digits_y) / CELL).floor();
        let col = ((vx - self.digits_x) / CELL).floor();
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

/// Alpha (0..=1) of one sample point in viewBox coordinates.
fn sample(vx: f32, vy: f32, fill_w: f32, charging: bool, layout: &Layout) -> f32 {
    if in_round_rect(vx, vy, 0.0, 0.0, BODY_W, layout.body_h, layout.body_r) {
        let cut = if layout.glyphs.is_empty() {
            charging && in_bolt(vx, vy)
        } else {
            layout.in_digits(vx, vy)
        };
        if cut {
            0.0
        } else if vx < fill_w {
            1.0
        } else {
            TRACK_ALPHA
        }
    } else if in_round_rect(vx, vy, 27.4, (layout.body_h - 4.8) / 2.0, 2.0, 4.8, 1.0) {
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
/// The 30x14 shape fills the width and is centered vertically.
pub fn render(level: u8, charging: bool, light_taskbar: bool, show_percent: bool, size: u32) -> Vec<u8> {
    let level = level.min(100);
    let fill_w = BODY_W * f32::from(level) / 100.0;
    let layout = Layout::new(level, charging, show_percent);
    let scale = size as f32 / VB_W;
    let oy = (size as f32 - layout.body_h * scale) / 2.0;
    let (b, g, r) = color(level, charging, light_taskbar);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut acc = 0.0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let vx = (px as f32 + (sx as f32 + 0.5) / SS as f32) / scale;
                    let vy = (py as f32 + (sy as f32 + 0.5) / SS as f32 - oy) / scale;
                    acc += sample(vx, vy, fill_w, charging, &layout);
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
    fn full_battery_body_is_opaque() {
        let buf = render(100, false, false, false, 32);
        assert_eq!(alpha_at(&buf, 32, 6, 16), 255);
    }

    #[test]
    fn empty_battery_shows_only_the_track() {
        let buf = render(0, true, false, false, 32);
        let a = alpha_at(&buf, 32, 6, 16);
        assert!((78..=86).contains(&a), "track alpha was {a}");
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
        let sizes = [16u32, 24, 32];
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
        assert_eq!(Layout::new(7, false, true).glyphs, vec![DIGITS[7]]);
        assert_eq!(Layout::new(60, false, true).glyphs, vec![DIGITS[6], DIGITS[0]]);
        assert_eq!(Layout::new(100, true, true).glyphs, vec![DIGITS[1], DIGITS[0], DIGITS[0]]);
        assert_eq!(Layout::new(60, true, true).glyphs.len(), 3); // bolt + two digits
        assert!(Layout::new(60, false, false).glyphs.is_empty());
    }
}
