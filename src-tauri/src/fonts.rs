//! The fonts installed on this computer, for the theme editor's font list.
//! Each family is checked for the Ghanaian letters ɛ ɔ ŋ, so fonts that lack them
//! can be marked (Noto Sans then fills those letters in).

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize, Clone)]
pub struct FontFamily {
    pub name: String,
    /// Has ɛ, ɔ and ŋ (Akan, Ga, Ewe, Dagbani letters).
    pub ghanaian: bool,
    pub styles: usize,
}

pub fn installed() -> Vec<FontFamily> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut fams: BTreeMap<String, FontFamily> = BTreeMap::new();
    let ids: Vec<(fontdb::ID, String)> = db
        .faces()
        .filter_map(|f| f.families.first().map(|(name, _)| (f.id, name.clone())))
        .filter(|(_, n)| !n.starts_with('.') && !n.starts_with('@'))
        .collect();
    for (id, name) in ids {
        let entry = fams.entry(name.to_lowercase()).or_insert_with(|| FontFamily { name: name.clone(), ghanaian: false, styles: 0 });
        entry.styles += 1;
        if !entry.ghanaian {
            entry.ghanaian = db
                .with_face_data(id, |data, index| ttf_parser::Face::parse(data, index).map(|f| ['ɛ', 'ɔ', 'ŋ'].iter().all(|c| f.glyph_index(*c).is_some())).unwrap_or(false))
                .unwrap_or(false);
        }
    }
    fams.into_values().collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn lists_installed_fonts_with_coverage() {
        let t = std::time::Instant::now();
        let all = super::installed();
        println!("{} families in {:?}", all.len(), t.elapsed());
        for f in all.iter().filter(|f| f.name.starts_with("Noto Sans") || f.name.starts_with("DejaVu") || f.name.starts_with("Liberation")).take(8) {
            println!("  {} ghanaian={} styles={}", f.name, f.ghanaian, f.styles);
        }
        assert!(!all.is_empty());
    }
}
