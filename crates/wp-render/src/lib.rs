//! Worship Presenter's built-in renderer. It draws a theme (the same JSON the screens
//! use) straight into pixels, without a browser window, so NDI gets exact transparency
//! (no green keying) and no hidden window is needed. Text is shaped properly (ɛ ɔ ŋ and
//! tone marks), with Noto Sans built in and system fonts for everything else.
//!
//! Supported: transparent/colour/picture backgrounds with dimming; boxes (fill, rounded
//! corners, outline); pictures (cover/contain); text fields with font, size, weight,
//! italic, colour, alignment, line height, capitals, shrink-to-fit, shadow and outline;
//! the alert band and live subtitles. Not drawn: video backgrounds, tickers, animations.

use cosmic_text::{Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

/// What a slide shows.
#[derive(Debug, Clone)]
pub struct Content {
    pub primary: Vec<String>,
    pub second: Vec<String>,
    pub reference: Option<String>,
    pub title: Option<String>,
    pub copyright: Option<String>,
    pub section: Option<String>,
    pub clock: String,
    /// "Clear": the theme without its words.
    pub hide_text: bool,
    pub alert: Option<String>,
    pub subtitle: Option<String>,
    /// The operator's quick change for this item: 1.0 = the theme's size.
    pub text_scale: f32,
    /// Moved down (+) or up (−), in % of the height.
    pub text_shift: f32,
    /// The item's first / last slide (for layers shown only there).
    pub first: bool,
    pub last: bool,
}

impl Default for Content {
    fn default() -> Self {
        Content {
            primary: vec![],
            second: vec![],
            reference: None,
            title: None,
            copyright: None,
            section: None,
            clock: String::new(),
            hide_text: false,
            alert: None,
            subtitle: None,
            text_scale: 1.0,
            text_shift: 0.0,
            first: true,
            last: true,
        }
    }
}

/// The church's brand kit (Settings → Brand kit): themes may refer to its colours as
/// "brand:N" (or "brand:N/AA" with opacity) and to its fonts as "brand"/"brand-heading".
#[derive(Debug, Clone, PartialEq)]
pub struct Brand {
    pub colors: [String; 4],
    pub font: String,
    pub heading: String,
}

impl Brand {
    pub fn from_settings(colors: Option<&str>, font: Option<&str>, heading: Option<&str>) -> Brand {
        let mut c = ["#1f4e9c".to_string(), "#f2b705".into(), "#ffffff".into(), "#0d1424".into()];
        if let Some(Ok(Value::Array(v))) = colors.map(serde_json::from_str::<Value>) {
            for (i, x) in v.iter().take(4).enumerate() {
                if let Some(h) = x.as_str().filter(|h| h.len() == 7 && h.starts_with('#')) {
                    c[i] = h.to_string();
                }
            }
        }
        let font = font.filter(|f| !f.is_empty()).unwrap_or("Noto Sans").to_string();
        let heading = heading.filter(|f| !f.is_empty()).map(str::to_string).unwrap_or_else(|| font.clone());
        Brand { colors: c, font, heading }
    }

    /// A number that changes when the brand changes (for caches).
    pub fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (&self.colors, &self.font, &self.heading).hash(&mut h);
        h.finish()
    }

    fn color(&self, v: &str) -> Option<String> {
        let rest = v.strip_prefix("brand:")?;
        let (n, alpha) = rest.split_once('/').map(|(n, a)| (n, Some(a))).unwrap_or((rest, None));
        let base = self.colors.get(n.parse::<usize>().ok()?.checked_sub(1)?)?.clone();
        Some(match alpha.filter(|a| a.len() == 2 && !a.eq_ignore_ascii_case("ff")) {
            Some(a) => format!("{base}{a}"),
            None => base,
        })
    }

    /// The theme with brand references replaced by the real colours and fonts.
    pub fn resolve(&self, t: &Value) -> Value {
        fn walk(b: &Brand, v: &Value, key: &str) -> Value {
            match v {
                Value::String(s) if key == "font" && s == "brand" => Value::String(b.font.clone()),
                Value::String(s) if key == "font" && s == "brand-heading" => Value::String(b.heading.clone()),
                Value::String(s) => b.color(s).map(Value::String).unwrap_or_else(|| v.clone()),
                Value::Array(a) => Value::Array(a.iter().map(|x| walk(b, x, "")).collect()),
                Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), walk(b, x, k))).collect()),
                _ => v.clone(),
            }
        }
        walk(self, t, "")
    }
}

pub struct Renderer {
    fonts: FontSystem,
    cache: SwashCache,
    images: HashMap<PathBuf, Option<Pixmap>>,
}

const NOTO: [&[u8]; 4] = [
    include_bytes!("../fonts/NotoSans-Regular.ttf"),
    include_bytes!("../fonts/NotoSans-SemiBold.ttf"),
    include_bytes!("../fonts/NotoSans-Bold.ttf"),
    include_bytes!("../fonts/NotoSans-Italic.ttf"),
];

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    /// Loads the computer's fonts once (about half a second) plus the built-in Noto Sans.
    pub fn new() -> Self {
        let mut fonts = FontSystem::new();
        for f in NOTO {
            fonts.db_mut().load_font_data(f.to_vec());
        }
        Renderer { fonts, cache: SwashCache::new(), images: HashMap::new() }
    }

    /// Draws one frame. Returns BGRA with straight (not premultiplied) alpha.
    pub fn render(&mut self, theme: &Value, c: &Content, width: u32, height: u32, media_dir: &Path) -> Vec<u8> {
        let mut px = Pixmap::new(width, height).expect("frame size");
        let (w, h) = (width as f32, height as f32);
        let bg = &theme["background"];
        match bg["kind"].as_str().unwrap_or("color") {
            "transparent" => {}
            "gradient" => {
                let full = Rect::from_xywh(0.0, 0.0, w, h).unwrap();
                if let Some(sh) = gradient_shader(&bg["gradient"], full, 1.0) {
                    fill_rect_shader(&mut px, full, sh, 0.0);
                }
            }
            kind => {
                if let Some(col) = parse_color(bg["color"].as_str().unwrap_or("#000000")) {
                    px.fill(col);
                }
                if kind == "image" {
                    if let Some(img) = self.image(bg["image"].as_str().unwrap_or(""), media_dir) {
                        draw_image(&mut px, &img, Rect::from_xywh(0.0, 0.0, w, h).unwrap(), bg["fit"].as_str() != Some("contain"), 1.0);
                    }
                    let dim = bg["dim"].as_f64().unwrap_or(0.0) as f32;
                    if dim > 0.0 {
                        fill_rect(&mut px, Rect::from_xywh(0.0, 0.0, w, h).unwrap(), tiny_skia::Color::from_rgba(0.0, 0.0, 0.0, dim.min(1.0)).unwrap(), 0.0, None);
                    }
                }
            }
        }
        let empty = Vec::new();
        // a chorus, bridge… may look different from the verses
        let sec_kind = c.section.as_deref().map(|s| s.to_lowercase()).and_then(|s| {
            if s.starts_with("pre-chorus") { Some("pre-chorus") } else { ["chorus", "bridge", "tag", "ending"].into_iter().find(|k| s.starts_with(k)) }
        });
        let sec_style = sec_kind.map(|k| &theme["sections"][k]).filter(|v| v.is_object());
        for l in theme["layers"].as_array().unwrap_or(&empty) {
            if l["hidden"].as_bool() == Some(true) {
                continue;
            }
            match l["onlyOn"].as_str() {
                Some("first") if !c.first => continue,
                Some("last") if !c.last => continue,
                _ => {}
            }
            let opacity = l["opacity"].as_f64().unwrap_or(1.0) as f32;
            let rect = |r: &Value| Rect::from_xywh(pc(r["x"].as_f64()) * w, pc(r["y"].as_f64()) * h, pc(r["w"].as_f64()) * w, pc(r["h"].as_f64()) * h);
            match l["kind"].as_str().unwrap_or("") {
                "shape" => {
                    let Some(r) = rect(l) else { continue };
                    let fill = parse_color(l["fill"].as_str().unwrap_or("#00000000"));
                    let radius = l["radius"].as_f64().unwrap_or(0.0) as f32 / 100.0 * h;
                    let border = &l["border"];
                    let bw = border["width"].as_f64().unwrap_or(0.0) as f32 / 100.0 * h;
                    let bc = parse_color(border["color"].as_str().unwrap_or("#00000000"));
                    match gradient_shader(&l["gradient"], r, opacity) {
                        Some(sh) => fill_rect_shader(&mut px, r, sh, radius),
                        None => {
                            if let Some(f) = fill {
                                fill_rect(&mut px, r, with_alpha(f, opacity), radius, None);
                            }
                        }
                    }
                    if bw > 0.0 {
                        if let Some(b) = bc {
                            fill_rect(&mut px, r, with_alpha(b, opacity), radius, Some(bw));
                        }
                    }
                }
                "image" => {
                    let Some(r) = rect(l) else { continue };
                    if let Some(img) = self.image(l["src"].as_str().unwrap_or(""), media_dir) {
                        draw_image(&mut px, &img, r, l["fit"].as_str() != Some("contain"), opacity);
                    }
                }
                "text" => {
                    // Clear: every text goes, except a clock
                    if c.hide_text && l["field"].as_str() != Some("clock") {
                        continue;
                    }
                    let lines: Vec<String> = match l["field"].as_str().unwrap_or("") {
                        "lyrics" => c.primary.clone(),
                        "second" => c.second.clone(),
                        "reference" => c.reference.clone().into_iter().collect(),
                        "title" => c.title.clone().into_iter().collect(),
                        "copyright" => c.copyright.clone().into_iter().collect(),
                        "section" => c.section.clone().into_iter().collect(),
                        "clock" => vec![c.clock.clone()],
                        "free" => l["text"].as_str().unwrap_or("").lines().map(str::to_string).collect(),
                        _ => vec![],
                    };
                    if lines.iter().all(|s| s.trim().is_empty()) {
                        continue;
                    }
                    // the box to use when there is no second language
                    let r = if l["field"] == "lyrics" && c.second.is_empty() && l["solo"].is_object() { rect(&l["solo"]) } else { rect(l) };
                    let Some(r) = r else { continue };
                    let field = l["field"].as_str().unwrap_or("");
                    let mut style = l["style"].clone();
                    if let (Some(sec), true) = (sec_style, field == "lyrics" || field == "second") {
                        if let Some(i) = sec["italic"].as_bool() {
                            style["italic"] = i.into();
                        }
                        if let Some(w) = sec["weight"].as_u64() {
                            style["weight"] = w.into();
                        }
                        if let (Some(col), "lyrics") = (sec["color"].as_str(), field) {
                            style["color"] = col.into();
                        }
                    }
                    // the operator's quick change applies to the slide's own words
                    let r = if matches!(field, "lyrics" | "second" | "reference" | "section") && (c.text_scale != 1.0 || c.text_shift != 0.0) {
                        for k in ["size", "minSize"] {
                            if let Some(v) = style[k].as_f64() {
                                style[k] = (v * c.text_scale as f64).into();
                            }
                        }
                        Rect::from_xywh(r.left(), r.top() + c.text_shift / 100.0 * h, r.width(), r.height()).unwrap_or(r)
                    } else {
                        r
                    };
                    self.text(&mut px, r, &lines.join("\n"), &style, opacity);
                }
                _ => {}
            }
        }
        // the alert band across the top, and live subtitles near the bottom
        if let Some(a) = c.alert.as_ref().filter(|a| !a.trim().is_empty()) {
            let band = Rect::from_xywh(0.0, 0.0, w, h * 0.085).unwrap();
            fill_rect(&mut px, band, tiny_skia::Color::from_rgba8(12, 13, 16, 220), 0.0, None);
            self.text(&mut px, Rect::from_xywh(w * 0.03, 0.0, w * 0.94, h * 0.085).unwrap(), a, &band_style(3.6, "middle"), 1.0);
        }
        if let Some(sub) = c.subtitle.as_ref().filter(|s| !s.trim().is_empty()) {
            let size = 3.2 / 100.0 * w / h * 100.0; // the screens size it by width
            let area = Rect::from_xywh(w * 0.08, h * 0.955 - size / 100.0 * h * 2.8, w * 0.84, size / 100.0 * h * 2.8).unwrap();
            let words: Vec<&str> = sub.split_whitespace().collect();
            let text = words[words.len().saturating_sub(22)..].join(" ");
            let used = self.measure(area.width(), &text, &band_style(size, "bottom"));
            let bh = used.1 + size / 100.0 * h * 0.7;
            let bw = (used.0 + size / 100.0 * h * 1.6).min(area.width());
            let band = Rect::from_xywh(w / 2.0 - bw / 2.0, h * 0.955 - bh, bw, bh).unwrap();
            fill_rect(&mut px, band, tiny_skia::Color::from_rgba8(0, 0, 0, 184), size / 100.0 * h * 0.3, None);
            self.text(&mut px, band, &text, &band_style(size, "middle"), 1.0);
        }
        to_bgra_straight(&px)
    }

    fn image(&mut self, src: &str, media_dir: &Path) -> Option<Pixmap> {
        let rel = src.strip_prefix("media:")?;
        let path = media_dir.join(rel);
        if let Some(p) = self.images.get(&path) {
            return p.clone();
        }
        let loaded = image::open(&path).ok().and_then(|i| {
            let rgba = i.to_rgba8();
            let (iw, ih) = rgba.dimensions();
            let mut data = rgba.into_raw();
            for p in data.chunks_exact_mut(4) {
                let a = p[3] as u16;
                for c in &mut p[..3] {
                    *c = ((*c as u16 * a + 127) / 255) as u8;
                }
            }
            Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(iw, ih)?)
        });
        self.images.insert(path, loaded.clone());
        loaded
    }

    fn buffer(&mut self, width: f32, text: &str, s: &Value, size_px: f32) -> Buffer {
        let line_h = size_px * s["lineHeight"].as_f64().unwrap_or(1.2) as f32;
        let mut buf = Buffer::new(&mut self.fonts, Metrics::new(size_px, line_h));
        buf.set_size(&mut self.fonts, Some(width.max(1.0)), None);
        let family = s["font"].as_str().unwrap_or("Noto Sans").to_string();
        let attrs = Attrs::new()
            .family(Family::Name(&family))
            .weight(Weight(s["weight"].as_u64().unwrap_or(400) as u16))
            .style(if s["italic"].as_bool() == Some(true) { Style::Italic } else { Style::Normal });
        let align = match s["align"].as_str() {
            Some("left") => Align::Left,
            Some("right") => Align::Right,
            _ => Align::Center,
        };
        let shown = if s["uppercase"].as_bool() == Some(true) { text.to_uppercase() } else { text.to_string() };
        buf.set_text(&mut self.fonts, &shown, &attrs, Shaping::Advanced, Some(align));
        buf.shape_until_scroll(&mut self.fonts, false);
        buf
    }

    /// Width of the widest line and the total height, for a text at its style's size.
    fn measure(&mut self, width: f32, text: &str, s: &Value) -> (f32, f32) {
        let size_px = s["size"].as_f64().unwrap_or(5.0) as f32 / 100.0 * 1080.0;
        let buf = self.buffer(width, text, s, size_px);
        extent(&buf)
    }

    fn text(&mut self, px: &mut Pixmap, r: Rect, text: &str, s: &Value, opacity: f32) {
        let h = px.height() as f32;
        // ProPresenter-style box behind the words: the padding comes out of the text area
        let bx = &s["box"];
        let mode = bx["mode"].as_str().unwrap_or("off");
        let (pad_x, pad_y) = if mode == "off" { (0.0, 0.0) } else { (bx["padX"].as_f64().unwrap_or(2.0) as f32 / 100.0 * h, bx["padY"].as_f64().unwrap_or(1.0) as f32 / 100.0 * h) };
        let outer = r;
        let Some(r) = Rect::from_xywh(r.left() + pad_x, r.top() + pad_y, (r.width() - 2.0 * pad_x).max(1.0), (r.height() - 2.0 * pad_y).max(1.0)) else { return };
        let mut size = s["size"].as_f64().unwrap_or(5.0) as f32;
        // shrink to fit, as the screens do: words are never cut off, so below the theme's
        // smallest size too (down to a quarter) if they still would not fit
        let min = size * 0.25;
        let mut buf = self.buffer(r.width(), text, s, size / 100.0 * h);
        {
            while size > min {
                let (ew, eh) = extent(&buf);
                if eh <= r.height() + 1.0 && ew <= r.width() + 1.0 {
                    break;
                }
                size = (size * 0.95).max(min);
                buf = self.buffer(r.width(), text, s, size / 100.0 * h);
            }
        }
        let (_, eh) = extent(&buf);
        let top = match s["valign"].as_str() {
            Some("top") => r.top(),
            Some("bottom") => r.bottom() - eh,
            _ => r.top() + (r.height() - eh) / 2.0,
        };
        if mode == "block" || mode == "lines" {
            if let Some(col) = parse_color(bx["color"].as_str().unwrap_or("#000000d9")) {
                let radius = bx["radius"].as_f64().unwrap_or(0.6) as f32 / 100.0 * h;
                // each line's inked extent (alignment is already in the glyph positions)
                let mut lines: Vec<(f32, f32, f32, f32)> = vec![];
                for run in buf.layout_runs() {
                    let (mut x0, mut x1) = (f32::MAX, f32::MIN);
                    for g in run.glyphs.iter() {
                        x0 = x0.min(g.x);
                        x1 = x1.max(g.x + g.w);
                    }
                    if x0 <= x1 {
                        lines.push((x0, x1, run.line_top, run.line_height));
                    }
                }
                let col = with_alpha(col, opacity);
                let blur = (bx["blur"].as_f64().unwrap_or(0.0) as f32 / 100.0 * h).round() as usize;
                if mode == "block" && !lines.is_empty() {
                    let x0 = lines.iter().map(|l| l.0).fold(f32::MAX, f32::min);
                    let x1 = lines.iter().map(|l| l.1).fold(f32::MIN, f32::max);
                    let y0 = lines.first().unwrap().2;
                    let y1 = lines.last().map(|l| l.2 + l.3).unwrap();
                    if let Some(b) = Rect::from_ltrb(r.left() + x0 - pad_x, top + y0 - pad_y, r.left() + x1 + pad_x, top + y1 + pad_y) {
                        let b = b.intersect(&outer).unwrap_or(b);
                        if blur > 0 {
                            blur_region(px, b, blur);
                        }
                        fill_rect(px, b, col, radius, None);
                    }
                } else {
                    for (x0, x1, y, lh) in &lines {
                        if let Some(b) = Rect::from_ltrb(r.left() + x0 - pad_x, top + y - pad_y * 0.5, r.left() + x1 + pad_x, top + y + lh + pad_y * 0.5) {
                            if blur > 0 {
                                blur_region(px, b, blur);
                            }
                            fill_rect(px, b, col, radius, None);
                        }
                    }
                }
            }
        }
        // glyph coverage into a mask the size of the frame
        let (fw, fh) = (px.width() as i32, px.height() as i32);
        let mut mask = vec![0u8; (fw * fh) as usize];
        let (ox, oy) = (r.left().round() as i32, top.round() as i32);
        buf.draw(&mut self.fonts, &mut self.cache, Color::rgba(255, 255, 255, 255), |x, y, w, hh, col| {
            let a = col.a();
            if a == 0 {
                return;
            }
            for yy in 0..hh as i32 {
                for xx in 0..w as i32 {
                    let (gx, gy) = (ox + x + xx, oy + y + yy);
                    if gx >= 0 && gy >= 0 && gx < fw && gy < fh {
                        let i = (gy * fw + gx) as usize;
                        mask[i] = mask[i].max(a);
                    }
                }
            }
        });
        let unit = h / 100.0;
        let sh = &s["shadow"];
        if sh["on"].as_bool() == Some(true) {
            if let Some(col) = parse_color(sh["color"].as_str().unwrap_or("#000000cc")) {
                let dx = (sh["x"].as_f64().unwrap_or(0.0) as f32 * unit).round() as i32;
                let dy = (sh["y"].as_f64().unwrap_or(0.0) as f32 * unit).round() as i32;
                let blur = (sh["blur"].as_f64().unwrap_or(0.5) as f32 * unit / 2.0).round().max(0.0) as usize;
                let shadow = blurred(&shifted(&mask, fw as usize, fh as usize, dx, dy), fw as usize, fh as usize, blur);
                paint_mask(px, &shadow, with_alpha(col, opacity));
            }
        }
        let ol = &s["outline"];
        if ol["on"].as_bool() == Some(true) {
            if let Some(col) = parse_color(ol["color"].as_str().unwrap_or("#000000")) {
                let rad = (ol["width"].as_f64().unwrap_or(0.15) as f32 * unit).round().max(1.0) as usize;
                paint_mask(px, &dilated(&mask, fw as usize, fh as usize, rad), with_alpha(col, opacity));
            }
        }
        let g = &s["gradient"];
        match (parse_color(g["from"].as_str().unwrap_or("")), parse_color(g["to"].as_str().unwrap_or(""))) {
            (Some(a), Some(b)) if g.is_object() => paint_mask_gradient(px, &mask, outer, a, b, g["angle"].as_f64().unwrap_or(180.0) as f32, opacity),
            _ => {
                if let Some(col) = parse_color(s["color"].as_str().unwrap_or("#ffffff")) {
                    paint_mask(px, &mask, with_alpha(col, opacity));
                }
            }
        }
    }
}

fn extent(buf: &Buffer) -> (f32, f32) {
    let mut w: f32 = 0.0;
    let mut bottom: f32 = 0.0;
    for run in buf.layout_runs() {
        w = w.max(run.line_w);
        bottom = bottom.max(run.line_top + run.line_height);
    }
    (w, bottom)
}

fn band_style(size: f32, valign: &str) -> Value {
    serde_json::json!({ "font": "Noto Sans", "size": size, "weight": 600, "color": "#ffffff", "align": "center", "valign": valign, "lineHeight": 1.3, "autoFit": true, "minSize": size * 0.7 })
}

fn pc(v: Option<f64>) -> f32 {
    (v.unwrap_or(0.0) / 100.0) as f32
}

/// "#rgb", "#rrggbb" or "#rrggbbaa".
pub fn parse_color(s: &str) -> Option<tiny_skia::Color> {
    let h = s.trim().strip_prefix('#')?;
    let hex = |i: usize, n: usize| u8::from_str_radix(&h[i..i + n], 16).ok();
    let (r, g, b, a) = match h.len() {
        3 => (hex(0, 1)? * 17, hex(1, 1)? * 17, hex(2, 1)? * 17, 255),
        6 => (hex(0, 2)?, hex(2, 2)?, hex(4, 2)?, 255),
        8 => (hex(0, 2)?, hex(2, 2)?, hex(4, 2)?, hex(6, 2)?),
        _ => return None,
    };
    Some(tiny_skia::Color::from_rgba8(r, g, b, a))
}

fn with_alpha(c: tiny_skia::Color, opacity: f32) -> tiny_skia::Color {
    let mut c = c;
    c.apply_opacity(opacity);
    c
}

/// A rectangle with rounded corners (square when the radius is tiny).
fn rounded(r: Rect, radius: f32) -> Option<tiny_skia::Path> {
    if radius <= 0.5 {
        return Some(PathBuilder::from_rect(r));
    }
    let rad = radius.min(r.width() / 2.0).min(r.height() / 2.0);
    let (l, t, rt, b) = (r.left(), r.top(), r.right(), r.bottom());
    let k = 0.552_284_8 * rad;
    let mut pb = PathBuilder::new();
    pb.move_to(l + rad, t);
    pb.line_to(rt - rad, t);
    pb.cubic_to(rt - rad + k, t, rt, t + rad - k, rt, t + rad);
    pb.line_to(rt, b - rad);
    pb.cubic_to(rt, b - rad + k, rt - rad + k, b, rt - rad, b);
    pb.line_to(l + rad, b);
    pb.cubic_to(l + rad - k, b, l, b - rad + k, l, b - rad);
    pb.line_to(l, t + rad);
    pb.cubic_to(l, t + rad - k, l + rad - k, t, l + rad, t);
    pb.close();
    pb.finish()
}

fn fill_rect(px: &mut Pixmap, r: Rect, col: tiny_skia::Color, radius: f32, stroke: Option<f32>) {
    let mut paint = Paint::default();
    paint.set_color(col);
    paint.anti_alias = true;
    let path = rounded(r, radius);
    let Some(path) = path else { return };
    match stroke {
        Some(width) => {
            let st = Stroke { width, ..Stroke::default() };
            px.stroke_path(&path, &paint, &st, Transform::identity(), None);
        }
        None => {
            px.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
}

fn draw_image(px: &mut Pixmap, img: &Pixmap, r: Rect, cover: bool, opacity: f32) {
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let scale = if cover { (r.width() / iw).max(r.height() / ih) } else { (r.width() / iw).min(r.height() / ih) };
    let (dw, dh) = (iw * scale, ih * scale);
    let tx = r.left() + (r.width() - dw) / 2.0;
    let ty = r.top() + (r.height() - dh) / 2.0;
    let clip = tiny_skia::Mask::from_pixmap(
        {
            let mut m = Pixmap::new(px.width(), px.height()).unwrap();
            let mut p = Paint::default();
            p.set_color(tiny_skia::Color::WHITE);
            m.fill_rect(r, &p, Transform::identity(), None);
            m
        }
        .as_ref(),
        tiny_skia::MaskType::Alpha,
    );
    let paint = PixmapPaint { opacity, quality: tiny_skia::FilterQuality::Bilinear, ..PixmapPaint::default() };
    px.draw_pixmap(0, 0, img.as_ref(), &paint, Transform::from_row(scale, 0.0, 0.0, scale, tx, ty), Some(&clip));
}

fn paint_mask(px: &mut Pixmap, mask: &[u8], col: tiny_skia::Color) {
    let c = col.premultiply().to_color_u8();
    let data = px.data_mut();
    for (i, &m) in mask.iter().enumerate() {
        if m == 0 {
            continue;
        }
        let a = m as u32 * c.alpha() as u32 / 255;
        let o = i * 4;
        let inv = 255 - a;
        let src = [c.red() as u32 * m as u32 / 255, c.green() as u32 * m as u32 / 255, c.blue() as u32 * m as u32 / 255, a];
        for k in 0..4 {
            data[o + k] = (src[k] + data[o + k] as u32 * inv / 255).min(255) as u8;
        }
    }
}

/// CSS linear-gradient geometry: 0° points up, 90° right; the line spans the box.
fn gradient_line(r: Rect, angle: f32) -> (tiny_skia::Point, tiny_skia::Point) {
    let a = angle.to_radians();
    let (dx, dy) = (a.sin(), -a.cos());
    let len = (r.width() * dx).abs() + (r.height() * dy).abs();
    let (cx, cy) = (r.left() + r.width() / 2.0, r.top() + r.height() / 2.0);
    (tiny_skia::Point::from_xy(cx - dx * len / 2.0, cy - dy * len / 2.0), tiny_skia::Point::from_xy(cx + dx * len / 2.0, cy + dy * len / 2.0))
}

fn gradient_shader(g: &Value, r: Rect, opacity: f32) -> Option<tiny_skia::Shader<'static>> {
    if !g.is_object() {
        return None;
    }
    let from = with_alpha(parse_color(g["from"].as_str()?)?, opacity);
    let to = with_alpha(parse_color(g["to"].as_str()?)?, opacity);
    let (a, b) = gradient_line(r, g["angle"].as_f64().unwrap_or(180.0) as f32);
    tiny_skia::LinearGradient::new(a, b, vec![tiny_skia::GradientStop::new(0.0, from), tiny_skia::GradientStop::new(1.0, to)], tiny_skia::SpreadMode::Pad, Transform::identity())
}

fn fill_rect_shader(px: &mut Pixmap, r: Rect, shader: tiny_skia::Shader<'static>, radius: f32) {
    let mut paint = Paint::default();
    paint.shader = shader;
    paint.anti_alias = true;
    if let Some(path) = rounded(r, radius) {
        px.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Words filled with a gradient across their layer.
fn paint_mask_gradient(px: &mut Pixmap, mask: &[u8], r: Rect, from: tiny_skia::Color, to: tiny_skia::Color, angle: f32, opacity: f32) {
    let w = px.width() as usize;
    let (a, b) = gradient_line(r, angle);
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let len2 = (vx * vx + vy * vy).max(1.0);
    let (f, t) = (from.to_color_u8(), to.to_color_u8());
    let data = px.data_mut();
    for (i, &m) in mask.iter().enumerate() {
        if m == 0 {
            continue;
        }
        let (x, y) = ((i % w) as f32, (i / w) as f32);
        let k = (((x - a.x) * vx + (y - a.y) * vy) / len2).clamp(0.0, 1.0);
        let mix = |p: u8, q: u8| p as f32 + (q as f32 - p as f32) * k;
        let alpha = mix(f.alpha(), t.alpha()) / 255.0 * opacity * m as f32 / 255.0;
        let src = [mix(f.red(), t.red()) * alpha, mix(f.green(), t.green()) * alpha, mix(f.blue(), t.blue()) * alpha, alpha * 255.0];
        let o = i * 4;
        for c in 0..4 {
            data[o + c] = (src[c] + data[o + c] as f32 * (1.0 - alpha)).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Frosted glass: blurs what is already drawn inside a rectangle.
fn blur_region(px: &mut Pixmap, r: Rect, radius: usize) {
    let (fw, fh) = (px.width() as i32, px.height() as i32);
    let (x0, y0) = ((r.left().floor() as i32).clamp(0, fw), (r.top().floor() as i32).clamp(0, fh));
    let (x1, y1) = ((r.right().ceil() as i32).clamp(0, fw), (r.bottom().ceil() as i32).clamp(0, fh));
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    if w < 2 || h < 2 || radius == 0 {
        return;
    }
    let data = px.data_mut();
    for c in 0..4 {
        let chan: Vec<u8> = (0..h).flat_map(|y| (0..w).map(move |x| (y, x))).map(|(y, x)| data[((y0 as usize + y) * fw as usize + x0 as usize + x) * 4 + c]).collect();
        let out = blurred(&chan, w, h, radius);
        for y in 0..h {
            for x in 0..w {
                data[((y0 as usize + y) * fw as usize + x0 as usize + x) * 4 + c] = out[y * w + x];
            }
        }
    }
}

fn shifted(mask: &[u8], w: usize, h: usize, dx: i32, dy: i32) -> Vec<u8> {
    let mut out = vec![0u8; w * h];
    for y in 0..h as i32 {
        let sy = y - dy;
        if sy < 0 || sy >= h as i32 {
            continue;
        }
        for x in 0..w as i32 {
            let sx = x - dx;
            if sx >= 0 && sx < w as i32 {
                out[(y as usize) * w + x as usize] = mask[sy as usize * w + sx as usize];
            }
        }
    }
    out
}

/// Three box blurs ≈ a Gaussian blur.
fn blurred(mask: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    if r == 0 {
        return mask.to_vec();
    }
    let mut a: Vec<u32> = mask.iter().map(|&v| v as u32).collect();
    let mut b = vec![0u32; w * h];
    for _ in 0..3 {
        for y in 0..h {
            let row = &a[y * w..(y + 1) * w];
            let mut sum: u32 = row[..r.min(w)].iter().sum();
            for x in 0..w {
                if x + r < w {
                    sum += row[x + r];
                }
                if x > r {
                    sum -= row[x - r - 1];
                }
                b[y * w + x] = sum / (2 * r as u32 + 1);
            }
        }
        for x in 0..w {
            let mut sum: u32 = (0..r.min(h)).map(|y| b[y * w + x]).sum();
            for y in 0..h {
                if y + r < h {
                    sum += b[(y + r) * w + x];
                }
                if y > r {
                    sum -= b[(y - r - 1) * w + x];
                }
                a[y * w + x] = sum / (2 * r as u32 + 1);
            }
        }
    }
    a.into_iter().map(|v| v.min(255) as u8).collect()
}

fn dilated(mask: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let mut tmp = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let lo = x.saturating_sub(r);
            let hi = (x + r).min(w - 1);
            tmp[y * w + x] = *mask[y * w + lo..=y * w + hi].iter().max().unwrap();
        }
    }
    let mut out = vec![0u8; w * h];
    for x in 0..w {
        for y in 0..h {
            let lo = y.saturating_sub(r);
            let hi = (y + r).min(h - 1);
            out[y * w + x] = (lo..=hi).map(|yy| tmp[yy * w + x]).max().unwrap();
        }
    }
    out
}

/// tiny-skia keeps premultiplied RGBA; NDI and the pump want straight BGRA.
fn to_bgra_straight(px: &Pixmap) -> Vec<u8> {
    let mut out = px.data().to_vec();
    for p in out.chunks_exact_mut(4) {
        let a = p[3] as u32;
        let (r, g, b) = if a == 0 { (0, 0, 0) } else { ((p[0] as u32 * 255 + a / 2) / a, (p[1] as u32 * 255 + a / 2) / a, (p[2] as u32 * 255 + a / 2) / a) };
        p[0] = b.min(255) as u8;
        p[1] = g.min(255) as u8;
        p[2] = r.min(255) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower_third() -> Value {
        serde_json::from_str(include_str!("../../wp-core/themes/lower-third.json")).unwrap()
    }

    fn save_png(path: &str, bgra: &[u8], w: u32, h: u32) {
        let mut rgba = bgra.to_vec();
        for p in rgba.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        image::save_buffer(path, &rgba, w, h, image::ColorType::Rgba8).unwrap();
    }

    #[test]
    fn gradients_and_frosted_glass() {
        let mut r = Renderer::new();
        let theme = serde_json::json!({ "background": { "kind": "gradient", "gradient": { "from": "#ff0000", "to": "#0000ff", "angle": 90 } },
            "layers": [{ "kind": "text", "field": "lyrics", "x": 10, "y": 40, "w": 80, "h": 20,
                "style": { "font": "Noto Sans", "size": 8, "weight": 800, "color": "#ffffff", "align": "center", "valign": "middle", "autoFit": false,
                    "gradient": { "from": "#ffffff", "to": "#ffd000", "angle": 180 },
                    "box": { "mode": "block", "color": "#ffffff40", "padX": 2, "padY": 1, "radius": 0, "blur": 1.5, "animate": true } } }] });
        let f = r.render(&theme, &Content { primary: vec!["Blessed assurance".into()], ..Content::default() }, 960, 540, Path::new("/x"));
        // a gradient box with round corners: the very corner stays clear
        let boxy = serde_json::json!({ "background": { "kind": "transparent" }, "layers": [{ "kind": "shape", "x": 10, "y": 10, "w": 50, "h": 50, "radius": 8, "fill": "#000000", "gradient": { "from": "#ff0000", "to": "#0000ff", "angle": 90 }, "border": { "color": "#0000", "width": 0 } }] });
        let g = r.render(&boxy, &Content::default(), 960, 540, Path::new("/x"));
        assert_eq!(g[(54 * 960 + 96) * 4 + 3], 0, "rounded corner");
        assert_eq!(g[(200 * 960 + 300) * 4 + 3], 255, "inside");
        save_png(&std::env::temp_dir().join("render-gradient.png").to_string_lossy(), &f, 960, 540);
        let px = |x: usize, y: usize| (f[(y * 960 + x) * 4 + 2], f[(y * 960 + x) * 4], f[(y * 960 + x) * 4 + 3]); // r, b, a
        let (lr, lb, _) = px(5, 10);
        let (rr, rb, _) = px(954, 10);
        assert!(lr > 200 && lb < 60 && rb > 200 && rr < 60, "red on the left, blue on the right: {:?} {:?}", (lr, lb), (rr, rb));
        // gradient words: white at the top of the layer turning yellow (less blue) lower down
        let blue_in_words = |y: usize| -> Option<f32> {
            let v: Vec<f32> = (0..960).filter_map(|x| { let p = &f[(y * 960 + x) * 4..]; (p[2] > 230 && p[1] > 200 && p[3] > 250).then_some(p[0] as f32) }).collect();
            (v.len() > 3).then(|| v.iter().sum::<f32>() / v.len() as f32)
        };
        let rows: Vec<(usize, f32)> = (200..340).filter_map(|y| blue_in_words(y).map(|b| (y, b))).collect();
        let (top, bottom) = (rows.first().unwrap().1, rows.last().unwrap().1);
        assert!(top > bottom + 25.0, "letters less blue lower down: top {top}, bottom {bottom}");
    }

    #[test]
    fn brand_references_resolve() {
        let b = Brand::from_settings(Some(r##"["#112233","#ffcc00","#ffffff","#000000"]"##), Some("Ubuntu"), None);
        let t = serde_json::json!({ "background": { "kind": "color", "color": "brand:4" }, "layers": [{ "kind": "shape", "fill": "brand:1/80" }, { "kind": "text", "style": { "font": "brand-heading", "color": "brand:2" } }] });
        let r = b.resolve(&t);
        assert_eq!(r["background"]["color"], "#000000");
        assert_eq!(r["layers"][0]["fill"], "#11223380");
        assert_eq!(r["layers"][1]["style"]["font"], "Ubuntu", "the heading font falls back to the text font");
        assert_eq!(r["layers"][1]["style"]["color"], "#ffcc00");
        assert_ne!(b.key(), Brand::from_settings(None, None, None).key());
    }

    #[test]
    fn colours() {
        assert_eq!(parse_color("#fff").unwrap().to_color_u8().red(), 255);
        assert_eq!(parse_color("#0c0d10db").unwrap().to_color_u8().alpha(), 219);
        assert!(parse_color("red").is_none());
    }

    #[test]
    fn lower_third_with_akan_and_exact_alpha() {
        let mut r = Renderer::new();
        let c = Content {
            primary: vec!["O for a thousand tongues to sing".into(), "My great Redeemer’s praise,".into()],
            second: vec!["Meenya ngyiramá apem matow ndwom".into(), "Meyi mo Pomfò ayɛw,".into()],
            ..Content::default()
        };
        let t = std::time::Instant::now();
        let f = r.render(&lower_third(), &c, 1920, 1080, Path::new("/nonexistent"));
        println!("1080p lower third rendered in {:?}", t.elapsed());
        save_png(&std::env::temp_dir().join("render-lt.png").to_string_lossy(), &f, 1920, 1080);
        let alpha = |x: usize, y: usize| f[(y * 1920 + x) * 4 + 3];
        assert_eq!(alpha(960, 100), 0, "top of the frame is see-through");
        assert!(alpha(30, 1000) > 150, "the band is (mostly) opaque");
        let text_px = f.chunks_exact(4).filter(|p| p[3] > 200 && p[0] > 200 && p[1] > 200 && p[2] > 200).count();
        assert!(text_px > 5000, "white words drawn: {text_px}");
        // blank-style content: nothing at all
        let empty = r.render(&serde_json::json!({ "background": { "kind": "transparent" }, "layers": [] }), &Content::default(), 64, 36, Path::new("/x"));
        assert!(empty.chunks_exact(4).all(|p| p[3] == 0));
    }

    #[test]
    fn box_grows_with_the_words() {
        let mut r = Renderer::new();
        let theme = |mode: &str| {
            serde_json::json!({ "background": { "kind": "transparent" }, "layers": [{ "kind": "text", "field": "lyrics", "x": 5, "y": 60, "w": 90, "h": 30,
                "style": { "font": "Noto Sans", "size": 6, "weight": 700, "color": "#ffffff", "align": "center", "valign": "middle", "lineHeight": 1.2, "autoFit": true, "minSize": 3,
                    "box": { "mode": mode, "color": "#000000ff", "padX": 2, "padY": 1, "radius": 0, "animate": true } } }] })
        };
        let width_of = |f: &[u8]| {
            let row = 810; // a row through the text
            let xs: Vec<usize> = (0..1920).filter(|x| f[(row * 1920 + x) * 4 + 3] > 200).collect();
            xs.last().map(|l| l - xs[0]).unwrap_or(0)
        };
        let short = r.render(&theme("block"), &Content { primary: vec!["The Lord".into()], ..Content::default() }, 1920, 1080, Path::new("/x"));
        let long = r.render(&theme("block"), &Content { primary: vec!["The Lord is my shepherd, I shall not want".into()], ..Content::default() }, 1920, 1080, Path::new("/x"));
        let (ws, wl) = (width_of(&short), width_of(&long));
        println!("box width: short {ws}px, long {wl}px");
        assert!(ws > 200 && wl > ws + 600, "the box follows the words ({ws} vs {wl})");
        assert_eq!(short[(810 * 1920 + 100) * 4 + 3], 0, "outside the box stays clear");
        save_png(&std::env::temp_dir().join("render-box.png").to_string_lossy(), &long, 1920, 1080);
        let lines = r.render(&theme("lines"), &Content { primary: vec!["The Lord is my shepherd".into(), "I shall not want".into()], ..Content::default() }, 1920, 1080, Path::new("/x"));
        save_png(&std::env::temp_dir().join("render-lines.png").to_string_lossy(), &lines, 1920, 1080);
        assert!(lines.chunks_exact(4).filter(|p| p[3] == 255 && p[0] < 10).count() > 20_000, "strips drawn behind the lines");
    }

    #[test]
    fn subtitles_and_alert_bands() {
        let mut r = Renderer::new();
        let c = Content { subtitle: Some("so turn with me to John three sixteen".into()), alert: Some("Car GT-1234-20 is blocking the gate".into()), ..Content::default() };
        let f = r.render(&serde_json::json!({ "background": { "kind": "transparent" }, "layers": [] }), &c, 1920, 1080, Path::new("/x"));
        save_png(&std::env::temp_dir().join("render-bands.png").to_string_lossy(), &f, 1920, 1080);
        assert!(f[(40 * 1920 + 960) * 4 + 3] > 150, "alert band at the top");
        assert!(f[(1000 * 1920 + 960) * 4 + 3] > 120, "subtitle band near the bottom");
    }
}
