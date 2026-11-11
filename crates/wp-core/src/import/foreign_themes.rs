//! Themes from EasyWorship (.ewtx) and ProPresenter 7 (.proTheme / Theme files).
//!
//! EasyWorship exports a theme as a zip holding `main.db` (its presentation database:
//! elements with positions, text styles, background colour/picture) and `media\…`.
//! ProPresenter 7 exports a zip of themes, each a protobuf `Theme` file (its format is
//! not published; the fields used here were read from real files) with `Assets/…`.
//!
//! Each design becomes this app's own theme package (.wptheme JSON plus its pictures),
//! so `themes::import` stores it exactly like a theme exported from Worship Presenter.
//! `review` returns every design with a preview (pictures embedded as data: URLs) so
//! the operator chooses which to bring in.

use crate::{Error, Result};
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Read;

fn bad(msg: impl Into<String>) -> Error {
    Error::Format("Theme".into(), "theme", msg.into())
}

#[derive(Debug, Clone, Serialize)]
pub struct Design {
    /// "Golden Ticket – Main Title"
    pub name: String,
    /// song | scripture | lower_third | announcement
    pub kind: String,
    /// The app's theme JSON with pictures as data: URLs, for a preview.
    pub preview: Value,
    /// What could not be carried over exactly.
    pub notes: Vec<String>,
    /// Ticked by default in the review (songs, scripture, lower thirds).
    pub suggested: bool,
    #[serde(skip)]
    pub package: Vec<u8>,
}

// ---------- shared helpers ----------

fn hex_rgba(r: f64, g: f64, b: f64, a: f64) -> String {
    let c = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if a >= 0.999 {
        format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c(r), c(g), c(b), c(a))
    }
}

fn anim() -> Value {
    json!({ "kind": "fade", "ms": 300, "delay": 0 })
}

#[allow(clippy::too_many_arguments)]
fn text_layer(id: &str, name: &str, field: &str, rect: [f64; 4], style: Value, text: &str, hidden: bool) -> Value {
    json!({
        "id": id, "name": name, "kind": "text", "field": field, "text": text,
        "x": r1(rect[0]), "y": r1(rect[1]), "w": r1(rect[2]), "h": r1(rect[3]), "solo": null,
        "opacity": 1, "hidden": hidden, "locked": false, "style": style, "animIn": anim(), "animOut": json!({ "kind": "fade", "ms": 200, "delay": 0 }),
    })
}

fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[allow(clippy::too_many_arguments)]
fn style(font: &str, size: f64, weight: u32, italic: bool, color: &str, align: &str, valign: &str, autofit: bool, shadow: bool, outline: Option<(String, f64)>) -> Value {
    json!({
        "font": font, "size": r1(size.max(0.5)), "weight": weight, "italic": italic, "color": color,
        "align": align, "valign": valign, "lineHeight": 1.15, "letterSpacing": 0, "uppercase": false,
        "autoFit": autofit, "minSize": r1((size * 0.45).max(1.5)),
        "shadow": { "on": shadow, "color": "#000000cc", "blur": 0.6, "x": 0, "y": 0.25 },
        "outline": match outline { Some((c, w)) => json!({ "on": true, "color": c, "width": r1(w.max(0.05)) }), None => json!({ "on": false, "color": "#000000", "width": 0.15 }) },
    })
}

fn package(theme: &Value, files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let f: serde_json::Map<String, Value> = files.iter().map(|(k, v)| (k.clone(), Value::String(b64.encode(v)))).collect();
    serde_json::to_vec(&json!({ "format": crate::themes::FORMAT, "version": 1, "theme": theme, "files": f })).unwrap_or_default()
}

/// The same theme with "media:path" pictures turned into data: URLs (preview only).
fn with_inline_media(theme: &Value, files: &BTreeMap<String, Vec<u8>>) -> Value {
    let mut s = theme.to_string();
    for (path, bytes) in files {
        let mime = if path.ends_with(".png") { "image/png" } else if path.ends_with(".gif") { "image/gif" } else { "image/jpeg" };
        // previews stay small: big pictures are left out of the preview only
        if bytes.len() < 3_000_000 {
            let url = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes));
            s = s.replace(&format!("\"media:{path}\""), &format!("\"{url}\""));
        }
    }
    serde_json::from_str(&s).unwrap_or_else(|_| theme.clone())
}

fn slug(s: &str) -> String {
    let v: String = s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    v.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

fn zip_files(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| bad("This theme file is not a zip archive, or it is damaged."))?;
    let mut out = vec![];
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|_| bad("A file inside the theme is damaged."))?;
        if f.is_dir() || f.size() == 0 {
            continue;
        }
        // EasyWorship writes Windows backslashes inside its zips
        let name = String::from_utf8_lossy(f.name_raw()).replace('\\', "/");
        if f.size() > 200_000_000 {
            continue;
        }
        let mut b = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut b).map_err(|_| bad("A file inside the theme is damaged."))?;
        out.push((name, b));
    }
    Ok(out)
}

/// Weight and italic from a font name like "NexaBold", "Helvetica-BoldOblique".
fn weight_of(name: &str) -> (u32, bool) {
    let n = name.to_lowercase();
    let w = if n.contains("black") || n.contains("heavy") { 900 } else if n.contains("extrabold") || n.contains("ultrabold") { 800 } else if n.contains("semibold") || n.contains("demibold") { 600 } else if n.contains("bold") { 700 } else if n.contains("medium") { 500 } else if n.contains("light") || n.contains("thin") { 300 } else { 400 };
    (w, n.contains("italic") || n.contains("oblique"))
}

// ---------- EasyWorship (.ewtx) ----------

/// EasyWorship colours are Windows BGR numbers (65280 = green).
fn ew_color(v: i64) -> String {
    let v = v as u32;
    format!("#{:02x}{:02x}{:02x}", v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff)
}

pub fn read_ewtx(bytes: &[u8]) -> Result<Vec<Design>> {
    let files = zip_files(bytes)?;
    let db = files.iter().find(|(n, _)| n.eq_ignore_ascii_case("main.db")).ok_or_else(|| bad("This is not an EasyWorship theme export (no main.db inside)."))?;
    let tmp = std::env::temp_dir().join(format!("wp-ewtx-{}-{}.db", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    std::fs::write(&tmp, &db.1)?;
    let out = (|| -> Result<Vec<Design>> {
        let c = rusqlite::Connection::open_with_flags(&tmp, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let (pid, title): (i64, String) = c
            .query_row("SELECT rowid, coalesce(title, 'EasyWorship theme') FROM presentation WHERE presentation_uid <> 'GLOBAL' ORDER BY rowid LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| bad("The EasyWorship theme has no design inside."))?;
        let (sid, kind_uid): (i64, Option<String>) = c.query_row("SELECT rowid, theme_slide_uid FROM slide WHERE presentation_id = ?1 ORDER BY order_index LIMIT 1", [pid], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let kind = match kind_uid.as_deref() {
            Some("SCRIPTURE") => "scripture",
            Some("PRESENTATION") => "announcement",
            _ => "song",
        };
        // properties: element rowid → group → key → value
        let mut props: BTreeMap<(i64, String, String), String> = BTreeMap::new();
        let mut st = c.prepare("SELECT g.link_id, g.group_name, p.key, p.value FROM element_property p JOIN element_property_group g ON g.rowid = p.group_id")?;
        for r in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?)))? {
            let (e, g, k, v) = r?;
            props.insert((e, g, k), v.unwrap_or_default());
        }
        let prop = |e: i64, g: &str, k: &str| props.get(&(e, g.to_string(), k.to_string())).cloned();
        let mut st = c.prepare("SELECT rowid, element_uid, x, y, width, height, background_resource_id, foreground_resource_id FROM element WHERE slide_id = ?1 ORDER BY order_index")?;
        #[allow(clippy::type_complexity)]
        let els: Vec<(i64, String, f64, f64, f64, f64, Option<i64>, Option<i64>)> = st
            .query_map([sid], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, Option<f64>>(2)?.unwrap_or(0.0), r.get::<_, Option<f64>>(3)?.unwrap_or(0.0), r.get::<_, Option<f64>>(4)?.unwrap_or(1.0), r.get::<_, Option<f64>>(5)?.unwrap_or(0.0), r.get(6)?, r.get(7)?)))?
            .collect::<std::result::Result<_, _>>()?;
        let mut media: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut notes = vec![];
        let mut bg = json!({ "kind": "color", "color": "#000000", "image": null, "fit": "cover", "dim": 0 });
        let mut layers: Vec<Value> = vec![];
        for (eid, uid, x, y, w, h, bg_res, fg_res) in &els {
            match uid.as_str() {
                "BACKGROUND" => {
                    if let Some(r) = bg_res {
                        if let Ok(col) = c.query_row("SELECT color FROM resource_color WHERE resource_id = ?1", [r], |r| r.get::<_, i64>(0)) {
                            bg["color"] = json!(ew_color(col));
                        }
                    }
                    if let Some(r) = fg_res {
                        let pic: Option<(String, String)> = c
                            .query_row("SELECT f.filename, coalesce(i.original_filename, f.filename) FROM resource_image i JOIN file f ON f.rowid = i.file_id WHERE i.resource_id = ?1", [r], |r| Ok((r.get(0)?, r.get(1)?)))
                            .ok();
                        if let Some((file, original)) = pic {
                            match files.iter().find(|(n, _)| n.rsplit('/').next() == Some(file.as_str())) {
                                Some((_, data)) => {
                                    let stem = original.rsplit(['>', '/', '\\']).next().unwrap_or(&file).to_string();
                                    let rel = format!("images/{}-{}", slug(&title), slug(&stem).replace("-jpg", ".jpg").replace("-png", ".png").replace("-jpeg", ".jpeg"));
                                    let rel = if rel.contains('.') { rel } else { format!("{rel}.jpg") };
                                    media.insert(rel.clone(), data.clone());
                                    bg["kind"] = json!("image");
                                    bg["image"] = json!(format!("media:{rel}"));
                                    // the picture's opacity over the colour reads as dimming
                                    let op: f64 = prop(*eid, "Element", "Opacity").and_then(|v| v.parse().ok()).unwrap_or(100.0);
                                    bg["dim"] = json!(r1(((100.0 - op) / 100.0).clamp(0.0, 0.9) * 10.0) / 10.0);
                                }
                                None => notes.push("The background picture is not inside the file; choose one in the theme editor.".into()),
                            }
                        }
                    }
                    let video: Option<i64> = fg_res.and_then(|r| c.query_row("SELECT rowid FROM resource_video WHERE resource_id = ?1", [r], |r| r.get(0)).ok());
                    if video.is_some() {
                        notes.push("Video backgrounds from EasyWorship are not copied; choose the video in the theme editor.".into());
                    }
                }
                "CONTENT_SONG" | "CONTENT_SCRIPTURE" | "CONTENT" | "CONTENT_PRESENTATION" => {
                    let hidden = prop(*eid, "Element", "Hidden").as_deref() == Some("1");
                    let wanted = match kind {
                        "scripture" => uid == "CONTENT_SCRIPTURE" || uid == "CONTENT",
                        "announcement" => uid == "CONTENT_PRESENTATION" || uid == "CONTENT",
                        _ => uid == "CONTENT_SONG" || uid == "CONTENT",
                    };
                    if hidden || !wanted || layers.iter().any(|l| l["field"] == "lyrics") {
                        continue;
                    }
                    let (main, label, refr) = if uid == "CONTENT_SCRIPTURE" { ("Text_Style_201", "", "Text_Style_203") } else { ("Text_Style_101", "Text_Style_102", "") };
                    let get = |g: &str, k: &str| prop(*eid, g, k);
                    let size = |g: &str| get(g, "Font_Size").and_then(|v| v.parse::<f64>().ok()).map(|f| f * 100.0);
                    let font = get(main, "Font_Name").unwrap_or_else(|| "Tahoma".into());
                    let (wt, it) = weight_of(&font);
                    let bold = get(main, "Font_Bold").as_deref() == Some("1");
                    let color = get(main, "Font_Color").and_then(|v| v.parse::<i64>().ok()).map(ew_color).unwrap_or_else(|| "#ffffff".into());
                    let sz = size(main).unwrap_or(8.5);
                    // EasyWorship keeps text away from the edges of its box
                    let m = 3.0;
                    let rect = [x * 100.0 + m, y * 100.0 + m, w * 100.0 - 2.0 * m, (if *h > 0.0 { *h } else { 1.0 }) * 100.0 - 2.0 * m];
                    let mut lyr_rect = rect;
                    // the verse label ("verse 1") style, when shown
                    if !label.is_empty() && get(label, "Hidden").as_deref() != Some("1") && (get(label, "Font_Color").is_some() || get(label, "Font_Name").is_some()) {
                        let lf = get(label, "Font_Name").unwrap_or_else(|| font.clone());
                        let (lw, li) = weight_of(&lf);
                        let lc = get(label, "Font_Color").and_then(|v| v.parse::<i64>().ok()).map(ew_color).unwrap_or_else(|| color.clone());
                        let lsz = (size(label).unwrap_or(sz) * 0.5).max(2.5);
                        layers.push(text_layer("section", "Section label", "section", [rect[0], rect[1], rect[2], lsz * 1.5], style(&lf, lsz, lw.max(600), li, &lc, "center", "top", false, true, None), "", false));
                        lyr_rect = [rect[0], rect[1] + lsz * 1.6, rect[2], rect[3] - lsz * 1.6];
                    }
                    layers.push(text_layer("lyrics", if kind == "scripture" { "Verse text" } else { "Lyrics" }, "lyrics", lyr_rect, style(&font, sz, if bold { wt.max(700) } else { wt }, it, &color, "center", "middle", true, true, None), "", false));
                    if !refr.is_empty() && get(refr, "Hidden").as_deref() != Some("1") {
                        layers.push(text_layer("reference", "Reference", "reference", [rect[0], rect[1] + rect[3] - 8.0, rect[2], 8.0], style(&font, (sz * 0.5).max(3.0), wt, false, &color, "right", "bottom", false, true, None), "", false));
                    }
                }
                "COPYRIGHT" => {
                    if prop(*eid, "Element", "Hidden").as_deref() != Some("1") {
                        layers.push(text_layer("copyright", "Copyright", "copyright", [3.0, 93.0, 94.0, 5.0], style("Noto Sans", 2.2, 400, false, "#ffffffcc", "center", "bottom", false, true, None), "", false));
                    }
                }
                _ => {}
            }
        }
        if !layers.iter().any(|l| l["field"] == "lyrics") {
            layers.push(text_layer("lyrics", "Lyrics", "lyrics", [5.0, 7.0, 90.0, 86.0], style("Noto Sans", 8.5, 600, false, "#ffffff", "center", "middle", true, true, None), "", false));
            notes.push("No text box was found; a standard one was added.".into());
        }
        notes.push("EasyWorship's text effects beyond font, size, colour and shadow (and its transitions) are not copied.".into());
        let theme = json!({ "schema_version": 1, "name": title, "kind": kind, "background": bg, "transition": { "kind": "fade", "ms": 300 }, "layers": layers });
        Ok(vec![Design { name: title.clone(), kind: kind.into(), preview: with_inline_media(&theme, &media), notes, suggested: true, package: package(&theme, &media) }])
    })();
    let _ = std::fs::remove_file(&tmp);
    out
}

// ---------- ProPresenter 7 (protobuf) ----------

#[derive(Debug, Clone)]
enum Pb {
    V(u64),
    N(f64),
    B(Vec<u8>),
}

type Msg = Vec<(u32, Pb)>;

fn pb_parse(b: &[u8]) -> Option<Msg> {
    let mut i = 0;
    let mut out = vec![];
    let varint = |i: &mut usize| -> Option<u64> {
        let mut v = 0u64;
        for s in (0..64).step_by(7) {
            let x = *b.get(*i)?;
            *i += 1;
            v |= ((x & 0x7f) as u64) << s;
            if x < 0x80 {
                return Some(v);
            }
        }
        None
    };
    while i < b.len() {
        let key = varint(&mut i)?;
        let (f, wt) = ((key >> 3) as u32, key & 7);
        if f == 0 {
            return None;
        }
        match wt {
            0 => out.push((f, Pb::V(varint(&mut i)?))),
            1 => {
                let v = f64::from_le_bytes(b.get(i..i + 8)?.try_into().ok()?);
                i += 8;
                out.push((f, Pb::N(v)));
            }
            5 => {
                let v = f32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?);
                i += 4;
                out.push((f, Pb::N(v as f64)));
            }
            2 => {
                let n = varint(&mut i)? as usize;
                out.push((f, Pb::B(b.get(i..i + n)?.to_vec())));
                i += n;
            }
            _ => return None,
        }
    }
    Some(out)
}

fn sub(m: &Msg, f: u32) -> Option<Msg> {
    m.iter().find(|(k, _)| *k == f).and_then(|(_, v)| if let Pb::B(b) = v { pb_parse(b) } else { None })
}
fn subs(m: &Msg, f: u32) -> Vec<Msg> {
    m.iter().filter(|(k, _)| *k == f).filter_map(|(_, v)| if let Pb::B(b) = v { pb_parse(b) } else { None }).collect()
}
fn num(m: &Msg, f: u32) -> f64 {
    match m.iter().find(|(k, _)| *k == f).map(|(_, v)| v) {
        Some(Pb::N(n)) => *n,
        Some(Pb::V(v)) => *v as f64,
        _ => 0.0,
    }
}
fn flag(m: &Msg, f: u32) -> bool {
    matches!(m.iter().find(|(k, _)| *k == f), Some((_, Pb::V(v))) if *v != 0)
}
fn text(m: &Msg, f: u32) -> String {
    match m.iter().find(|(k, _)| *k == f) {
        Some((_, Pb::B(b))) => String::from_utf8_lossy(b).into_owned(),
        _ => String::new(),
    }
}
fn color(m: Option<Msg>) -> Option<String> {
    let c = m?;
    Some(hex_rgba(num(&c, 1), num(&c, 2), num(&c, 3), if c.iter().any(|(k, _)| *k == 4) { num(&c, 4) } else { 0.0 }))
}

fn classify(slide: &str, element_names: &[String]) -> Option<&'static str> {
    let s = slide.to_lowercase();
    let has = |n: &str| element_names.iter().any(|e| e.to_lowercase().contains(n));
    if s.contains("countdown") || s.contains("timer") || has("timer") {
        return None;
    }
    if s.contains("lower third") || s.contains("lower-third") || s.contains("lower 3rd") || s.contains("lower3rd") || s == "l3" || s.starts_with("l3 ") || has("l3") {
        return Some("lower_third");
    }
    if s.contains("scripture") || s.contains("bible") || (has("reference") && has("verse")) || has("scripture") {
        return Some(if s.contains("lower") { "lower_third" } else { "scripture" });
    }
    if s.contains("lyric") || s.contains("song") || s == "theme slide" || s.contains("two lines") || s.contains("four lines") || s.contains("lines only") || has("lyrics") || s.is_empty() {
        return Some("song");
    }
    Some("announcement")
}

/// ProPresenter's "Theme" protobuf (one of the themes inside a .proTheme), with its
/// Assets/ files, into one design per slide.
pub fn read_pro_theme_file(theme_name: &str, bytes: &[u8], assets: &BTreeMap<String, Vec<u8>>) -> Vec<Design> {
    let Some(root) = pb_parse(bytes) else { return vec![] };
    let mut out = vec![];
    let slides = subs(&root, 3);
    let single = slides.len() == 1;
    for (si, entry) in slides.iter().enumerate() {
        let slide_name = text(entry, 2);
        let Some(slide) = sub(entry, 1) else { continue };
        let size = sub(&slide, 6);
        let (sw, sh) = size.map(|s| (num(&s, 1), num(&s, 2))).filter(|(w, h)| *w > 0.0 && *h > 0.0).unwrap_or((1920.0, 1080.0));
        let wrappers = subs(&slide, 1);
        let elements: Vec<Msg> = wrappers.iter().filter_map(|w| sub(w, 1)).collect();
        let names: Vec<String> = elements.iter().map(|e| text(e, 2)).collect();
        let generic = slide_name.is_empty() || slide_name.eq_ignore_ascii_case("theme slide");
        let Some(kind) = classify(if generic { theme_name } else { &slide_name }, &names).or_else(|| (!generic).then_some(()).and(None)) else { continue };
        let mut notes = vec![];
        let mut media: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let draws_bg = flag(&slide, 4);
        let bg_color = color(sub(&slide, 5)).unwrap_or_else(|| "#000000".into());
        let bg = if !draws_bg {
            // ProPresenter only paints the slide colour when "draw background" is on
            if kind == "lower_third" {
                json!({ "kind": "transparent", "color": "#000000", "image": null, "fit": "cover", "dim": 0 })
            } else {
                json!({ "kind": "color", "color": "#000000", "image": null, "fit": "cover", "dim": 0 })
            }
        } else {
            json!({ "kind": "color", "color": if bg_color.len() == 9 && bg_color.ends_with("00") { "#000000".to_string() } else { bg_color }, "image": null, "fit": "cover", "dim": 0 })
        };
        // text boxes: largest gets the main words
        let mut texts: Vec<(usize, f64)> = vec![];
        for (i, e) in elements.iter().enumerate() {
            let t = sub(e, 13);
            let plain = t.as_ref().map(|t| crate::import::songfiles::rtf_text(&text(t, 5)).join(" ")).unwrap_or_default();
            if !plain.trim().is_empty() {
                let b = sub(e, 3).and_then(|b| sub(&b, 2)).map(|s| num(&s, 1) * num(&s, 2)).unwrap_or(0.0);
                texts.push((i, b));
            }
        }
        let mut by_size = texts.clone();
        by_size.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let is_side = |j: usize| {
            let n = names[j].to_lowercase();
            n.contains("reference") || n == "title" || n.contains("main title") || n.contains("author") || n.contains("verse 2") || n.contains("timer")
        };
        let main_idx = by_size.iter().find(|(j, _)| !is_side(*j)).or(by_size.first()).map(|(j, _)| *j);
        let mut main_used = false;
        let mut layers: Vec<Value> = vec![];
        // ProPresenter lists the front element first; this app draws later layers on top
        for (i, e) in elements.iter().enumerate().rev() {
            let name = text(e, 2);
            let lname = name.to_lowercase();
            let bounds = sub(e, 3);
            let (ox, oy) = bounds.as_ref().and_then(|b| sub(b, 1)).map(|o| (num(&o, 1), num(&o, 2))).unwrap_or((0.0, 0.0));
            let (w, h) = bounds.as_ref().and_then(|b| sub(b, 2)).map(|s| (num(&s, 1), num(&s, 2))).unwrap_or((sw, sh));
            let rect = [ox / sw * 100.0, oy / sh * 100.0, w / sw * 100.0, h / sh * 100.0];
            let id = format!("pp{i}");
            let hidden = flag(e, 15);
            // fill: picture or colour (switched on by field 4)
            if let Some(fill) = sub(e, 9) {
                if flag(&fill, 4) {
                    let pic = sub(&fill, 3).and_then(|m| sub(&m, 2)).and_then(|u| sub(&u, 4)).map(|p| text(&p, 2));
                    match pic {
                        Some(rel) if !rel.is_empty() => {
                            let file = rel.rsplit('/').next().unwrap_or(&rel).to_string();
                            match assets.get(&file) {
                                Some(data) => {
                                    let path = format!("images/{}-{}", slug(theme_name), file.replace(' ', "-"));
                                    media.insert(path.clone(), data.clone());
                                    layers.push(json!({ "id": id, "name": if name.is_empty() { "Picture".into() } else { name.clone() }, "kind": "image", "src": format!("media:{path}"), "fit": "cover",
                                        "x": r1(rect[0]), "y": r1(rect[1]), "w": r1(rect[2]), "h": r1(rect[3]), "opacity": r1(if e.iter().any(|(k, _)| *k == 5) { num(e, 5) } else { 1.0 }), "hidden": hidden, "locked": false, "animIn": anim(), "animOut": anim() }));
                                }
                                None => notes.push(format!("The picture for “{name}” is not in the file.")),
                            }
                        }
                        _ => {
                            let stroke = sub(e, 10).filter(|s| flag(s, 5));
                            layers.push(json!({ "id": format!("{id}-fill"), "name": if name.is_empty() { "Shape".into() } else { format!("{name} box") }, "kind": "shape", "shape": "rect",
                                "fill": color(sub(&fill, 1)).unwrap_or_else(|| "#000000".into()), "radius": 0,
                                "border": { "color": stroke.as_ref().and_then(|s| color(sub(s, 3))).unwrap_or_else(|| "#ffffff00".into()), "width": stroke.as_ref().map(|s| r1(num(s, 2) / sh * 100.0)).unwrap_or(0.0) },
                                "x": r1(rect[0]), "y": r1(rect[1]), "w": r1(rect[2]), "h": r1(rect[3]), "opacity": 1, "hidden": hidden, "locked": false, "animIn": anim(), "animOut": anim() }));
                        }
                    }
                } else if let Some(stroke) = sub(e, 10).filter(|s| flag(s, 5)) {
                    // an outline-only box
                    layers.push(json!({ "id": format!("{id}-line"), "name": format!("{name} outline"), "kind": "shape", "shape": "rect", "fill": "#00000000", "radius": 0,
                        "border": { "color": color(sub(&stroke, 3)).unwrap_or_else(|| "#ffffff".into()), "width": r1(num(&stroke, 2) / sh * 100.0) },
                        "x": r1(rect[0]), "y": r1(rect[1]), "w": r1(rect[2]), "h": r1(rect[3]), "opacity": 1, "hidden": hidden, "locked": false, "animIn": anim(), "animOut": anim() }));
                }
            }
            // text
            if !texts.iter().any(|(j, _)| *j == i) {
                continue;
            }
            let Some(t) = sub(e, 13) else { continue };
            let attrs = sub(&t, 3);
            let font = attrs.as_ref().and_then(|a| sub(a, 1));
            let ps = font.as_ref().map(|f| text(f, 1)).unwrap_or_default();
            let family = font.as_ref().map(|f| text(f, 9)).filter(|s| !s.is_empty()).unwrap_or_else(|| ps.clone());
            let pt = font.as_ref().map(|f| num(f, 2)).filter(|v| *v > 0.0).unwrap_or(60.0);
            let (wt, it) = weight_of(&ps);
            let col = attrs.as_ref().and_then(|a| color(sub(a, 3))).unwrap_or_else(|| "#ffffff".into());
            let para = attrs.as_ref().and_then(|a| sub(a, 6));
            let align = match para.as_ref().map(|p| num(p, 1) as i64).unwrap_or(0) {
                1 => "right",
                2 => "center",
                _ => "left",
            };
            let valign = match num(&t, 6) as i64 {
                1 => "middle",
                2 => "bottom",
                _ => "top",
            };
            let outline = sub(&t, 12).filter(|s| flag(s, 1) || flag(s, 5)).map(|s| (color(sub(&s, 3)).unwrap_or_else(|| "#000000".into()), num(&s, 2) / sh * 100.0));
            let shadow = sub(&t, 4).map(|s| flag(&s, 1) || flag(&s, 7)).unwrap_or(false);
            let field = if Some(i) == main_idx {
                "lyrics"
            } else if lname.contains("reference") {
                if lname.contains('2') { "free" } else { "reference" }
            } else if lname.contains("verse 2") || lname.contains("second") {
                "second"
            } else if lname == "title" || lname.contains("main title") {
                if kind == "announcement" { "title" } else { "free" }
            } else {
                "free"
            };
            let field = if field == "lyrics" && main_used { "free" } else { field };
            if field == "lyrics" {
                main_used = true;
            }
            let placeholder = crate::import::songfiles::rtf_text(&text(&t, 5)).join("\n");
            let st = style(&family, pt / sh * 100.0, wt, it, &col, align, valign, field == "lyrics", shadow, outline);
            // extra text boxes keep their wording as fixed text, hidden until wanted
            let (txt, hide) = if field == "free" { (placeholder.as_str(), true) } else { ("", hidden) };
            layers.push(text_layer(&format!("{id}-text"), if name.is_empty() { "Text" } else { &name }, field, rect, st, txt, hide));
        }
        if !main_used {
            notes.push("No text box held the main words; a standard one was added.".into());
            layers.push(text_layer("lyrics", "Lyrics", "lyrics", [5.0, 7.0, 90.0, 86.0], style("Noto Sans", 8.5, 600, false, "#ffffff", "center", "middle", true, true, None), "", false));
        }
        if layers.iter().any(|l| l["kind"] == "text" && l["hidden"] == true && l["field"] == "free") {
            notes.push("Extra text boxes were kept as hidden fixed text; show them in the theme editor if wanted.".into());
        }
        notes.push("Masks, gradients, custom shapes, scrolling text and animations from ProPresenter are not copied.".into());
        // unnamed designs are named after their text boxes ("UNITED – Reference, Scripture")
        let label = if !slide_name.is_empty() {
            slide_name.clone()
        } else {
            let t: Vec<String> = texts.iter().map(|(j, _)| names[*j].clone()).filter(|n| !n.is_empty()).take(2).collect();
            if t.is_empty() { format!("design {}", si + 1) } else { t.join(", ") }
        };
        let name = if single { theme_name.to_string() } else { format!("{theme_name} – {label}") };
        let theme = json!({ "schema_version": 1, "name": name, "kind": kind, "background": bg, "transition": { "kind": "fade", "ms": 300 }, "layers": layers });
        out.push(Design {
            name,
            kind: kind.into(),
            preview: with_inline_media(&theme, &media),
            notes,
            suggested: kind != "announcement" || single,
            package: package(&theme, &media),
        });
        let _ = si;
    }
    out
}

/// A .proTheme (zip of themes) or a single ProPresenter Theme file.
pub fn read_pro_theme(bytes: &[u8]) -> Result<Vec<Design>> {
    if bytes.starts_with(b"PK") {
        let files = zip_files(bytes)?;
        let mut themes: BTreeMap<String, (Option<Vec<u8>>, BTreeMap<String, Vec<u8>>)> = BTreeMap::new();
        for (name, data) in files {
            let mut parts = name.splitn(2, '/');
            let top = parts.next().unwrap_or("").to_string();
            let rest = parts.next().unwrap_or("");
            let e = themes.entry(top).or_default();
            if rest == "Theme" {
                e.0 = Some(data);
            } else if let Some(a) = rest.strip_prefix("Assets/") {
                e.1.insert(a.to_string(), data);
            }
        }
        let mut out = vec![];
        for (name, (theme, assets)) in themes {
            if let Some(t) = theme {
                out.extend(read_pro_theme_file(&name, &t, &assets));
            }
        }
        if out.is_empty() {
            return Err(bad("No ProPresenter themes were found in this file."));
        }
        Ok(out)
    } else {
        let d = read_pro_theme_file("ProPresenter theme", bytes, &BTreeMap::new());
        if d.is_empty() {
            return Err(bad("This is not a ProPresenter theme."));
        }
        Ok(d)
    }
}

/// Every design in an EasyWorship or ProPresenter theme file, for the review.
pub fn review(path: &std::path::Path) -> Result<Vec<Design>> {
    let bytes = std::fs::read(path)?;
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "ewtx" | "ewt" => read_ewtx(&bytes),
        "protheme" | "pro6template" => read_pro_theme(&bytes),
        _ => {
            if let Ok(d) = read_ewtx(&bytes) {
                return Ok(d);
            }
            read_pro_theme(&bytes).map_err(|_| bad("Choose an EasyWorship theme (.ewtx) or a ProPresenter theme (.proTheme)."))
        }
    }
}

/// Imports the chosen designs (indexes from `review`) as themes; returns (id, name).
pub fn import(conn: &rusqlite::Connection, media_dir: &std::path::Path, path: &std::path::Path, chosen: &[usize]) -> Result<Vec<(i64, String)>> {
    let designs = review(path)?;
    let mut out = vec![];
    for i in chosen {
        if let Some(d) = designs.get(*i) {
            out.push(crate::themes::import(conn, media_dir, &d.package)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_weights() {
        assert_eq!(ew_color(65280), "#00ff00");
        assert_eq!(ew_color(255), "#ff0000", "BGR: 255 is red");
        assert_eq!(hex_rgba(1.0, 1.0, 1.0, 1.0), "#ffffff");
        assert_eq!(weight_of("Helvetica-BoldOblique"), (700, true));
        assert_eq!(weight_of("Parkinsans GH ExtraBold").0, 800);
    }

    #[test]
    fn real_theme_files_when_present() {
        let Some(dir) = std::env::var_os("WP_THEMES").map(std::path::PathBuf::from) else { return };
        let ew = review(&dir.join("BLACK.ewtx")).unwrap();
        assert_eq!(ew.len(), 1);
        let t = &ew[0].preview;
        assert_eq!((ew[0].name.as_str(), ew[0].kind.as_str()), ("BLACK", "song"));
        assert_eq!(t["background"]["kind"], "image");
        assert!(t["background"]["image"].as_str().unwrap().starts_with("data:image/jpeg;base64,"), "picture inside the preview");
        let lyr = t["layers"].as_array().unwrap().iter().find(|l| l["field"] == "lyrics").unwrap();
        assert_eq!(lyr["style"]["font"], "Parkinsans GH ExtraBold");
        assert!((lyr["style"]["size"].as_f64().unwrap() - 12.8).abs() < 0.2, "{}", lyr["style"]["size"]);
        let label = t["layers"].as_array().unwrap().iter().find(|l| l["field"] == "section").unwrap();
        assert_eq!(label["style"]["color"], "#00ff00", "the green verse label");

        let pp = review(&dir.join("Theme.proTheme")).unwrap();
        println!("{} ProPresenter designs:", pp.len());
        for d in &pp {
            println!("  {} [{}] {}", d.name, d.kind, if d.suggested { "*" } else { "" });
        }
        assert!(pp.len() > 30);
        let vib = pp.iter().find(|d| d.name == "VIBIN PIXELS - Pink & Blue – Title").unwrap();
        let layers = vib.preview["layers"].as_array().unwrap();
        assert!(layers.iter().any(|l| l["kind"] == "image" && l["src"].as_str().unwrap().starts_with("data:image/png")), "the pixels picture");
        let shape = layers.iter().find(|l| l["name"] == "Title Background Shape box").unwrap();
        assert_eq!(shape["fill"], "#fc6fcf", "pink box");
        let scr = pp.iter().find(|d| d.name == "Black – Scripture").unwrap();
        assert_eq!(scr.kind, "scripture");
        let ls = scr.preview["layers"].as_array().unwrap();
        assert!(ls.iter().any(|l| l["field"] == "reference") && ls.iter().any(|l| l["field"] == "lyrics"));
        assert!(pp.iter().all(|d| !d.name.contains("Countdowns")), "countdown timers are left out");
        // for the browser preview's sample data (test runs only)
        if let Some(out) = std::env::var_os("WP_THEMES_DUMP") {
            let all: Vec<&Design> = ew.iter().chain(pp.iter()).collect();
            std::fs::write(out, serde_json::to_vec(&all).unwrap()).unwrap();
        }
    }
}
