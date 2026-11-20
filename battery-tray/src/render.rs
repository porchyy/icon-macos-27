//! Pure icon renderer. Geometry matches `battery-icon.svg` / `preview.html`
//! (30x14 viewBox) and is drawn with 4x4 supersampling for smooth edges.

const VB_W: f32 = 30.0;
const VB_H: f32 = 14.0;
const BODY_W: f32 = 26.0;
const BODY_R: f32 = 4.5;
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

/// Alpha (0..=1) of one sample point in viewBox coordinates.
fn sample(vx: f32, vy: f32, fill_w: f32, charging: bool) -> f32 {
    if in_round_rect(vx, vy, 0.0, 0.0, BODY_W, VB_H, BODY_R) {
        if charging && in_bolt(vx, vy) {
            0.0
        } else if vx < fill_w {
            1.0
        } else {
            TRACK_ALPHA
        }
    } else if in_round_rect(vx, vy, 27.4, 4.6, 2.0, 4.8, 1.0) {
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
pub fn render(level: u8, charging: bool, light_taskbar: bool, size: u32) -> Vec<u8> {
    let level = level.min(100);
    let fill_w = BODY_W * f32::from(level) / 100.0;
    let scale = size as f32 / VB_W;
    let oy = (size as f32 - VB_H * scale) / 2.0;
    let (b, g, r) = color(level, charging, light_taskbar);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for py in 0..size {
        for px in 0..size {
            let mut acc = 0.0;
            for sy in 0..SS {
                for sx in 0..SS {
                    let vx = (px as f32 + (sx as f32 + 0.5) / SS as f32) / scale;
                    let vy = (py as f32 + (sy as f32 + 0.5) / SS as f32 - oy) / scale;
                    acc += sample(vx, vy, fill_w, charging);
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
        let buf = render(100, false, false, 32);
        assert_eq!(alpha_at(&buf, 32, 6, 16), 255);
    }

    #[test]
    fn empty_battery_shows_only_the_track() {
        let buf = render(0, true, false, 32);
        let a = alpha_at(&buf, 32, 6, 16);
        assert!((78..=86).contains(&a), "track alpha was {a}");
    }

    #[test]
    fn corners_are_transparent() {
        let buf = render(100, false, false, 32);
        assert_eq!(alpha_at(&buf, 32, 0, 0), 0);
        assert_eq!(alpha_at(&buf, 32, 31, 31), 0);
    }

    #[test]
    fn bolt_cuts_pixels_out_when_charging() {
        let plain = opaque_count(&render(70, false, false, 32));
        let charging = opaque_count(&render(70, true, false, 32));
        assert!(charging < plain, "{charging} vs {plain}");
    }

    #[test]
    fn red_at_20_or_lower_unless_charging() {
        let low = render(15, false, false, 32);
        let px = &low[((16 * 32 + 1) * 4) as usize..][..4];
        assert_eq!((px[2], px[1], px[0]), (0xff, 0x45, 0x3a));
        let chg = render(15, true, false, 32);
        let px = &chg[((16 * 32 + 1) * 4) as usize..][..4];
        assert_eq!((px[2], px[1], px[0]), (0xff, 0xff, 0xff));
    }

    #[test]
    fn light_taskbar_uses_dark_icon() {
        let buf = render(100, false, true, 32);
        let px = &buf[((16 * 32 + 6) * 4) as usize..][..4];
        assert_eq!((px[2], px[1], px[0], px[3]), (0x1a, 0x1a, 0x1a, 255));
    }

    #[test]
    fn output_size_matches() {
        for s in [16, 20, 24, 32] {
            assert_eq!(render(50, false, false, s).len(), (s * s * 4) as usize);
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
        let states = [(100u8, false), (60, false), (15, false), (70, true)];
        let sizes = [16u32, 24, 32];
        let (w, h) = (CELL * states.len() as u32, CELL * (sizes.len() as u32 * 2));
        let mut rgb = vec![0u8; (w * h * 3) as usize];
        for (theme, light) in [false, true].into_iter().enumerate() {
            let bg: u8 = if light { 0xec } else { 0x1f };
            for (row, &size) in sizes.iter().enumerate() {
                for (col, &(level, charging)) in states.iter().enumerate() {
                    let icon = render(level, charging, light, size);
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
        let path = std::env::temp_dir().join("battery-tray-contact-sheet.png");
        std::fs::write(&path, png_rgb(w, h, &rgb)).unwrap();
        println!("wrote {}", path.display());
    }
}
