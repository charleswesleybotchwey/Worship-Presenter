//! Turns a capture of the NDI output window, drawn on pure green (#00FF00), into
//! BGRA with real transparency for OBS and vMix.
//!
//! Each pixel's "greenness" (green above the larger of red and blue) decides its
//! alpha: the key colour becomes fully see-through, ordinary colours (white and
//! yellow text, dark bands, even mid greens like the live colour) stay opaque, and
//! anti-aliased edges become partly see-through. The key colour is then removed
//! from those edges so text has no green fringe.

/// Pure green used behind the NDI window. Never used for public outputs.
pub const KEY_BGRA: [u8; 4] = [0, 255, 0, 255];

/// Greenness at or below this stays fully opaque. Low, so that see-through parts of
/// the theme (a band at 86 % opacity) and anti-aliased edges keep their real
/// transparency; the cost is that green-tinted theme colours key partly, so the
/// theme editor warns about any colour with greenness above this.
pub const OPAQUE_BELOW: i32 = 8;

/// In place: BGRA (alpha ignored) with green background → BGRA, straight alpha.
pub fn key_in_place(bgra: &mut [u8]) {
    for px in bgra.chunks_exact_mut(4) {
        let (b, g, r) = (px[0] as i32, px[1] as i32, px[2] as i32);
        let greenness = g - b.max(r);
        if greenness <= OPAQUE_BELOW {
            px[3] = 255;
            continue;
        }
        // linear from opaque (OPAQUE_BELOW) to clear (255)
        let t = ((greenness - OPAQUE_BELOW) * 255) / (255 - OPAQUE_BELOW);
        let a = (255 - t).clamp(0, 255);
        if a == 0 {
            px.copy_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        // unmix: observed = a·F + (1−a)·key, key = (b 0, g 255, r 0)
        let fb = (b * 255 / a).min(255);
        let fr = (r * 255 / a).min(255);
        let fg = ((g * 255 - (255 - a) * 255) / a).clamp(0, 255);
        // despill: green never brighter than the other two
        let fg = fg.min(fb.max(fr));
        px[0] = fb as u8;
        px[1] = fg as u8;
        px[2] = fr as u8;
        px[3] = a as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key1(b: u8, g: u8, r: u8) -> [u8; 4] {
        let mut px = [b, g, r, 255];
        key_in_place(&mut px);
        px
    }

    #[test]
    fn background_clear_content_opaque() {
        assert_eq!(key1(0, 255, 0)[3], 0, "key colour is see-through");
        assert_eq!(key1(255, 255, 255), [255, 255, 255, 255], "white text stays");
        assert_eq!(key1(0x66, 0xe0, 0xff), [0x66, 0xe0, 0xff, 255], "Fante yellow stays");
        assert_eq!(key1(0x10, 0x0d, 0x0c), [0x10, 0x0d, 0x0c, 255], "dark band stays");
        assert!(key1(0xaa, 0xd6, 0x8a)[3] < 255, "a mid green keys partly: the theme editor warns about it");
        assert_eq!(key1(0xc0, 0xc0, 0xc0)[3], 255, "neutral grey stays");
    }

    #[test]
    fn translucent_band_keeps_its_opacity() {
        // the built-in lower third band: #0c0d10 at 86 % over the key colour
        let a = 0.86f32;
        let mix = |c: f32, k: f32| (a * c + (1.0 - a) * k).round() as u8;
        let px = key1(mix(16.0, 0.0), mix(13.0, 255.0), mix(12.0, 0.0));
        let got = px[3] as f32 / 255.0;
        assert!((got - a).abs() < 0.06, "band alpha {got} should be about {a}");
        assert!(px[0] < 40 && px[1] < 40 && px[2] < 40, "band colour stays dark {px:?}");
    }

    #[test]
    fn edges_partly_clear_without_green_fringe() {
        // white text edge half over the green background
        let e = key1(128, 255, 128);
        assert!(e[3] > 100 && e[3] < 160, "half-covered edge is about half see-through {e:?}");
        assert!(e[1] <= e[0].max(e[2]), "no green spill {e:?}");
        assert!(e[0] > 200 && e[2] > 200, "edge colour recovers to near white {e:?}");
    }

    #[test]
    fn full_hd_frame_is_fast_enough() {
        let mut f = vec![0u8; 1920 * 1080 * 4];
        for (i, px) in f.chunks_exact_mut(4).enumerate() {
            px.copy_from_slice(if i % 7 == 0 { &[255, 255, 255, 255] } else { &KEY_BGRA });
        }
        let t = std::time::Instant::now();
        key_in_place(&mut f);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("keyed 1920x1080 in {ms:.1} ms");
        // generous: debug builds are slow; release is measured by the app's stats
        assert!(ms < 400.0);
    }
}
