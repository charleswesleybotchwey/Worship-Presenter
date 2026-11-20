//! Free pictures and videos from Pixabay and Unsplash, searched and downloaded into the
//! media library (as FreeShow offers). Each needs the church's own free API key, kept
//! protected on this computer. Their rules are followed: Unsplash downloads are reported
//! to Unsplash and the photographer is credited (the credit is kept in the media title);
//! Pixabay files are downloaded rather than linked.

use crate::{err, AppState, Res};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Found {
    /// "pixabay" or "unsplash"
    pub source: String,
    pub id: String,
    /// "image" or "video"
    pub kind: String,
    pub title: String,
    pub thumb: String,
    /// The file to download.
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub author: String,
    pub author_url: String,
    pub page_url: String,
    /// Unsplash: the address that must be called when the photo is downloaded.
    pub track: Option<String>,
}

const UTM: &str = "utm_source=worship_presenter&utm_medium=referral";

fn with_utm(url: &str) -> String {
    if url.is_empty() {
        return String::new();
    }
    format!("{url}{}{UTM}", if url.contains('?') { "&" } else { "?" })
}

pub fn pixabay_photos(json: &str) -> Vec<Found> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    v["hits"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|h| {
                    let file = h["largeImageURL"].as_str().or(h["webformatURL"].as_str())?;
                    let tags = h["tags"].as_str().unwrap_or("picture");
                    Some(Found {
                        source: "pixabay".into(),
                        id: h["id"].as_i64()?.to_string(),
                        kind: "image".into(),
                        title: tags.split(',').next().unwrap_or(tags).trim().to_string(),
                        thumb: h["previewURL"].as_str().or(h["webformatURL"].as_str()).unwrap_or(file).to_string(),
                        file: file.to_string(),
                        width: h["imageWidth"].as_u64().unwrap_or(0) as u32,
                        height: h["imageHeight"].as_u64().unwrap_or(0) as u32,
                        author: h["user"].as_str().unwrap_or("").to_string(),
                        author_url: format!("https://pixabay.com/users/{}-{}/", h["user"].as_str().unwrap_or(""), h["user_id"].as_i64().unwrap_or(0)),
                        page_url: h["pageURL"].as_str().unwrap_or("").to_string(),
                        track: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn pixabay_videos(json: &str) -> Vec<Found> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    v["hits"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|h| {
                    // the largest size up to full HD
                    let size = ["large", "medium", "small", "tiny"].iter().map(|k| &h["videos"][*k]).find(|s| s["url"].as_str().is_some_and(|u| !u.is_empty()))?;
                    let tags = h["tags"].as_str().unwrap_or("video");
                    Some(Found {
                        source: "pixabay".into(),
                        id: h["id"].as_i64()?.to_string(),
                        kind: "video".into(),
                        title: tags.split(',').next().unwrap_or(tags).trim().to_string(),
                        thumb: size["thumbnail"].as_str().or(h["videos"]["tiny"]["thumbnail"].as_str()).unwrap_or("").to_string(),
                        file: size["url"].as_str()?.to_string(),
                        width: size["width"].as_u64().unwrap_or(0) as u32,
                        height: size["height"].as_u64().unwrap_or(0) as u32,
                        author: h["user"].as_str().unwrap_or("").to_string(),
                        author_url: format!("https://pixabay.com/users/{}-{}/", h["user"].as_str().unwrap_or(""), h["user_id"].as_i64().unwrap_or(0)),
                        page_url: h["pageURL"].as_str().unwrap_or("").to_string(),
                        track: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn unsplash_photos(json: &str) -> Vec<Found> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
    v["results"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    let raw = p["urls"]["raw"].as_str()?;
                    let title = p["alt_description"].as_str().or(p["description"].as_str()).unwrap_or("Photo");
                    Some(Found {
                        source: "unsplash".into(),
                        id: p["id"].as_str()?.to_string(),
                        kind: "image".into(),
                        title: title.to_string(),
                        thumb: p["urls"]["small"].as_str().or(p["urls"]["thumb"].as_str()).unwrap_or(raw).to_string(),
                        // full HD is plenty for a projector
                        file: format!("{raw}&w=1920&q=85&fm=jpg"),
                        width: p["width"].as_u64().unwrap_or(0) as u32,
                        height: p["height"].as_u64().unwrap_or(0) as u32,
                        author: p["user"]["name"].as_str().unwrap_or("").to_string(),
                        author_url: with_utm(p["user"]["links"]["html"].as_str().unwrap_or("")),
                        page_url: with_utm(p["links"]["html"].as_str().unwrap_or("")),
                        track: p["links"]["download_location"].as_str().map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The media title: what it shows, and who made it (as the services ask).
pub fn credited_title(f: &Found) -> String {
    let what: String = f.title.chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).take(60).collect();
    let site = if f.source == "unsplash" { "Unsplash" } else { "Pixabay" };
    if f.author.is_empty() { format!("{what} — {site}") } else { format!("{what} — by {} on {site}", f.author) }
}

fn enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn agent() -> ureq::Agent {
    // searches: 20 s to connect, 30 s with no answer
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(20)).timeout_read(Duration::from_secs(30)).user_agent(concat!("Worship Presenter/", env!("CARGO_PKG_VERSION"))).build()
}

fn key(st: &AppState, service: &str) -> Res<String> {
    crate::secrets::get(&st.db.lock().unwrap(), &format!("online:{service}")).ok_or_else(|| format!("Add your free {} key first (in the Online panel).", if service == "unsplash" { "Unsplash" } else { "Pixabay" }))
}

/// Saves (or, empty, removes) a key: "pixabay" or "unsplash".
#[tauri::command(async)]
pub fn online_key_set(st: State<'_, AppState>, service: String, value: String) -> Res<()> {
    if !matches!(service.as_str(), "pixabay" | "unsplash") {
        return Err("Unknown service.".into());
    }
    crate::secrets::set(&st.db.lock().unwrap(), &format!("online:{service}"), &value)
}

#[tauri::command(async)]
pub fn online_keys_saved(st: State<'_, AppState>) -> std::collections::HashMap<String, bool> {
    let conn = st.db.lock().unwrap();
    ["pixabay", "unsplash"].iter().map(|s| (s.to_string(), crate::secrets::get(&conn, &format!("online:{s}")).is_some())).collect()
}

/// Searches one source: "pixabay-photo", "pixabay-video" or "unsplash".
#[tauri::command]
pub async fn online_search(app: AppHandle, source: String, query: String, page: u32) -> Res<Vec<Found>> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let q = enc(query.trim());
        let page = page.max(1);
        let call = |r: ureq::Request| -> Res<String> {
            match r.call() {
                Ok(resp) => resp.into_string().map_err(err),
                Err(ureq::Error::Status(401 | 403, _)) => Err("The service refused the key. Check it in the Online panel.".into()),
                Err(ureq::Error::Status(429, _)) => Err("Too many searches for now: wait a little and try again.".into()),
                Err(e) => Err(format!("No answer ({e}). Check the internet connection.")),
            }
        };
        match source.as_str() {
            "pixabay-photo" => {
                let k = key(&st, "pixabay")?;
                Ok(pixabay_photos(&call(agent().get(&format!("https://pixabay.com/api/?key={}&q={q}&image_type=photo&safesearch=true&per_page=30&page={page}", enc(&k))))?))
            }
            "pixabay-video" => {
                let k = key(&st, "pixabay")?;
                Ok(pixabay_videos(&call(agent().get(&format!("https://pixabay.com/api/videos/?key={}&q={q}&safesearch=true&per_page=30&page={page}", enc(&k))))?))
            }
            "unsplash" => {
                let k = key(&st, "unsplash")?;
                let r = agent().get(&format!("https://api.unsplash.com/search/photos?query={q}&per_page=30&page={page}&content_filter=high")).set("Authorization", &format!("Client-ID {k}")).set("Accept-Version", "v1");
                Ok(unsplash_photos(&call(r)?))
            }
            _ => Err("Unknown source.".into()),
        }
    })
    .await
    .map_err(err)?
}

/// Downloads one result into the media library (titled with its credit).
#[tauri::command]
pub async fn online_download(app: AppHandle, item: Found) -> Res<wp_core::media::MediaItem> {
    tauri::async_runtime::spawn_blocking(move || {
        let st = app.state::<AppState>();
        let allowed = ["https://cdn.pixabay.com/", "https://pixabay.com/", "https://images.unsplash.com/"];
        if !allowed.iter().any(|a| item.file.starts_with(a)) {
            return Err("That file is not from Pixabay or Unsplash.".into());
        }
        // Unsplash asks that each download be reported to them
        if let (Some(track), "unsplash") = (&item.track, item.source.as_str()) {
            if track.starts_with("https://api.unsplash.com/") {
                if let Ok(k) = key(&st, "unsplash") {
                    let _ = agent().get(track).set("Authorization", &format!("Client-ID {k}")).call();
                }
            }
        }
        let ext = if item.kind == "video" { "mp4" } else { "jpg" };
        let dir = std::env::temp_dir().join(format!("wp-online-{}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(err)?;
        let path = dir.join(format!("{}.{ext}", credited_title(&item)));
        // to disk as it arrives, resuming after a dropped connection (videos are large)
        let progress = |m: String| *st.speech.install.lock().unwrap() = Some(m);
        let got = crate::speech::download_to(&item.file, &path, &progress, if item.kind == "video" { "video" } else { "picture" });
        *st.speech.install.lock().unwrap() = None;
        got?;
        let m = wp_core::media::import(&st.db.lock().unwrap(), &st.profile_dir().join("media"), &path).map_err(err);
        let _ = std::fs::remove_file(&path);
        m
    })
    .await
    .map_err(err)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_from_each_service() {
        let px = r#"{"total":2,"hits":[{"id":195893,"pageURL":"https://pixabay.com/en/blossom-195893/","tags":"blossom, bloom, flower","previewURL":"https://cdn.pixabay.com/photo/2013/10/15/09/12/flower-195893_150.jpg","webformatURL":"https://pixabay.com/get/35bbf209_640.jpg","largeImageURL":"https://pixabay.com/get/ed6a99fd0a76647_1280.jpg","imageWidth":4000,"imageHeight":2250,"user_id":48777,"user":"Josch13"}]}"#;
        let p = pixabay_photos(px);
        assert_eq!((p[0].title.as_str(), p[0].author.as_str(), p[0].kind.as_str()), ("blossom", "Josch13", "image"));
        assert!(p[0].file.ends_with("_1280.jpg"));
        let pv = r#"{"hits":[{"id":125,"pageURL":"https://pixabay.com/videos/id-125/","tags":"flowers, yellow","duration":12,"videos":{"large":{"url":"","width":0,"height":0},"medium":{"url":"https://cdn.pixabay.com/video/2015/08/08/125-135736646_medium.mp4","width":1280,"height":720,"thumbnail":"https://cdn.pixabay.com/video/2015/08/08/125-135736646_medium.jpg"}},"user_id":1281706,"user":"Coverr-Free-Footage"}]}"#;
        let v = pixabay_videos(pv);
        assert_eq!((v[0].width, v[0].kind.as_str()), (1280, "video"), "empty large size skipped");
        let un = r#"{"total":1,"results":[{"id":"eOLpJytrbsQ","width":4000,"height":3000,"alt_description":"sunrise over the hills","urls":{"raw":"https://images.unsplash.com/photo-1416339306562?ixid=abc","small":"https://images.unsplash.com/photo-1416339306562?w=400"},"links":{"html":"https://unsplash.com/photos/eOLpJytrbsQ","download_location":"https://api.unsplash.com/photos/eOLpJytrbsQ/download?ixid=abc"},"user":{"name":"Jeff Sheldon","links":{"html":"https://unsplash.com/@ugmonk"}}}]}"#;
        let u = unsplash_photos(un);
        assert_eq!(u[0].file, "https://images.unsplash.com/photo-1416339306562?ixid=abc&w=1920&q=85&fm=jpg");
        assert_eq!(u[0].author_url, "https://unsplash.com/@ugmonk?utm_source=worship_presenter&utm_medium=referral", "credit links carry the referral Unsplash asks for");
        assert!(u[0].track.as_deref().unwrap().starts_with("https://api.unsplash.com/"));
        assert_eq!(credited_title(&u[0]), "sunrise over the hills — by Jeff Sheldon on Unsplash");
        assert!(pixabay_photos("not json").is_empty() && unsplash_photos("{}").is_empty());
    }
}

/// Opens a Pixabay or Unsplash page (a photographer's credit) in the web browser.
#[tauri::command(async)]
pub fn online_open(url: String) -> Res<()> {
    let ok = ["https://pixabay.com/", "https://unsplash.com/"].iter().any(|p| url.starts_with(p)) && !url.contains(['"', ' ', '\n']);
    if !ok {
        return Err("Only Pixabay and Unsplash pages open from here.".into());
    }
    let r = if cfg!(windows) {
        std::process::Command::new("explorer").arg(&url).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(&url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(&url).spawn()
    };
    r.map(drop).map_err(|e| format!("The page could not be opened ({e})."))
}
