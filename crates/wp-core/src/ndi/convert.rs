//! BGRA (straight alpha) to NDI's own UYVA layout: a UYVY 4:2:2 plane (BT.709,
//! video levels: Y 16–235, U/V 16–240, as OBS and vMix expect for HD) followed by a
//! full-resolution alpha plane. Sending NDI's native layout saves the runtime a
//! conversion, and the alpha plane keeps the exact transparency.

/// Width must be even (UYVY pairs pixels); an odd last column is dropped.
pub fn bgra_to_uyva(bgra: &[u8], width: usize, height: usize) -> Vec<u8> {
    let w = width & !1;
    let mut out = vec![0u8; w * 2 * height + w * height];
    let (uyvy, alpha) = out.split_at_mut(w * 2 * height);
    for y in 0..height {
        let row = &bgra[y * width * 4..];
        let o = &mut uyvy[y * w * 2..(y + 1) * w * 2];
        let a = &mut alpha[y * w..(y + 1) * w];
        for x in (0..w).step_by(2) {
            let p0 = &row[x * 4..x * 4 + 4];
            let p1 = &row[x * 4 + 4..x * 4 + 8];
            let (b0, g0, r0) = (p0[0] as i32, p0[1] as i32, p0[2] as i32);
            let (b1, g1, r1) = (p1[0] as i32, p1[1] as i32, p1[2] as i32);
            // fixed point, coefficients ×1024 (BT.709 limited range)
            let luma = |r: i32, g: i32, b: i32| (16 * 1024 + 187 * r + 629 * g + 63 * b + 512) >> 10;
            let (r, g, b) = ((r0 + r1) / 2, (g0 + g1) / 2, (b0 + b1) / 2);
            let u = (128 * 1024 - 103 * r - 347 * g + 450 * b + 512) >> 10;
            let v = (128 * 1024 + 450 * r - 409 * g - 41 * b + 512) >> 10;
            o[x * 2] = u.clamp(16, 240) as u8;
            o[x * 2 + 1] = luma(r0, g0, b0).clamp(16, 235) as u8;
            o[x * 2 + 2] = v.clamp(16, 240) as u8;
            o[x * 2 + 3] = luma(r1, g1, b1).clamp(16, 235) as u8;
            a[x] = p0[3];
            a[x + 1] = p1[3];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(b: u8, g: u8, r: u8, a: u8) -> (u8, u8, u8, u8) {
        let px = [b, g, r, a, b, g, r, a];
        let o = bgra_to_uyva(&px, 2, 1);
        (o[1], o[0], o[2], o[4]) // Y, U, V, alpha
    }

    #[test]
    fn bt709_levels() {
        assert_eq!(one(255, 255, 255, 255), (235, 128, 128, 255), "white");
        assert_eq!(one(0, 0, 0, 0), (16, 128, 128, 0), "black, clear");
        let (y, u, v, _) = one(0, 0, 255, 255);
        assert!((y as i32 - 63).abs() <= 1 && (u as i32 - 102).abs() <= 1 && (v as i32 - 240).abs() <= 1, "red {y} {u} {v}");
        let (y, ..) = one(0, 255, 0, 128);
        assert!((y as i32 - 173).abs() <= 1, "green {y}");
    }

    #[test]
    fn layout_and_speed() {
        let (w, h) = (1920usize, 1080usize);
        let src = vec![200u8; w * h * 4];
        let t = std::time::Instant::now();
        let o = bgra_to_uyva(&src, w, h);
        println!("1080p BGRA→UYVA {:?}", t.elapsed());
        assert_eq!(o.len(), w * h * 3, "UYVY plane (2 bytes/pixel) + alpha plane (1 byte/pixel)");
    }
}
