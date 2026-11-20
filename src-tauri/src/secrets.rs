//! Secrets (stream keys): encrypted with Windows' data protection (DPAPI), so only this
//! Windows user on this computer can read them, and kept in this computer's settings
//! (never synced, never in the interface's storage, never written to the log).

use crate::{err, AppState, Res};
use tauri::State;

#[cfg(windows)]
fn protect(data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    let ok = unsafe {
        if encrypt {
            CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out)
        }
    };
    if ok == 0 || out.pbData.is_null() {
        return Err("Windows could not protect or read the stream key.".into());
    }
    let v = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec() };
    unsafe { LocalFree(out.pbData as _) };
    Ok(v)
}

/// Off Windows (development only) the value is kept as it is.
#[cfg(not(windows))]
fn protect(data: &[u8], _encrypt: bool) -> Result<Vec<u8>, String> {
    Ok(data.to_vec())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    (0..s.len()).step_by(2).map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok())).collect()
}

pub fn set(conn: &rusqlite::Connection, name: &str, value: &str) -> Result<(), String> {
    let key = format!("secret:{name}");
    if value.trim().is_empty() {
        conn.execute("DELETE FROM setting WHERE key = ?1", [&key]).map_err(err)?;
    } else {
        let sealed = protect(value.trim().as_bytes(), true)?;
        conn.execute("INSERT OR REPLACE INTO setting (key, value) VALUES (?1, ?2)", [&key, &hex(&sealed)]).map_err(err)?;
    }
    Ok(())
}

pub fn get(conn: &rusqlite::Connection, name: &str) -> Option<String> {
    let v: String = conn.query_row("SELECT value FROM setting WHERE key = ?1", [format!("secret:{name}")], |r| r.get(0)).ok()?;
    String::from_utf8(protect(&unhex(&v)?, false).ok()?).ok()
}

/// Saves (or, when empty, removes) a stream key: kind "youtube", "facebook" or "custom".
#[tauri::command(async)]
pub fn stream_key_set(st: State<'_, AppState>, kind: String, value: String) -> Res<()> {
    if !matches!(kind.as_str(), "youtube" | "facebook" | "custom") {
        return Err("Unknown destination.".into());
    }
    set(&st.db.lock().unwrap(), &format!("stream:{kind}"), &value)
}

/// Which stream keys are saved (never the keys themselves).
#[tauri::command(async)]
pub fn stream_keys_saved(st: State<'_, AppState>) -> std::collections::HashMap<String, bool> {
    let conn = st.db.lock().unwrap();
    ["youtube", "facebook", "custom"].iter().map(|k| (k.to_string(), get(&conn, &format!("stream:{k}")).is_some())).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn keys_round_trip_and_stay_out_of_plain_sight() {
        let c = wp_core::db::open_memory().unwrap();
        super::set(&c, "stream:youtube", "abcd-efgh-ijkl").unwrap();
        assert_eq!(super::get(&c, "stream:youtube").as_deref(), Some("abcd-efgh-ijkl"));
        let stored: String = c.query_row("SELECT value FROM setting WHERE key = 'secret:stream:youtube'", [], |r| r.get(0)).unwrap();
        assert!(!stored.contains("abcd"), "stored sealed (hex), not as typed");
        super::set(&c, "stream:youtube", "").unwrap();
        assert_eq!(super::get(&c, "stream:youtube"), None, "emptied: removed");
    }
}
