//! Creeds and prayers (texts out of copyright, the traditional English wording), added to the
//! library as songs in a "Creeds and prayers" collection, ready for the schedule. A church's
//! own Fante, Twi or Ga versions are added as a second language in the song editor.

use crate::songs::{Lyric, SongDoc};
use crate::Result;
use rusqlite::{Connection, OptionalExtension};

pub const COLLECTION: &str = "Creeds and prayers";

/// (title, source, body with sections)
const TEXTS: &[(&str, &str, &str)] = &[
    (
        "The Apostles' Creed",
        "Book of Common Prayer (1662)",
        "[V1]\nI believe in God the Father Almighty,\nMaker of heaven and earth:\nAnd in Jesus Christ his only Son our Lord,\nWho was conceived by the Holy Ghost,\nBorn of the Virgin Mary,\nSuffered under Pontius Pilate,\n\n[V2]\nWas crucified, dead, and buried:\nHe descended into hell;\nThe third day he rose again from the dead;\nHe ascended into heaven,\nAnd sitteth on the right hand of God the Father Almighty;\nFrom thence he shall come to judge the quick and the dead.\n\n[V3]\nI believe in the Holy Ghost;\nThe holy Catholick Church;\nThe Communion of Saints;\nThe Forgiveness of sins;\nThe Resurrection of the body,\nAnd the Life everlasting. Amen.",
    ),
    (
        "The Nicene Creed",
        "Book of Common Prayer (1662)",
        "[V1]\nI believe in one God the Father Almighty,\nMaker of heaven and earth,\nAnd of all things visible and invisible:\nAnd in one Lord Jesus Christ, the only-begotten Son of God,\nBegotten of his Father before all worlds,\nGod of God, Light of Light,\nVery God of very God,\nBegotten, not made,\nBeing of one substance with the Father;\nBy whom all things were made:\n\n[V2]\nWho for us men, and for our salvation came down from heaven,\nAnd was incarnate by the Holy Ghost of the Virgin Mary,\nAnd was made man,\nAnd was crucified also for us under Pontius Pilate.\nHe suffered and was buried,\nAnd the third day he rose again according to the Scriptures,\nAnd ascended into heaven,\nAnd sitteth on the right hand of the Father.\nAnd he shall come again with glory to judge both the quick and the dead:\nWhose kingdom shall have no end.\n\n[V3]\nAnd I believe in the Holy Ghost,\nThe Lord and giver of life,\nWho proceedeth from the Father and the Son,\nWho with the Father and the Son together is worshipped and glorified,\nWho spake by the Prophets.\nAnd I believe one Catholick and Apostolick Church.\nI acknowledge one Baptism for the remission of sins.\nAnd I look for the Resurrection of the dead,\nAnd the life of the world to come. Amen.",
    ),
    (
        "The Lord's Prayer",
        "Traditional",
        "[V1]\nOur Father, which art in heaven,\nHallowed be thy Name.\nThy Kingdom come.\nThy will be done in earth,\nAs it is in heaven.\nGive us this day our daily bread.\n\n[V2]\nAnd forgive us our trespasses,\nAs we forgive them that trespass against us.\nAnd lead us not into temptation,\nBut deliver us from evil:\nFor thine is the kingdom,\nThe power, and the glory,\nFor ever and ever. Amen.",
    ),
    (
        "Gloria Patri",
        "Traditional",
        "[V1]\nGlory be to the Father, and to the Son,\nAnd to the Holy Ghost;\nAs it was in the beginning, is now, and ever shall be,\nWorld without end. Amen.",
    ),
    (
        "The Doxology",
        "Thomas Ken (1674)",
        "[V1]\nPraise God, from whom all blessings flow;\nPraise Him, all creatures here below;\nPraise Him above, ye heavenly host;\nPraise Father, Son, and Holy Ghost. Amen.",
    ),
    (
        "Psalm 24 (responsive reading)",
        "Psalm 24, King James Version",
        "[V1]\nLeader: The earth is the LORD's, and the fulness thereof; the world, and they that dwell therein.\nPeople: For he hath founded it upon the seas, and established it upon the floods.\n\n[V2]\nLeader: Who shall ascend into the hill of the LORD? or who shall stand in his holy place?\nPeople: He that hath clean hands, and a pure heart; who hath not lifted up his soul unto vanity, nor sworn deceitfully.\n\n[V3]\nLeader: He shall receive the blessing from the LORD, and righteousness from the God of his salvation.\nPeople: This is the generation of them that seek him, that seek thy face, O Jacob. Selah.\n\n[V4]\nLeader: Lift up your heads, O ye gates; and be ye lift up, ye everlasting doors; and the King of glory shall come in.\nPeople: Who is this King of glory? The LORD strong and mighty, the LORD mighty in battle.\n\n[V5]\nLeader: Lift up your heads, O ye gates; even lift them up, ye everlasting doors; and the King of glory shall come in.\nAll: Who is this King of glory? The LORD of hosts, he is the King of glory. Selah.",
    ),
];

/// Adds the creeds and prayers not yet in the library (same title); returns (added, already there).
pub fn add(conn: &Connection) -> Result<(usize, usize)> {
    let cid: i64 = match conn.query_row("SELECT id FROM my_collection WHERE name = ?1", [COLLECTION], |r| r.get(0)).optional()? {
        Some(c) => c,
        None => crate::browse::create_collection(conn, COLLECTION)?,
    };
    let (mut added, mut there) = (0, 0);
    for (title, source, body) in TEXTS {
        let exists: bool = conn.query_row("SELECT count(*) FROM song WHERE lower(title) = lower(?1)", [title], |r| r.get::<_, i64>(0))? > 0;
        if exists {
            there += 1;
            continue;
        }
        let id = crate::songs::save(
            conn,
            &SongDoc {
                id: None,
                title: (*title).into(),
                author: Some((*source).into()),
                copyright: Some("Public Domain".into()),
                ccli: None,
                numbers: vec![],
                lyrics: vec![Lyric { lang: "en".into(), body: (*body).into() }],
                sequence: String::new(),
                lines_per_slide: Some(2),
                key: None,
            },
        )?;
        crate::browse::add_to_collection(conn, cid, &[id])?;
        added += 1;
    }
    Ok((added, there))
}

#[cfg(test)]
mod tests {
    #[test]
    fn creeds_and_prayers_are_added_once() {
        let c = crate::db::open_memory().unwrap();
        assert_eq!(super::add(&c).unwrap(), (6, 0));
        assert_eq!(super::add(&c).unwrap(), (0, 6), "nothing twice");
        let n: i64 = c.query_row("SELECT count(*) FROM my_collection_item i JOIN my_collection m ON m.id = i.collection_id WHERE m.name = 'Creeds and prayers'", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 6);
        let psalm: String = c.query_row("SELECT body FROM song_lyric l JOIN song s ON s.id = l.song_id WHERE s.title LIKE 'Psalm 24%'", [], |r| r.get(0)).unwrap();
        assert!(psalm.contains("Leader: The earth is the LORD's") && psalm.contains("People: For he hath founded it"));
    }
}
